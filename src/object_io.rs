use crate::{
    errors::*,
    names::{KnownName, Name},
    objects::*,
};
use byteorder::*;
use std::{
    collections::HashMap,
    fs::File,
    io,
    io::{BufWriter, Cursor, Seek, SeekFrom, Write},
    sync::Arc,
};
use twox_hash::RandomXxh3HashBuilder64;
use zstd::{
    dict::EncoderDictionary,
    zstd_safe::{CParameter, CompressionLevel},
    Encoder,
};

const ARC_HEADER: u64 = u64::from_le_bytes(*b"DiarArc1");
const END_HEADER: u64 = u64::from_le_bytes(*b"DiarEnd1");

/// A trait for stream-like objects that can be efficiently truncated.
pub trait Truncate {
    /// Truncates the stream to a certain length.
    ///
    /// If the current location in the stream is past the end, this will also set the cursor to
    /// the end of the stream.
    fn truncate(&mut self, len: u64) -> io::Result<()>;
}
impl Truncate for Cursor<Vec<u8>> {
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        if self.position() > len {
            self.set_position(len);
        }
        self.get_mut().truncate(len as usize);
        Ok(())
    }
}
impl Truncate for File {
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        if self.seek(SeekFrom::Current(0))? > len {
            self.seek(SeekFrom::Start(len))?;
        }
        self.set_len(len)
    }
}

pub struct DiarIo<S> {
    stream: S,
    obj_ids: HashMap<ObjectId, u64, RandomXxh3HashBuilder64>,
    rel_offset: u64,
}
impl<S: Write + Seek> DiarIo<S> {
    pub fn create(mut stream: S) -> Result<Self> {
        stream.write_u64::<LE>(ARC_HEADER)?;
        let rel_offset = stream.seek(SeekFrom::Current(0))?;
        Ok(DiarIo { stream, obj_ids: Default::default(), rel_offset })
    }

    fn write_varint(&mut self, mut data: i64) -> Result<()> {
        if data < 0 {
            data = data ^ 0x7FFFFFFFFFFFFFFF;
        }
        data = (data << 1) | ((data >> 63) & 1);
        let mut data = data as u64;

        loop {
            let frag = data & 0x7F;
            data = data >> 7;

            if data == 0 {
                self.stream.write_u8(frag as u8)?;
                break;
            } else {
                self.stream.write_u8(0x80 | frag as u8)?;
            }
        }
        Ok(())
    }
    fn write_varuint(&mut self, mut data: u64) -> Result<()> {
        loop {
            let frag = data & 0x7F;
            data = data >> 7;

            if data == 0 {
                self.stream.write_u8(frag as u8)?;
                break;
            } else {
                self.stream.write_u8(0x80 | frag as u8)?;
            }
        }
        Ok(())
    }
    fn get_object_offset(&self, id: ObjectId) -> Result<u64> {
        if id == ObjectId::NONE {
            Ok(0)
        } else {
            match self.obj_ids.get(&id) {
                Some(offset) => Ok(*offset),
                None => ErrorContents::ObjectIdError(id).emit(),
            }
        }
    }
    fn write_object_id(&mut self, id: ObjectId) -> Result<()> {
        let id = self.get_object_offset(id)?;
        self.write_varuint(id)
    }
    fn write_object_ids(&mut self, list: &[ObjectId]) -> Result<()> {
        for id in list {
            self.write_object_id(*id)?;
        }
        self.write_object_id(ObjectId::NONE)?;
        Ok(())
    }
    fn write_full_string(&mut self, value: &str) -> Result<()> {
        self.write_varuint(value.len() as u64)?;
        self.stream.write_all(value.as_bytes())?;
        Ok(())
    }

    fn write_metadata(&mut self, metadata: &Metadata) -> Result<()> {
        match metadata {
            Metadata::VarInt(v) => {
                self.write_varuint(META_TAG_VARINT)?;
                self.write_varint(*v)?;
            }
            Metadata::VarUInt(v) => {
                self.write_varuint(META_TAG_VARUINT)?;
                self.write_varuint(*v)?;
            }
            Metadata::ObjectRef(v) => {
                self.write_varuint(META_TAG_OBJECTREF)?;
                self.write_object_id(*v)?;
            }
            Metadata::String(v) => {
                self.write_varuint(META_TAG_STRING)?;
                self.write_full_string(v)?;
            }
        }
        Ok(())
    }
    fn write_metadata_table(&mut self, table: &MetadataMap) -> Result<()> {
        for (k, v) in table {
            ensure(*k != MetadataTag::EndTag, &"early EndTag encountered!")?;
            self.write_varuint(*k as u64)?;
            self.write_metadata(v)?;
        }
        self.write_varuint(MetadataTag::EndTag as u64)?;
        Ok(())
    }

    pub fn write_object(&mut self, obj: &DiarObject) -> Result<ObjectId> {
        self.write_object_contents(obj, 0)
    }
    pub fn write_object_with_data(
        &mut self,
        obj: &DiarObject,
        data_write: impl FnOnce(&mut S) -> Result<()>,
    ) -> Result<ObjectId> {
        let start_offset = self.stream.seek(SeekFrom::Current(0))?;
        data_write(&mut self.stream)?;
        let end_offset = self.stream.seek(SeekFrom::Current(0))?;
        let length = end_offset - start_offset;
        self.write_object_contents(obj, length)
    }
    fn write_object_contents(&mut self, obj: &DiarObject, length: u64) -> Result<ObjectId> {
        let header_off = self.stream.seek(SeekFrom::Current(0))? - self.rel_offset;
        match obj {
            DiarObject::BlobPlain(obj) => {
                self.write_varuint(ObjectType::BlobPlain as u64)?;
                self.write_object_ids(&obj.filters)?;
            }
            DiarObject::Directory(obj) => {
                self.write_varuint(ObjectType::Directory as u64)?;
                ensure(length == 0, &"length not allowed for Directory")?;
                for entry in &obj.entries {
                    self.write_object_id(entry.data)?;
                    self.write_object_id(entry.metadata)?;
                    self.write_full_string(&entry.name)?;
                }
                self.write_object_id(ObjectId::NONE)?;
            }
            DiarObject::Metadata(obj) => {
                self.write_varuint(ObjectType::Metadata as u64)?;
                ensure(length == 0, &"length not allowed for Metadata")?;
                self.write_metadata_table(&obj.metadata)?;
            }
            DiarObject::Archive(obj) => {
                self.write_varuint(ObjectType::Archive as u64)?;
                ensure(length == 0, &"length not allowed for Archive")?;
                self.write_object_id(obj.root)?;
                self.write_metadata_table(&obj.metadata)?;
            }
            DiarObject::Root(obj) => {
                self.write_varuint(ObjectType::Root as u64)?;
                ensure(length == 0, &"length not allowed for Root")?;
                self.write_object_id(obj.main)?;
                for (k, v) in &obj.alt {
                    self.write_object_id(*v)?;
                    self.write_full_string(k)?;
                }
                self.write_object_id(ObjectId::NONE)?;
                self.write_metadata_table(&obj.metadata)?;
            }
            DiarObject::FilterZstd(obj) => {
                self.write_varuint(ObjectType::FilterZstd as u64)?;
                ensure(length == 0, &"length not allowed for FilterZstd")?;
                self.write_object_ids(&obj.dict_sources)?;
            }
            DiarObject::ZstdPreloadList(obj) => {
                self.write_varuint(ObjectType::FilterZstd as u64)?;
                ensure(length == 0, &"length not allowed for ZstdPreloadList")?;
                self.write_object_ids(&obj.list)?;
            }
        }

        let id = ObjectId::new();
        self.obj_ids.insert(id, header_off);
        Ok(id)
    }

    pub fn finish(&mut self, root_id: ObjectId) -> Result<()> {
        let arc_end = self.stream.seek(SeekFrom::Current(0))?;
        let length = arc_end - self.rel_offset;

        let obj_offset = self.get_object_offset(root_id)?;

        self.stream.write_u64::<LE>(END_HEADER)?;
        self.stream.write_u64::<LE>(length)?;
        self.stream.write_u64::<LE>(obj_offset)?;
        Ok(())
    }
}

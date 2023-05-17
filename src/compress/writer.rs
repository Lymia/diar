use crate::{
    errors::*,
    names::{KnownName, Name},
};
use byteorder::*;
use std::{
    collections::HashMap,
    io,
    io::{BufWriter, Write},
    sync::Arc,
};
use zstd::{
    dict::EncoderDictionary,
    zstd_safe::{CParameter, CompressionLevel},
    Encoder,
};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct ObjectId(u64);

pub struct ByteCounter<W> {
    inner: W,
    count: u64,
}

impl<W: Write> ByteCounter<W> {
    fn new(inner: W) -> Self {
        ByteCounter { inner, count: 0 }
    }
}

impl<W: Write> Write for ByteCounter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = self.inner.write(buf);
        if let Ok(size) = res {
            self.count += size as u64;
        }
        res
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub struct DiarWriter<W: Write> {
    out: ByteCounter<W>,
    string_table_index: HashMap<Arc<str>, u64>,
    string_table: Vec<Arc<str>>,
}
impl<W: Write> DiarWriter<W> {
    pub fn new(mut w: W) -> Result<Self> {
        w.write_all(b"DiarArc1")?;
        Ok(DiarWriter {
            out: ByteCounter::new(w),
            string_table_index: Default::default(),
            string_table: vec![],
        })
    }

    fn count(&self) -> u64 {
        self.out.count
    }

    fn write_varuint(&mut self, mut data: u64) -> Result<()> {
        loop {
            let frag = data & 0x7F;
            data = data >> 7;

            if data == 0 {
                self.out.write_u8(frag as u8)?;
                break;
            } else {
                self.out.write_u8(0x80 | frag as u8)?;
            }
        }
        Ok(())
    }
    fn intern_str(&mut self, str: &str) -> u64 {
        match self.string_table_index.get(str) {
            Some(x) => *x,
            None => {
                let str: Arc<str> = str.into();
                let tok = self.string_table_index.len() as u64;

                self.string_table_index.insert(str.clone(), tok);
                self.string_table.push(str.clone());

                tok
            }
        }
    }

    fn write_name(&mut self, name: &Name<'_>) -> Result<()> {
        let id = self.intern_str(name.as_str());
        self.write_varuint(id)
    }
    fn write_known_name(&mut self, name: KnownName) -> Result<()> {
        let id = self.intern_str(name.as_str());
        self.write_varuint(id)
    }

    fn start_object(&mut self, name: KnownName) -> Result<ObjectId> {
        let id = ObjectId(self.count());
        self.write_known_name(name)?;
        Ok(id)
    }
    fn write_opt_object_ref(&mut self, id: Option<ObjectId>) -> Result<()> {
        match id {
            Some(x) => self.write_object_ref(x),
            None => self.write_varuint(0),
        }
    }
    fn write_object_ref(&mut self, id: ObjectId) -> Result<()> {
        self.write_varuint(id.0 + 1)
    }
    fn write_full_string(&mut self, value: &str) -> Result<()> {
        self.write_varuint(value.len() as u64)?;
        self.out.write_all(value.as_bytes())?;
        Ok(())
    }

    pub fn write_uncompressed_blob(
        &mut self,
        callback: impl FnOnce(&mut ByteCounter<W>) -> Result<()>,
    ) -> Result<ObjectId> {
        // write the raw object underlying the blob
        let off_start = self.count();
        callback(&mut self.out)?;
        let length = self.count() - off_start;

        // write the blob itself
        let blob = self.start_object(KnownName::CoreObjectBlobPlain)?;
        self.write_varuint(length)?;

        // return the blob itself
        Ok(blob)
    }
    pub fn write_compressed_blob(
        &mut self,
        level: CompressionLevel,
        dict: Option<(ObjectId, &EncoderDictionary)>,
        callback: impl FnOnce(&mut Encoder<BufWriter<&mut ByteCounter<W>>>) -> Result<()>,
    ) -> Result<ObjectId> {
        // write the raw object underlying the blob
        let off_start = self.count();

        let mut dict_id = None;
        let mut zstd = match dict {
            Some((id, dict)) => {
                dict_id = Some(id);
                Encoder::with_prepared_dictionary(BufWriter::new(&mut self.out), dict)
            }
            None => Encoder::new(BufWriter::new(&mut self.out), level),
        }?;
        zstd.include_checksum(true)?;
        zstd.set_parameter(CParameter::CompressionLevel(level))?;
        zstd.set_parameter(CParameter::WindowLog(30))?;
        zstd.set_parameter(CParameter::EnableDedicatedDictSearch(true))?;
        callback(&mut zstd)?;
        zstd.finish()?;

        let length = self.count() - off_start;

        // write the blob itself
        let blob = self.start_object(KnownName::CoreObjectBlobZstd)?;
        self.write_varuint(length)?;
        self.write_opt_object_ref(dict_id)?;

        // return the blob itself
        Ok(blob)
    }
    pub fn write_dictionary(&mut self, dictionary: &[u8]) -> Result<ObjectId> {
        let blob = self.write_compressed_blob(15, None, |x| {
            x.write_all(dictionary)?;
            Ok(())
        })?;

        let dict = self.start_object(KnownName::CoreObjectZstdDictionary)?;
        self.write_object_ref(blob)?;
        Ok(dict)
    }
    pub fn write_patch_dictionary(
        &mut self,
        raw_dict: Option<ObjectId>,
        patch_data: ObjectId,
    ) -> Result<ObjectId> {
        let dict = self.start_object(KnownName::CoreObjectZstdPatchDictionary)?;
        self.write_opt_object_ref(raw_dict)?;
        self.write_object_ref(patch_data)?;
        Ok(dict)
    }

    pub fn start_write_directory(&mut self) -> Result<WriteDirectory<W>> {
        let id = self.start_object(KnownName::CoreObjectDirectory)?;
        Ok(WriteDirectory(self, id))
    }

    pub fn finish(mut self, root: ObjectId) -> Result<()> {
        let offset_string_table = self.count();
        self.write_varuint(self.string_table.len() as u64)?;
        for string in self.string_table.clone() {
            self.write_full_string(&string)?;
        }

        let offset_end_header = self.count();
        self.out.write_all(b"DiarEnd1")?;
        self.out.write_u64::<LE>(offset_string_table)?;
        self.out.write_u64::<LE>(offset_end_header)?;
        self.out.write_u64::<LE>(root.0)?;
        Ok(())
    }
}

pub struct WriteDirectory<'a, W: Write>(&'a mut DiarWriter<W>, ObjectId);
impl<'a, W: Write> WriteDirectory<'a, W> {
    pub fn write_entry(
        &mut self,
        name: &str,
        obj: ObjectId,
        metadata: Option<ObjectId>,
    ) -> Result<()> {
        self.0.write_object_ref(obj)?;
        self.0.write_opt_object_ref(metadata)?;
        self.0.write_full_string(name)?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<ObjectId> {
        self.0.write_opt_object_ref(None)?;
        Ok(self.1)
    }
}

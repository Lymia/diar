use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::{
    collections::HashMap,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct ObjectId(u64);
impl ObjectId {
    pub const NONE: ObjectId = ObjectId(0);

    pub(crate) fn new() -> ObjectId {
        static OBJECT_ID: AtomicU64 = AtomicU64::new(1);
        ObjectId(OBJECT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
#[derive(TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum ObjectType {
    BlobPlain = 0,
    Directory = 1,
    Metadata = 2,
    Archive = 3,
    Root = 4,

    FilterZstd = 0x20,

    ZstdPreloadList = 0x40,
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
#[derive(TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum MetadataTag {
    ZstdPreloadList = 0x40,
    EntryArchive = 0x41,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
#[repr(u32)]
pub enum Metadata {
    VarInt(i64),
    VarUInt(u64),
    ObjectRef(ObjectId),
}

#[derive(Clone, Debug)]
pub struct ObjBlobPlain {
    pub filters: Vec<ObjectId>,
}

#[derive(Clone, Debug)]
pub struct ObjDirectory {
    pub entries: Vec<DirectoryEntry>,
}
#[derive(Clone, Debug)]
pub struct DirectoryEntry {
    pub name: String,
    pub data: ObjectId,
    pub metadata: ObjectId,
}

#[derive(Clone, Debug)]
pub struct ObjMetadata {
    pub metadata: HashMap<MetadataTag, Metadata>,
}

#[derive(Clone, Debug)]
pub struct ObjArchive {
    pub metadata: HashMap<MetadataTag, Metadata>,
}

#[derive(Clone, Debug)]
pub struct ObjRoot {
    pub main: ObjectId,
    pub alt: HashMap<String, ObjectId>,
    pub metadata: ObjectId,
}

#[derive(Clone, Debug)]
pub struct ObjFilterZstd {
    pub sources: Vec<ObjectId>,
}

#[derive(Clone, Debug)]
pub struct ObjZstdPreloadList {
    pub list: Vec<ObjectId>,
}

#[derive(Clone, Debug)]
pub enum DiarObject {
    BlobPlain(ObjBlobPlain),
    Directory(ObjDirectory),
    Metadata(ObjMetadata),
    Archive(ObjArchive),
    Root(ObjRoot),

    FilterZstd(ObjFilterZstd),

    ZstdPreloadList(ObjZstdPreloadList),
}
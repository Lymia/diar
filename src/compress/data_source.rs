use crate::errors::*;
use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ResolvedDataSource {
    Path { path: PathBuf, len_hint: u64 },
    Data { path_hint: PathBuf, data: Vec<u8> },
}
impl ResolvedDataSource {
    pub fn from_path(path: &Path) -> Result<ResolvedDataSource> {
        let path = path.to_path_buf();
        let len_hint = path.metadata()?.len();
        Ok(ResolvedDataSource::Path { path, len_hint })
    }

    pub fn len_hint(&self) -> u64 {
        match self {
            ResolvedDataSource::Path { len_hint, .. } => *len_hint,
            ResolvedDataSource::Data { data, .. } => data.len() as u64,
        }
    }

    pub fn push_to_vec(&self, vec: &mut Vec<u8>) -> Result<()> {
        match self {
            ResolvedDataSource::Path { path, .. } => {
                File::open(path)?.read_to_end(vec)?;
            }
            ResolvedDataSource::Data { data, .. } => {
                vec.extend_from_slice(data);
            }
        }
        Ok(())
    }
    pub fn write_to_stream(&self, out: &mut impl Write) -> Result<()> {
        match self {
            ResolvedDataSource::Path { path, .. } => {
                std::io::copy(&mut File::open(path)?, out)?;
            }
            ResolvedDataSource::Data { data, .. } => {
                out.write_all(data)?;
            }
        }
        Ok(())
    }
}

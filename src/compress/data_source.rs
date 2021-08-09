use crate::compress::constants::OCTET_STREAM;
use crate::errors::*;
use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

fn classify_file_mime(file: &Path) -> Cow<'static, str> {
	match tree_magic_mini::from_filepath(file) {
		Some(OCTET_STREAM) | None => match mime_guess::from_path(file).first_raw() {
			Some(v) => v.into(),
			None => match file.extension().and_then(|x| x.to_str()) {
				Some(x) => format!(".ext:{}", x).into(),
				None => OCTET_STREAM.into(),
			},
		},
		Some(v) => v.into(),
	}
}
fn classify_byte_buf(filename: &Path, buf: &[u8]) -> Cow<'static, str> {
	match tree_magic_mini::from_u8(buf) {
		OCTET_STREAM => match mime_guess::from_path(filename).first_raw() {
			Some(v) => v.into(),
			None => match filename.extension().and_then(|x| x.to_str()) {
				Some(x) => format!(".ext:{}", x).into(),
				None => OCTET_STREAM.into(),
			},
		},
		v => v.into(),
	}
}

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
	pub fn mime_type(&self) -> Cow<'static, str> {
		match self {
			ResolvedDataSource::Path { path, .. } => classify_file_mime(path),
			ResolvedDataSource::Data { path_hint, data, .. } => classify_byte_buf(path_hint, &data),
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
}

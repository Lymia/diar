use crate::{
	compress::{DictionarySet, DictionarySetBuilder},
	errors::*,
};
use jwalk::WalkDir;
use std::path::Path;

fn is_symlink(path: &Path) -> bool {
	std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

pub fn compress_dir(dir: &Path) -> Result<()> {
	Ok(())
}

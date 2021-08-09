use crate::{
	compress::{DictionarySet, DictionarySetBuilder},
	errors::*,
};
use std::path::Path;

fn is_symlink(path: &Path) -> bool {
	std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}
fn build_dicts(path: &Path, builder: &mut DictionarySetBuilder) -> Result<()> {
	if path.is_dir() {
		for path in path.read_dir()? {
			let path = path?;
			build_dicts(&path.path(), builder)?;
		}
	} else if path.is_file() {
		builder.add_file(path);
	}
	Ok(())
}

pub fn compress_dir(dir: &Path) -> Result<()> {
	let mut builder = DictionarySetBuilder::new();
	build_dicts(dir, &mut builder)?;
	let dir_set = builder.build()?;
	Ok(())
}

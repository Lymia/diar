use crate::{
	compress::{DictionarySet, DictionarySetBuilder},
	errors::*,
};
use jwalk::WalkDir;
use std::path::Path;
use zstd::dict::CDict;
use std::collections::HashMap;
use std::io::{Read, Write};
use crate::compress::writer::CompressWriter;

/*
struct LoadedDictSet {
	dicts: HashMap<>
}

fn write_blob(mut source: impl Read, target: &mut CompressWriter<impl Write>)

 */
pub fn compress_dir(dir: &Path) -> Result<()> {
	let dicts = DictionarySet::builder().add_directory(dir).unwrap().build().unwrap();

	Ok(())
}

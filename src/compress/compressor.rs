use crate::compress::data_source::ResolvedDataSource;
use crate::compress::dictionary_set::{DictionarySetBuilder, LoadedDictionarySet};
use crate::compress::dir_tree::{DirNode, DirNodeData};
use crate::compress::writer::CompressWriter;
use crate::errors::*;
use std::io::Write;
use std::path::{Path, PathBuf};
use crate::names::KnownName;

fn write_file(
	target: &mut CompressWriter<impl Write>,
	contents: &ResolvedDataSource,
	mime_type: &str,
	dicts: &LoadedDictionarySet<'_>,
) -> Result<()> {
	target.write_known_name(KnownName::CompressionZstd)?;
	target.write_varuint(1)?;
	target.write_known_name(KnownName::ZstdDictionary)?;
	target.write_name(&mime_type.into())?;

	let mut zstd = target.compress_stream(mime_type, dicts)?;
	contents.write_to_stream(&mut zstd)?;
	zstd.finish()?;

	Ok(())
}
fn write_dir(
	target: &mut CompressWriter<impl Write>,
	node: &DirNode,
	dicts: &LoadedDictionarySet<'_>,
) -> Result<()> {
	match &node.data {
		DirNodeData::FileNode { contents, mime_type, .. } => {
			write_file(target, contents, mime_type, dicts)?;
		}
		DirNodeData::DirNode { contents, .. } => {
			for node in contents.values() {
				write_dir(target, node, dicts)?;
			}
		}
	}
	Ok(())
}

pub fn compress(dir: &Path, mut target: impl Write) -> Result<()> {
	let nodes = DirNode::from_path(dir)?;
	let dicts = DictionarySetBuilder::new().add_nodes(&nodes)?.build()?;
	let loaded = dicts.load(12);

	// test
	std::fs::create_dir(PathBuf::from("dict")).unwrap();
	for (k, v) in dicts.iter_dicts() {
		std::fs::write(format!("dict/{}.dict", k.replace("/", ":")), v)?;
	}

	write_dir(&mut CompressWriter::new(&mut target), &nodes, &loaded)?;
	Ok(())
}

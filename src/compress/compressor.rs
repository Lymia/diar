use crate::compress::data_source::ResolvedDataSource;
use crate::compress::dictionary_sample_builder::{BuildSamples, BuildSamplesConfiguration};
use crate::compress::dir_tree::{DirNode, DirNodeData};
use crate::compress::writer::CompressWriter;
use crate::errors::*;
use crate::names::KnownName;
use std::io::Write;
use std::path::{Path, PathBuf};
use zstd::dict::EncoderDictionary;

fn write_file(
    target: &mut CompressWriter<impl Write>,
    contents: &ResolvedDataSource,
    dict: &EncoderDictionary,
) -> Result<()> {
    target.write_known_name(KnownName::CompressionZstd)?;
    target.write_varuint(1)?;
    target.write_known_name(KnownName::ZstdDictionary)?;

    let mut zstd = target.compress_stream(dict)?;
    contents.write_to_stream(&mut zstd)?;
    zstd.finish()?;

    Ok(())
}
fn write_dir(
    target: &mut CompressWriter<impl Write>,
    node: &DirNode,
    dict: &EncoderDictionary,
) -> Result<()> {
    match &node.data {
        DirNodeData::FileNode { contents, .. } => {
            write_file(target, contents, dict)?;
        }
        DirNodeData::DirNode { contents, .. } => {
            for node in contents.values() {
                write_dir(target, node, dict)?;
            }
        }
    }
    Ok(())
}

pub fn compress(dir: &Path, mut target: impl Write) -> Result<()> {
    let nodes = DirNode::from_path(dir)?;

    // test
    if !PathBuf::from("dict").exists() {
        std::fs::create_dir(PathBuf::from("dict"))?;
    }
    let mut samples = BuildSamples::new(&BuildSamplesConfiguration::default());

    trace!("Building samples...");
    samples.add_nodes(&nodes)?;
    trace!("Building dictionary...");
    let data = samples.build_dictionary(1024 * 512)?;
    std::fs::write("dict/test_dict.dict", &data)?;
    trace!("Compressing data...");
    let dict = EncoderDictionary::new(&data, 12);
    write_dir(&mut CompressWriter::new(&mut target), &nodes, &dict)?;
    trace!("Done!");

    Ok(())
}

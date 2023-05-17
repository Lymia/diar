use crate::{
    compress::{
        data_source::ResolvedDataSource,
        dictionary_sample_builder::{BuildSamples, BuildSamplesConfiguration},
        dir_tree::{DirNode, DirNodeData},
        writer::{DiarWriter, ObjectId},
    },
    errors::*,
    names::KnownName,
};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use zstd::{dict::EncoderDictionary, zstd_safe::CompressionLevel};

const LEVEL: CompressionLevel = 6;

fn write_file(
    target: &mut DiarWriter<impl Write>,
    contents: &ResolvedDataSource,
    dict_obj: ObjectId,
    dict: &EncoderDictionary,
) -> Result<ObjectId> {
    target.write_compressed_blob(LEVEL, Some((dict_obj, dict)), |x| {
        contents.write_to_stream(x)?;
        Ok(())
    })
}
fn write_dir(
    target: &mut DiarWriter<impl Write>,
    node: &DirNode,
    dict_obj: ObjectId,
    dict: &EncoderDictionary,
) -> Result<ObjectId> {
    match &node.data {
        DirNodeData::FileNode { contents, .. } => write_file(target, contents, dict_obj, dict),
        DirNodeData::DirNode { contents, .. } => {
            let mut entries = Vec::new();
            for (name, node) in contents {
                let id = write_dir(target, node, dict_obj, dict)?;
                entries.push((name, id));
            }

            let mut dir = target.start_write_directory()?;
            for (name, node) in entries {
                dir.write_entry(name, node, None)?;
            }
            dir.finish()
        }
    }
}

pub fn compress(dir: &Path, mut target: impl Write) -> Result<()> {
    let nodes = DirNode::from_path(dir)?;
    let mut writer = DiarWriter::new(&mut target)?;

    // test
    if !PathBuf::from("dict").exists() {
        std::fs::create_dir(PathBuf::from("dict"))?;
    }

    trace!("Building samples...");
    let mut samples = BuildSamples::new(&BuildSamplesConfiguration::default());
    samples.add_nodes(&nodes)?;

    trace!("Building dictionary...");
    let data = samples.build_dictionary()?;
    std::fs::write("dict/test_dict.dict", &data)?;
    let dict_obj = writer.write_dictionary(&data)?;

    trace!("Compressing data...");
    let dict = EncoderDictionary::new(&data, LEVEL);
    let root_obj = write_dir(&mut writer, &nodes, dict_obj, &dict)?;
    trace!(" - Done!");

    trace!("Finishing archive...");
    writer.finish(root_obj)?;

    Ok(())
}

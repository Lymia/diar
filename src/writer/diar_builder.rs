use crate::{
    errors::*,
    names::KnownName,
    object_io::DiarIo,
    objects::*,
    writer::{
        dict_builder::{BuildSamples, BuildSamplesConfiguration},
        dir_tree::{DataSource, DirNode, DirNodeData},
    },
};
use std::{
    collections::HashMap,
    io::{Seek, Write},
    path::{Path, PathBuf},
};
use zstd::{
    dict::EncoderDictionary,
    zstd_safe::{CParameter, CompressionLevel},
    Encoder,
};

const LEVEL: CompressionLevel = 6;

fn write_compressed_blob<S: Write + Seek>(
    target: &mut DiarIo<&mut S>,
    dict: Option<&EncoderDictionary>,
    zstd_filter_id: ObjectId,
    callback: impl FnOnce(&mut Encoder<&mut &mut S>) -> Result<()>,
) -> Result<ObjectId> {
    target.write_object_with_data(
        &DiarObject::BlobPlain(ObjBlobPlain { filters: vec![zstd_filter_id] }),
        |x| {
            let mut zstd = match dict {
                None => Encoder::new(x, LEVEL)?,
                Some(dict) => Encoder::with_prepared_dictionary(x, dict)?,
            };
            zstd.set_parameter(CParameter::CompressionLevel(LEVEL))?;
            zstd.set_parameter(CParameter::WindowLog(30))?;
            zstd.set_parameter(CParameter::HashLog(30))?;
            zstd.set_parameter(CParameter::EnableDedicatedDictSearch(true))?;
            callback(&mut zstd)?;
            zstd.finish()?;
            Ok(())
        },
    )
}

fn write_file(
    target: &mut DiarIo<&mut (impl Write + Seek)>,
    contents: &DataSource,
    filter_obj: ObjectId,
    dict: &EncoderDictionary,
) -> Result<ObjectId> {
    write_compressed_blob(target, Some(dict), filter_obj, |x| {
        contents.write_to_stream(x)?;
        Ok(())
    })
}

fn write_dir(
    target: &mut DiarIo<&mut (impl Write + Seek)>,
    node: &DirNode,
    filter_obj: ObjectId,
    dict: &EncoderDictionary,
) -> Result<ObjectId> {
    match &node.data {
        DirNodeData::FileNode { contents, .. } => write_file(target, contents, filter_obj, dict),
        DirNodeData::DirNode { contents, .. } => {
            let mut entries = Vec::new();
            for (name, node) in contents {
                let id = write_dir(target, node, filter_obj, dict)?;
                entries.push(DirectoryEntry {
                    name: name.to_string(),
                    data: id,
                    metadata: ObjectId::NONE,
                });
            }

            target.write_object(&DiarObject::Directory(ObjDirectory { entries }))
        }
    }
}

pub fn compress(dir: &Path, mut target: impl Write + Seek) -> Result<()> {
    let nodes = DirNode::from_path(dir)?;
    let mut writer = DiarIo::create(&mut target)?;

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

    trace!("Writing dictionary object...");
    let plain_zstd_filter =
        writer.write_object(&DiarObject::FilterZstd(ObjFilterZstd { dict_sources: vec![] }))?;
    let dict_data = write_compressed_blob(&mut writer, None, plain_zstd_filter, |x| {
        x.write_all(&data)?;
        Ok(())
    })?;
    let dict_obj = writer
        .write_object(&DiarObject::FilterZstd(ObjFilterZstd { dict_sources: vec![dict_data] }))?;

    trace!("Compressing data...");
    let dict = EncoderDictionary::new(&data, LEVEL);
    let root_obj = write_dir(&mut writer, &nodes, dict_obj, &dict)?;
    trace!(" - Done!");

    trace!("Finishing archive...");
    let archive_obj = writer.write_object(&DiarObject::Archive(ObjArchive {
        root: root_obj,
        metadata: Default::default(),
    }))?;
    let root_obj = writer.write_object(&DiarObject::Root(ObjRoot {
        main: archive_obj,
        alt: Default::default(),
        metadata: Default::default(),
    }))?;

    writer.finish(root_obj)?;

    Ok(())
}

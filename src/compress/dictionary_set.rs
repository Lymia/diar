use crate::errors::*;

use crate::compress::constants::OCTET_STREAM;
use crate::compress::data_source::ResolvedDataSource;
use crate::compress::dir_tree::DirNodeData;
use crate::compress::DirNode;
use derive_setters::Setters;
use rayon::prelude::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

// TODO: Add a way to speed up dictionary construction by taking a subset of files only.

fn read_path(path: &Path, target: &mut Vec<u8>) -> Result<()> {
	File::open(path)?.read_to_end(target)?;
	Ok(())
}
fn dictionary_from_files(sources: &[&ResolvedDataSource], max_size: usize) -> Result<Vec<u8>> {
	let mut linear = Vec::new();
	let mut sizes = Vec::new();
	for source in sources {
		let len = linear.len();
		if let Err(e) = source.push_to_vec(&mut linear) {
			warn!("Skipping file due to error: {:?}", e);
		}
		sizes.push(linear.len() - len);
	}
	trace!(" - Creating dictionary from {} bytes and {} files", linear.len(), sizes.len());
	Ok(zstd::dict::from_continuous(&linear, &sizes, max_size)?)
}

/// A builder for [`DictionarySet`]s.
#[derive(Setters)]
pub struct DictionarySetBuilder<'a> {
	#[setters(skip)]
	sources: HashMap<Cow<'static, str>, Vec<&'a ResolvedDataSource>>,

	/// Sets the maximum size of every dictionary.
	///
	/// Note that a dictionary is generated for every MIME type present in the source files, hence
	/// the total sizes of all dictionaries in the source data may be significantly larger.
	max_dictionary_size: usize,
}
impl<'a> DictionarySetBuilder<'a> {
	/// Creates a new builder.
	pub fn new() -> Self {
		DictionarySetBuilder {
			sources: HashMap::new(),
			max_dictionary_size: 1024 * 512, // 512 KiB
		}
	}

	/// Adds all files in a directory to the builder.
	pub fn add_nodes(&mut self, node: &'a DirNode) -> Result<&mut Self> {
		match &node.data {
			DirNodeData::FileNode { contents, mime_type, .. } => {
				self.sources.entry(mime_type.clone()).or_default().push(contents);
			}
			DirNodeData::DirNode { contents, .. } => {
				for node in contents.values() {
					self.add_nodes(node)?;
				}
			}
		}
		Ok(self)
	}

	/// Builds dictionaries from the provided files.
	pub fn build(&mut self) -> Result<DictionarySet> {
		enum MimeResult<'a> {
			Dictionary(Cow<'static, str>, Vec<u8>),
			Failed(Vec<&'a ResolvedDataSource>),
		}
		let r: Vec<MimeResult<'a>> = std::mem::take(&mut self.sources)
			.into_par_iter()
			.map(|x| {
				let (mime, files) = x;
				let span = trace_span!("dict", mime = mime.as_ref());
				let _guard = span.enter();

				trace!("Creating dictionary for MIME: {}", mime);
				if mime == OCTET_STREAM {
					MimeResult::Failed(files)
				} else {
					match dictionary_from_files(&files, self.max_dictionary_size) {
						Ok(dict) => MimeResult::Dictionary(mime, dict),
						Err(e) => {
							trace!(" - Failed, adding to application/octet-stream dictionary");
							trace!(" - Cause: {:?}", e);
							MimeResult::Failed(files)
						}
					}
				}
			})
			.collect();

		let mut set = DictionarySet { mime_dictionaries: Default::default() };
		let mut octet_stream = Vec::new();
		for res in r {
			match res {
				MimeResult::Dictionary(mime, dict) => {
					set.mime_dictionaries.insert(mime, dict);
				}
				MimeResult::Failed(files) => octet_stream.extend(files),
			}
		}

		trace!("Creating dictionary for MIME: application/octet-stream");
		match dictionary_from_files(&octet_stream, self.max_dictionary_size) {
			Ok(dict) => {
				set.mime_dictionaries.insert(OCTET_STREAM.into(), dict);
			}
			Err(e) => {
				trace!(" - Failed to create fallback dictionary: {:?}", e);
			}
		}
		std::mem::drop(octet_stream);

		trace!("Dictionaries completed!");
		for (k, v) in &set.mime_dictionaries {
			trace!(" - Dictionary for {}: {} bytes", k, v.len());
		}

		Ok(set)
	}
}

/// A dictionary set which may be used to compress an `diar` archive.
pub struct DictionarySet {
	/// The dictionary for each MIME type.
	pub(crate) mime_dictionaries: HashMap<Cow<'static, str>, Vec<u8>>,
}
impl DictionarySet {
	/// Creates a builder for a dictionary set.
	pub fn builder<'a>() -> DictionarySetBuilder<'a> {
		DictionarySetBuilder::new()
	}
}

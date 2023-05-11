use crate::errors::*;

use crate::compress::constants::OCTET_STREAM;
use crate::compress::data_source::ResolvedDataSource;
use crate::compress::dir_tree::DirNodeData;
use crate::compress::DirNode;
use derive_setters::Setters;
use rand::prelude::*;
use rand_pcg::Lcg64Xsh32;
use rayon::prelude::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, ErrorKind};
use std::path::Path;
use zstd::dict::EncoderDictionary;
use std::ffi::CStr;

// TODO: Add a way to speed up dictionary construction by taking a subset of files only.

fn read_path(path: &Path, target: &mut Vec<u8>) -> Result<()> {
	File::open(path)?.read_to_end(target)?;
	Ok(())
}
fn train_from_continuous(linear: &[u8], sizes: &[usize], max_size: usize) -> Result<Vec<u8>> {
	unsafe {
		let mut data: Vec<u8> = Vec::with_capacity(max_size);
		data.set_len(max_size);

		assert_eq!(sizes.iter().sum::<usize>(), linear.len());
		let result = zstd_sys::ZDICT_trainFromBuffer_fastCover(
			data.as_mut_ptr() as *mut _,
			data.len(),
			linear.as_ptr() as *const _,
			sizes.as_ptr(),
			sizes.len() as u32,
			zstd_sys::ZDICT_fastCover_params_t {
				k: 16,
				d: 8,
				f: 26,
				steps: 0,
				nbThreads: num_cpus::get() as u32,
				splitPoint: 0.85,
				accel: 1,
				shrinkDict: 1,
				shrinkDictMaxRegression: 5,
				zParams: zstd_sys::ZDICT_params_t {
					compressionLevel: 0, // TODO Figure out how to thread this through.
					notificationLevel: 0,
					dictID: 0
				}
			}
		);
		if zstd_sys::ZSTD_isError(result) != 0 {
			let cstr = CStr::from_ptr(zstd_sys::ZSTD_getErrorName(result));
			let err = cstr.to_str().unwrap();
			Err(std::io::Error::new(ErrorKind::Other, err).into())
		} else {
			data.set_len(result);
			data.shrink_to_fit();
			Ok(data)
		}
	}
}
fn dictionary_from_files(sources: &[&ResolvedDataSource], max_size: usize) -> Result<Vec<u8>> {
	let mut linear = Vec::new();
	let mut sizes = Vec::new();

	for source in sources.iter() {
		let len = linear.len();
		if let Err(e) = source.push_to_vec(&mut linear) {
			warn!("Skipping file due to error: {:?}", e);
		}
		sizes.push(linear.len() - len);
	}
	trace!(" - Creating dictionary from {} bytes and {} files", linear.len(), sizes.len());
	Ok(train_from_continuous(&linear, &sizes, max_size)?)
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
			max_dictionary_size: 1024 * 128, // 128 KiB
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
	mime_dictionaries: HashMap<Cow<'static, str>, Vec<u8>>,
}
impl DictionarySet {
	/// Creates a builder for a dictionary set.
	pub fn builder<'a>() -> DictionarySetBuilder<'a> {
		DictionarySetBuilder::new()
	}

	pub fn iter_dicts(&self) -> impl Iterator<Item = (&str, &[u8])> {
		self.mime_dictionaries.iter().map(|(k, v)| (k.as_ref(), v.as_slice()))
	}

	pub(crate) fn load(&self, level: i32) -> LoadedDictionarySet<'_> {
		let mut dictionaries = HashMap::new();
		for (k, v) in &self.mime_dictionaries {
			dictionaries.insert(k.as_ref(), EncoderDictionary::new(v.as_slice(), level));
		}
		LoadedDictionarySet { level, mime_dictionaries: dictionaries }
	}
}

/// A dictionary loaded and prepared for use by the archiver.
pub struct LoadedDictionarySet<'a> {
	pub level: i32,
	mime_dictionaries: HashMap<&'a str, EncoderDictionary<'a>>,
}
impl<'a> LoadedDictionarySet<'a> {
	pub fn get_for_mime(&self, mime: &str) -> Option<&EncoderDictionary<'a>> {
		self.mime_dictionaries.get(mime).or_else(|| self.mime_dictionaries.get(OCTET_STREAM))
	}
}

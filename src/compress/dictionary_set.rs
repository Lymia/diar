use crate::errors::*;

use std::{
	borrow::Cow,
	collections::HashMap,
	fs::File,
	io::Read,
	path::{Path, PathBuf},
};

const OCTET_STREAM: &'static str = "application/octet-stream";

fn classify_file_mime(file: &Path) -> &'static str {
	match tree_magic_mini::from_filepath(file) {
		Some(v) => v,
		None => match mime_guess::from_path(file).first_raw() {
			Some(v) => v,
			None => OCTET_STREAM,
		},
	}
}
fn classify_byte_buf(filename: &Path, buf: &[u8]) -> &'static str {
	match tree_magic_mini::from_u8(buf) {
		OCTET_STREAM => match mime_guess::from_path(filename).first_raw() {
			Some(v) => v,
			None => OCTET_STREAM,
		},
		v => v,
	}
}

enum BuilderSource<'a> {
	File(PathBuf),
	Data(PathBuf, Cow<'a, [u8]>),
}
impl<'a> BuilderSource<'a> {
	fn path(&self) -> &Path {
		match self {
			BuilderSource::File(path) => path,
			BuilderSource::Data(path, _) => path,
		}
	}
	fn classify(&self) -> &'static str {
		match self {
			BuilderSource::File(path) => classify_file_mime(path),
			BuilderSource::Data(path, data) => classify_byte_buf(path, data),
		}
	}
}

// TODO: Customizable dictionary max sizes.

#[derive(Default)]
struct DictionaryBuilder {
	linear: Vec<u8>,
	sizes: Vec<usize>,
}
impl DictionaryBuilder {
	fn build_from_files(&mut self, sources: &[BuilderSource<'_>]) -> Result<Vec<u8>> {
		self.linear.clear();
		self.sizes.clear();
		for file in sources {
			match file {
				BuilderSource::File(path) => {
					let len = self.linear.len();
					File::open(path)?.read_to_end(&mut self.linear)?;
					let size = self.linear.len() - len;
					self.sizes.push(size);
				}
				BuilderSource::Data(_, data) => {
					self.linear.extend_from_slice(&data);
					self.sizes.push(data.len());
				}
			}
		}
		trace!(
			" - Creating dictionary from {} bytes and {} files",
			self.linear.len(),
			self.sizes.len(),
		);
		Ok(zstd::dict::from_continuous(&self.linear, &self.sizes, 1024 * 1024)?)
	}
}

/// A builder for [`DictionarySet`]s.
pub struct DictionarySetBuilder<'a> {
	sources: HashMap<&'static str, Vec<BuilderSource<'a>>>,
}
impl<'a> DictionarySetBuilder<'a> {
	/// Creates a new builder.
	pub fn new() -> Self {
		DictionarySetBuilder { sources: HashMap::new() }
	}

	fn add_source(&mut self, source: BuilderSource<'a>) {
		let mime = source.classify();
		self.sources.entry(mime).or_default().push(source);
	}
	pub fn add_file(&mut self, path: impl AsRef<Path>) {
		self.add_source(BuilderSource::File(path.as_ref().to_path_buf()));
	}
	pub fn add_data(&mut self, path: impl AsRef<Path>, data: impl Into<Cow<'a, [u8]>>) {
		self.add_source(BuilderSource::Data(path.as_ref().to_path_buf(), data.into()))
	}

	pub fn build(self) -> Result<DictionarySet> {
		let mut set = DictionarySet { mime_dictionaries: Default::default() };
		let mut builder = DictionaryBuilder::default();
		let mut octet_stream = Vec::new();

		for (k, files) in self.sources {
			trace!("Creating dictionary for MIME: {}", k);
			if k == OCTET_STREAM {
				octet_stream.extend(files);
			} else {
				match builder.build_from_files(&files) {
					Ok(dict) => {
						set.mime_dictionaries.insert(k, dict);
					},
					Err(e) => {
						trace!(" - Failed, falling back to application/octet-stream dictionary");
						trace!(" - Cause: {:?}", e);
						octet_stream.extend(files);
					}
				};
			}
		}

		trace!("Creating dictionary for MIME: application/octet-stream");
		match builder.build_from_files(&octet_stream) {
			Ok(dict) => {
				set.mime_dictionaries.insert(OCTET_STREAM, dict);
			},
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
	mime_dictionaries: HashMap<&'static str, Vec<u8>>,
}
impl DictionarySet {
	/// Creates a builder for a dictionary set.
	pub fn builder<'a>() -> DictionarySetBuilder<'a> {
		DictionarySetBuilder::new()
	}
}

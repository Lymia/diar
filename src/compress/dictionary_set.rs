use crate::errors::*;

use std::{
	borrow::Cow,
	collections::HashMap,
	fs::File,
	io::Read,
	path::{Path, PathBuf},
};

fn classify_file_mime(file: &Path) -> &'static str {
	match tree_magic_mini::from_filepath(file) {
		Some(v) => v,
		None => match mime_guess::from_path(file).first_raw() {
			Some(v) => v,
			None => "application/octet-stream",
		},
	}
}
fn classify_byte_buf(filename: &Path, buf: &[u8]) -> &'static str {
	match tree_magic_mini::from_u8(buf) {
		"application/octet-stream" => match mime_guess::from_path(filename).first_raw() {
			Some(v) => v,
			None => "application/octet-stream",
		},
		v => v,
	}
}

enum BuilderSource<'a> {
	File(PathBuf),
	Data(PathBuf, Cow<'a, [u8]>),
}
impl<'a> BuilderSource<'a> {
	fn classify(&self) -> &'static str {
		match self {
			BuilderSource::File(path) => classify_file_mime(path),
			BuilderSource::Data(path, data) => classify_byte_buf(path, data),
		}
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
		let mut set =
			DictionarySet { meta_dictionary: vec![], mime_dictionaries: Default::default() };
		let mut linear = Vec::new();
		let mut sizes = Vec::new();

		for (k, files) in self.sources {
			for file in files {
				match file {
					BuilderSource::File(path) => {
						let len = linear.len();
						File::open(path)?.read_to_end(&mut linear)?;
						sizes.push(linear.len() - len);
					}
					BuilderSource::Data(_, data) => {
						linear.extend_from_slice(&data);
						sizes.push(data.len());
					}
				}
			}

			let dict = zstd::dict::from_continuous(&linear, &sizes, 1024 * 1024)?; // TODO Customizable size
			set.mime_dictionaries.insert(k, dict);

			linear.clear();
			sizes.clear();
		}

		for (_, dict) in &set.mime_dictionaries {
			linear.extend_from_slice(&dict);
			sizes.push(dict.len());
		}
		set.meta_dictionary = zstd::dict::from_continuous(&linear, &sizes, 1024 * 1024)?; // TODO Customizable size

		Ok(set)
	}
}

/// A dictionary set which may be used to compress an `diar` archive.
pub struct DictionarySet {
	/// The "meta" dictionary which is used to compress the other dictionaries.
	meta_dictionary: Vec<u8>,
	/// The dictionary for each MIME type.
	mime_dictionaries: HashMap<&'static str, Vec<u8>>,
}
impl DictionarySet {
	/// Creates a builder for a dictionary set.
	pub fn builder<'a>() -> DictionarySetBuilder<'a> {
		DictionarySetBuilder::new()
	}
}

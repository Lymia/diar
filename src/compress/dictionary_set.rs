use crate::errors::*;

use jwalk::WalkDir;
use rayon::prelude::*;
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

fn read_path(path: &Path, target: &mut Vec<u8>) -> Result<()> {
	File::open(path)?.read_to_end(target)?;
	Ok(())
}
fn dictionary_from_files(sources: &[BuilderSource<'_>]) -> Result<Vec<u8>> {
	let mut linear = Vec::new();
	let mut sizes = Vec::new();
	for file in sources {
		match file {
			BuilderSource::File(path) => {
				let len = linear.len();
				if let Err(e) = read_path(path, &mut linear) {
					warn!("Skipping file {} due to error: {:?}", path.display(), e);
				}
				sizes.push(linear.len() - len);
			}
			BuilderSource::Data(_, data) => {
				linear.extend_from_slice(&data);
				sizes.push(data.len());
			}
		}
	}
	trace!(" - Creating dictionary from {} bytes and {} files", linear.len(), sizes.len());
	Ok(zstd::dict::from_continuous(&linear, &sizes, 1024 * 1024)?)
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

	/// Adds a new file data source to the builder.
	///
	/// If the given path is not a file, it is ignored.
	pub fn add_file(&mut self, path: impl AsRef<Path>) -> &mut Self {
		let path = path.as_ref();
		if path.is_file() {
			self.add_source(BuilderSource::File(path.to_path_buf()));
		}
		self
	}

	/// Adds a new data source to the builder.
	pub fn add_data(
		&mut self,
		path: impl AsRef<Path>,
		data: impl Into<Cow<'a, [u8]>>,
	) -> &mut Self {
		self.add_source(BuilderSource::Data(path.as_ref().to_path_buf(), data.into()));
		self
	}

	/// Adds all files in a directory to the builder.
	pub fn add_directory(&mut self, path: impl AsRef<Path>) -> Result<&mut Self> {
		trace!("Walking directory {}...", path.as_ref().display());
		let r: jwalk::Result<Vec<_>> = WalkDir::new(path.as_ref())
			.skip_hidden(false)
			.follow_links(true)
			.into_iter()
			.par_bridge()
			.filter_map(|r| match r {
				Ok(x) => {
					let path = x.path();
					if !path.is_file() {
						None
					} else {
						let mime = classify_file_mime(&path);
						Some(Ok((mime, BuilderSource::File(x.path()))))
					}
				}
				Err(e) => Some(Err(e)),
			})
			.collect();
		for (mime, source) in r? {
			self.sources.entry(mime).or_default().push(source);
		}
		Ok(self)
	}


	pub fn build(&mut self) -> Result<DictionarySet> {
		enum MimeResult<'a> {
			Dictionary(&'static str, Vec<u8>),
			Failed(Vec<BuilderSource<'a>>),
		}
		let r: Vec<MimeResult<'a>> = std::mem::take(&mut self.sources).into_par_iter()
			.map(|x| {
				let (mime, files) = x;
				let span = trace_span!("dict", mime);
				let _guard = span.enter();

				trace!("Creating dictionary for MIME: {}", mime);
				if mime == OCTET_STREAM {
					MimeResult::Failed(files)
				} else {
					match dictionary_from_files(&files) {
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
				},
				MimeResult::Failed(files) => octet_stream.extend(files),
			}
		}

		trace!("Creating dictionary for MIME: application/octet-stream");
		match dictionary_from_files(&octet_stream) {
			Ok(dict) => {
				set.mime_dictionaries.insert(OCTET_STREAM, dict);
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
	mime_dictionaries: HashMap<&'static str, Vec<u8>>,
}
impl DictionarySet {
	/// Creates a builder for a dictionary set.
	pub fn builder<'a>() -> DictionarySetBuilder<'a> {
		DictionarySetBuilder::new()
	}
}

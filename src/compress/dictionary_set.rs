use crate::errors::*;

use derive_setters::Setters;
use jwalk::WalkDir;
use rayon::prelude::*;
use std::{
	borrow::Cow,
	collections::HashMap,
	fs::File,
	io::Read,
	path::{Path, PathBuf},
};

// TODO: Add a way to speed up dictionary construction by taking a subset of files only.

const OCTET_STREAM: &'static str = "application/octet-stream";

fn classify_file_mime(file: &Path) -> Cow<'static, str> {
	match tree_magic_mini::from_filepath(file) {
		Some(OCTET_STREAM) | None => match mime_guess::from_path(file).first_raw() {
			Some(v) => v.into(),
			None => match file.extension().and_then(|x| x.to_str()) {
				Some(x) => format!(".ext:{}", x).into(),
				None => OCTET_STREAM.into(),
			}
		},
		Some(v) => v.into(),
	}
}
fn classify_byte_buf(filename: &Path, buf: &[u8]) -> Cow<'static, str> {
	match tree_magic_mini::from_u8(buf) {
		OCTET_STREAM => match mime_guess::from_path(filename).first_raw() {
			Some(v) => v.into(),
			None => match filename.extension().and_then(|x| x.to_str()) {
				Some(x) => format!(".ext:{}", x).into(),
				None => OCTET_STREAM.into(),
			},
		},
		v => v.into(),
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
	fn classify(&self) -> Cow<'static, str> {
		match self {
			BuilderSource::File(path) => classify_file_mime(path),
			BuilderSource::Data(path, data) => classify_byte_buf(path, data),
		}
	}
}

fn read_path(path: &Path, target: &mut Vec<u8>) -> Result<()> {
	File::open(path)?.read_to_end(target)?;
	Ok(())
}
fn dictionary_from_files(sources: &[BuilderSource<'_>], max_size: usize) -> Result<Vec<u8>> {
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
	Ok(zstd::dict::from_continuous(&linear, &sizes, max_size)?)
}

/// A builder for [`DictionarySet`]s.
#[derive(Setters)]
pub struct DictionarySetBuilder<'a> {
	#[setters(skip)]
	sources: HashMap<Cow<'static, str>, Vec<BuilderSource<'a>>>,

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
		let r: jwalk::Result<Vec<jwalk::DirEntry<((), ())>>> = WalkDir::new(path.as_ref())
			.skip_hidden(false)
			.follow_links(false)
			.into_iter()
			.collect();
		let r: Vec<_> = r?
			.into_par_iter()
			.filter_map(|x| {
				let path = x.path();
				if !path.is_file() {
					None
				} else {
					let mime = classify_file_mime(&path);
					Some((mime, BuilderSource::File(x.path())))
				}
			})
			.collect();
		for (mime, source) in r {
			self.sources.entry(mime).or_default().push(source);
		}
		Ok(self)
	}

	/// Builds dictionaries from the provided files.
	pub fn build(&mut self) -> Result<DictionarySet> {
		enum MimeResult<'a> {
			Dictionary(Cow<'static, str>, Vec<u8>),
			Failed(Vec<BuilderSource<'a>>),
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

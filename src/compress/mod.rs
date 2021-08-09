mod compressor;
mod data_source;
mod dictionary_set;
mod dir_tree;
mod writer;

pub use compressor::compress_dir;
pub use dictionary_set::{DictionarySet, DictionarySetBuilder};
pub use dir_tree::DirNode;

// Misc constants
mod constants {
	pub const OCTET_STREAM: &'static str = "application/octet-stream";
}

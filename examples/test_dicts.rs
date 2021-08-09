use std::path::PathBuf;

fn main() {
	tracing_subscriber::fmt::init();
	diar::compress::DictionarySet::builder()
		.add_directory(&PathBuf::from("/home/lymia/Downloads/linux-5.13.9"))
		.unwrap()
		.build()
		.unwrap();
}

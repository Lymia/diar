use std::fs::File;
use std::path::PathBuf;

fn main() {
	tracing_subscriber::fmt::init();
	let path = PathBuf::from("/home/lymia/Downloads/linux-5.13.9");
	diar::compress::compress(&path, File::create("linux-5.13.9.diar").unwrap()).unwrap();
}

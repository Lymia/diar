use std::path::PathBuf;

fn main() {
	tracing_subscriber::fmt::init();
	diar::compress::compress_dir(&PathBuf::from("/home/lymia/Downloads/linux-5.13.9")).unwrap();
}

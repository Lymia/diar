use std::path::PathBuf;

fn main() {
	tracing_subscriber::fmt::init();
	let path = PathBuf::from("/home/lymia/Downloads/linux-5.13.9");
	let nodes = diar::compress::DirNode::from_path(&path).unwrap();
	diar::compress::DictionarySet::builder().add_nodes(&nodes).unwrap().build().unwrap();
}

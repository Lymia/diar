use std::fs::File;
use std::path::PathBuf;

fn main() {
    tracing_subscriber::fmt::init();
    let path = PathBuf::from("linux-6.3.2");
    diar::compress::compress(&path, File::create("linux-6.3.2.diar").unwrap()).unwrap();
}

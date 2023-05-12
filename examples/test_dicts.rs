use std::fs::File;
use std::path::PathBuf;

fn main() {
    tracing_subscriber::fmt::init();
    let path = PathBuf::from("pkmn_test");
    diar::compress::compress(&path, File::create("pkmn_test.diar").unwrap()).unwrap();
}

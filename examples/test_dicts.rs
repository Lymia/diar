use std::{
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};
use zstd::zstd_safe::{CParameter, DParameter};

fn main() {
    tracing_subscriber::fmt::init();
    let path = PathBuf::from("./pkmn_test");
    diar::compress::compress(&path, File::create("pkmn_test.diar").unwrap()).unwrap();

    /*  */
}

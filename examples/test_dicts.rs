use std::{
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};
use zstd::zstd_safe::{CParameter, DParameter};

fn main() {
    tracing_subscriber::fmt::init();
    let path = PathBuf::from("./linux-6.3.2");
    diar::writer::compress(&path, File::create("linux-6.3.2.diar").unwrap()).unwrap();

    /*  */
}

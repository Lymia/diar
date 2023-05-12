use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use zstd::zstd_safe::{CParameter, DParameter};

fn main() {
    tracing_subscriber::fmt::init();
    /*let path = PathBuf::from("pkmn_test");
    diar::compress::compress(&path, File::create("pkmn_test.diar").unwrap()).unwrap();*/

    let mut files = Vec::new();
    for file in std::fs::read_dir("pkmn_test").unwrap() {
        let file = file.unwrap();
        let path = file.path();

        let mut hash = diar::compress::content_hash::ContentHash::calculate(BufReader::new(
            File::open(&path).unwrap(),
        ))
        .unwrap();
        println!("{} {}", hash, path.display());

        files.push((hash, path.to_path_buf()));
    }

    for (hash, file) in &files {
        println!("=== {} ===", file.display());

        let mut data = Vec::new();
        for (hash_2, file_2) in &files {
            data.push((hash.distance(&hash_2), file_2.clone()));
        }
        data.sort_by_key(|x| x.0);
        for (distance, file_2) in data {
            println!("{:4} {}", distance, file_2.display());
        }

        println!()
    }

    /*let mut out = zstd::Encoder::with_dictionary(
        File::create("test_file.bin").unwrap(),
        12,
        &std::fs::read("pkmn_test/Pokemon - Diamond Version (USA) (Rev 5).nds").unwrap(),
    )
    .unwrap();
    out.set_parameter(CParameter::EnableLongDistanceMatching(true))
        .unwrap();
    out.set_parameter(CParameter::EnableDedicatedDictSearch(true))
        .unwrap();
    out.write_all(&std::fs::read("pkmn_test/Pokemon - Edicion Perla (Spain) (Rev 5).nds").unwrap())
        .unwrap();
    out.finish().unwrap();

    let mut out = zstd::Decoder::with_dictionary(
        BufReader::new(File::open("test_file.bin").unwrap()),
        &std::fs::read("pkmn_test/Pokemon - Diamond Version (USA) (Rev 5).nds").unwrap(),
    )
    .unwrap();
    std::io::copy(&mut out, &mut File::create("test_file_out.bin").unwrap()).unwrap();*/
}

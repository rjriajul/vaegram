use std::{env, fs};

#[path = "compiler/mod.rs"]
mod compiler;

fn main() {
    println!("cargo:rerun-if-changed=compiler");
    let api = fs::read_to_string("compiler/source/api.tl").unwrap();
    let mtproto = fs::read_to_string("compiler/source/mtproto.tl").unwrap();
    let out = format!("{}/raw.rs", env::var("OUT_DIR").unwrap());
    fs::write(out, compiler::generate(&api, &mtproto)).unwrap();
}

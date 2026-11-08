use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=IC_RUNTIME");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("runtime.bin");
    let data = match env::var("IC_RUNTIME") {
        Ok(rel) if !rel.is_empty() => {
            let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("../..");
            let path = root.join(rel);
            println!("cargo:rerun-if-changed={}", path.display());
            fs::read(&path).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    fs::write(out, data).expect("write runtime.bin");
}

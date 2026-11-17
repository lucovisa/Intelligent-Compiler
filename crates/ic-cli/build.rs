use std::{env, fs, path::PathBuf};

// Значок программы в проводнике Windows: ic.ico из ресурсов
fn embed_windows_icon() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../ic-gui/assets/ic.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    let rc = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("ic.rc");
    let icon_path = icon.display().to_string().replace('\\', "/");
    fs::write(&rc, format!("1 ICON \"{icon_path}\"\n")).expect("write ic.rc");
    if let Err(e) = embed_resource::compile(&rc, embed_resource::NONE).manifest_optional() {
        println!("cargo:warning=значок не встроен: {e}");
    }
}

fn main() {
    embed_windows_icon();
    println!("cargo:rerun-if-env-changed=IC_RUNTIME");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("runtime.bin");
    let data = match env::var("IC_RUNTIME") {
        Ok(rel) if !rel.is_empty() => {
            let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
                .join("../..");
            let path = root.join(rel);
            println!("cargo:rerun-if-changed={}", path.display());
            fs::read(&path).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    fs::write(out, data).expect("write runtime.bin");
}

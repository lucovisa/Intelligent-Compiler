#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{env, fs, process::exit};

fn report(msg: &str) {
    eprintln!("[IC] {msg}");
    if cfg!(windows) {
        if let Some(dir) = env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        {
            let _ = fs::write(dir.join("ic-error.log"), msg);
        }
    }
}

fn main() {
    ic_player::confine_caches();
    let files = env::current_exe()
        .ok()
        .and_then(|e| ic_build::read_embedded(&e));
    match files {
        Some(f) => {
            if let Err(e) = ic_player::run_embedded(f) {
                report(&e);
                exit(1);
            }
        }
        None => {
            report("в этом файле нет игры");
            exit(1);
        }
    }
}

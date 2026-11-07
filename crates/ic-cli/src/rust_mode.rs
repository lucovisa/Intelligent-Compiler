//! Режим Б (по желанию): игры на Rust. Компилятор качается ЛОКАЛЬНО в `./.ic/`
//! (RUSTUP_HOME / CARGO_HOME указаны туда), глобальные настройки и PATH не трогаем.
//! Внешние зависимости — только `curl` и `tar` (есть в Windows 10+, Linux, macOS).
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const ZIG_VERSION: &str = "0.13.0";

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

fn absolute(p: &Path) -> Result<PathBuf, String> {
    if p.is_absolute() {
        Ok(p.to_path_buf())
    } else {
        env::current_dir().map(|c| c.join(p)).map_err(|e| e.to_string())
    }
}

fn run_ok(cmd: &mut Command, what: &str) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{what}: не удалось запустить ({e})"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: ошибка ({status})")) }
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    run_ok(
        Command::new("curl").args(["-fL", "--proto", "=https", "--tlsv1.2", "-o"]).arg(to).arg(url),
        &format!("скачивание {url}"),
    )
}

/// Windows: линкер-обёртка на локальном Zig (чтобы не требовать Visual Studio Build Tools).
fn setup_zig(ic: &Path) -> Result<(), String> {
    let zig_dir = ic.join("zig");
    if !zig_dir.join("zig.exe").exists() {
        let archive = ic.join("zig.zip");
        let name = format!("zig-windows-x86_64-{ZIG_VERSION}");
        println!("Скачиваю Zig (линкер) в {} ...", zig_dir.display());
        download(&format!("https://ziglang.org/download/{ZIG_VERSION}/{name}.zip"), &archive)?;
        run_ok(Command::new("tar").arg("-xf").arg(&archive).arg("-C").arg(ic), "распаковка Zig")?;
        fs::rename(ic.join(&name), &zig_dir).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&archive);
    }
    fs::write(
        ic.join("zigcc.cmd"),
        "@echo off\r\n\"%~dp0zig\\zig.exe\" cc -target x86_64-windows-gnu %*\r\n",
    )
    .map_err(|e| e.to_string())
}

/// Скачивает Rust (+ линкер на Windows) в `<project>/.ic/`.
pub fn setup(project: &Path) -> Result<(), String> {
    if !cfg!(windows) && Command::new("cc").arg("--version").output().is_err() {
        return Err("нужен C-компилятор (cc). Linux: поставь gcc/build-essential, macOS: xcode-select --install".into());
    }

    let ic = absolute(project)?.join(".ic");
    fs::create_dir_all(&ic).map_err(|e| e.to_string())?;
    let rustup_home = ic.join("rustup");
    let cargo_home = ic.join("cargo");

    if cargo_home.join("bin").join(exe("cargo")).exists() {
        println!("Rust уже установлен в {}", ic.display());
    } else {
        println!("Скачиваю Rust (официальный rustup) в {} ...", ic.display());
        let args = ["-y", "--no-modify-path", "--profile", "minimal", "--default-toolchain", "stable"];
        if cfg!(windows) {
            let init = ic.join("rustup-init.exe");
            download("https://win.rustup.rs/x86_64", &init)?;
            run_ok(
                Command::new(&init)
                    .args(args)
                    .args(["--default-host", "x86_64-pc-windows-gnu"])
                    .env("RUSTUP_HOME", &rustup_home)
                    .env("CARGO_HOME", &cargo_home),
                "rustup-init",
            )?;
        } else {
            let script = ic.join("rustup.sh");
            download("https://sh.rustup.rs", &script)?;
            run_ok(
                Command::new("sh")
                    .arg(&script)
                    .args(args)
                    .env("RUSTUP_HOME", &rustup_home)
                    .env("CARGO_HOME", &cargo_home),
                "rustup",
            )?;
        }
    }

    if cfg!(windows) {
        setup_zig(&ic)?;
        let cfg_dir = absolute(project)?.join(".cargo");
        fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;
        fs::write(
            cfg_dir.join("config.toml"),
            "[target.x86_64-pc-windows-gnu]\nlinker = \".ic/zigcc.cmd\"\n",
        )
        .map_err(|e| e.to_string())?;
    }

    println!("Готово. Собрать: ic rust build   Запустить: ic rust run");
    Ok(())
}

/// Запускает cargo из локальной установки.
pub fn cargo(project: &Path, args: &[&str]) -> Result<(), String> {
    let root = absolute(project)?;
    let ic = root.join(".ic");
    let cargo_home = ic.join("cargo");
    let cargo_bin = cargo_home.join("bin");
    let cargo_exe = cargo_bin.join(exe("cargo"));
    if !cargo_exe.exists() {
        return Err("Rust не установлен в этой папке. Выполни: ic rust setup".into());
    }

    let old_path = env::var_os("PATH").unwrap_or_default();
    let new_path = env::join_paths(std::iter::once(cargo_bin).chain(env::split_paths(&old_path)))
        .map_err(|e| e.to_string())?;

    run_ok(
        Command::new(cargo_exe)
            .args(args)
            .current_dir(&root)
            .env("RUSTUP_HOME", ic.join("rustup"))
            .env("CARGO_HOME", &cargo_home)
            .env("PATH", new_path),
        "cargo",
    )
}

use crate::{confine, downloads, langs};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

// Языки, которые IC умеет скачивать: фиксированные версии с официальных сайтов
pub struct Tool {
    pub id: &'static str,
    pub name: &'static str,
    pub size: &'static str,
}

pub const TOOLS: &[Tool] = &[
    Tool {
        id: "cpp",
        name: "C++ (Zig)",
        size: "≈ 80 МБ",
    },
    Tool {
        id: "c",
        name: "C (Zig)",
        size: "≈ 80 МБ",
    },
    Tool {
        id: "zig",
        name: "Zig",
        size: "≈ 80 МБ",
    },
    Tool {
        id: "rust",
        name: "Rust",
        size: "≈ 250 МБ",
    },
    Tool {
        id: "go",
        name: "Go",
        size: "≈ 70 МБ",
    },
    Tool {
        id: "csharp",
        name: "C# (.NET)",
        size: "≈ 250 МБ",
    },
    Tool {
        id: "python",
        name: "Python",
        size: "≈ 65 МБ",
    },
    Tool {
        id: "node",
        name: "JavaScript (Node.js)",
        size: "≈ 30 МБ",
    },
];

const GO_VERSION: &str = "1.23.4";
const NODE_VERSION: &str = "22.12.0";
const PYTHON_VERSION: &str = "3.12.8";
const PYTHON_TAG: &str = "20241206";

pub fn os_arch() -> (&'static str, &'static str) {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };
    (os, arch)
}

pub fn go_url(os: &str, arch: &str) -> String {
    let (o, a) = (
        if os == "macos" { "darwin" } else { os },
        if arch == "aarch64" { "arm64" } else { "amd64" },
    );
    let ext = if os == "windows" { "zip" } else { "tar.gz" };
    format!("https://go.dev/dl/go{GO_VERSION}.{o}-{a}.{ext}")
}

pub fn node_url(os: &str, arch: &str) -> String {
    let (o, ext) = match os {
        "windows" => ("win", "zip"),
        "macos" => ("darwin", "tar.gz"),
        _ => ("linux", "tar.xz"),
    };
    let a = if arch == "aarch64" { "arm64" } else { "x64" };
    format!("https://nodejs.org/dist/v{NODE_VERSION}/node-v{NODE_VERSION}-{o}-{a}.{ext}")
}

pub fn python_url(os: &str, arch: &str) -> String {
    let triple = match (os, arch) {
        ("windows", _) => "x86_64-pc-windows-msvc",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", _) => "x86_64-apple-darwin",
        (_, "aarch64") => "aarch64-unknown-linux-gnu",
        _ => "x86_64-unknown-linux-gnu",
    };
    format!(
        "https://github.com/astral-sh/python-build-standalone/releases/download/{PYTHON_TAG}/cpython-{PYTHON_VERSION}+{PYTHON_TAG}-{triple}-install_only.tar.gz"
    )
}

pub fn rustup_url(os: &str, arch: &str) -> String {
    let triple = match (os, arch) {
        ("windows", _) => "x86_64-pc-windows-gnu",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", _) => "x86_64-apple-darwin",
        (_, "aarch64") => "aarch64-unknown-linux-gnu",
        _ => "x86_64-unknown-linux-gnu",
    };
    let file = if os == "windows" {
        "rustup-init.exe"
    } else {
        "rustup-init"
    };
    format!("https://static.rust-lang.org/rustup/dist/{triple}/{file}")
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

// Распаковка: tar понимает tar.gz, tar.xz и (в Windows 10+) zip
fn extract(archive: &Path, dest: &Path) -> Result<(), String> {
    let mut cmd = Command::new("tar");
    confine::child_env(&mut cmd, dest);
    let status = cmd
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "не удалось распаковать {} ({status})",
            archive.display()
        ))
    }
}

fn fetch_and_extract(url: &str, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let name = url.rsplit('/').next().unwrap_or("download");
    let archive = dest.join(format!("download-{name}"));
    downloads::download(url, &archive)?;
    let result = extract(&archive, dest);
    let _ = fs::remove_file(&archive);
    result
}

fn run_checked(cmd: &mut Command, what: &str) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let tail: String = String::from_utf8_lossy(&out.stderr)
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .join(" | ");
        Err(format!("{what}: {} {tail}", out.status))
    }
}

pub struct Installed {
    pub compiler: PathBuf,
    // (имя файла .lang, содержимое)
    pub langs: Vec<(String, String)>,
}

#[allow(clippy::too_many_arguments)]
fn lang_text(
    name: &str,
    exts: &[&str],
    badge: &str,
    color: &str,
    compiler: &Path,
    build: &[&str],
    run: &[&str],
    env: &[(&str, String)],
) -> String {
    let list = |v: &[&str]| {
        v.iter()
            .map(|x| format!("\"{x}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let exts_list = list(exts);
    let mut out = format!(
        "name = \"{name}\"\nextensions = [{exts_list}]\nbadge = \"{badge}\"\ncolor = \"{color}\"\ncompiler = \"{}\"\n",
        compiler.display()
    );
    if !build.is_empty() {
        out.push_str(&format!("build = [{}]\n", list(build)));
    }
    out.push_str(&format!("run = [{}]\n", list(run)));
    if !env.is_empty() {
        let items: Vec<String> = env
            .iter()
            .map(|(k, v)| format!("\"{k}={}\"", v.replace('"', "")))
            .collect();
        out.push_str(&format!("env = [{}]\n", items.join(", ")));
    }
    out
}

fn cache(data: &Path, name: &str) -> String {
    data.join("cache").join(name).display().to_string()
}

pub fn install(id: &str, data: &Path) -> Result<Installed, String> {
    confine::check(data)?;
    let tc = data.join("toolchains");
    let (os, arch) = os_arch();
    match id {
        "cpp" | "c" | "zig" => {
            let zig = downloads::install_zig(&tc)?;
            let text = match id {
                "cpp" => downloads::cpp_lang_text(&zig),
                "c" => lang_text(
                    "C",
                    &["c"],
                    "C",
                    "#4a86c0",
                    &zig,
                    &["{compiler}", "cc", "{file}", "-o", "{out}"],
                    &["{out}"],
                    &[],
                ),
                _ => lang_text(
                    "Zig",
                    &["zig"],
                    "Zg",
                    "#f7a41d",
                    &zig,
                    &["{compiler}", "build-exe", "{file}", "-femit-bin={out}"],
                    &["{out}"],
                    &[],
                ),
            };
            Ok(Installed {
                compiler: zig,
                langs: vec![(format!("{id}.lang"), text)],
            })
        }
        "go" => {
            let dest = tc.join("go-sdk");
            let go = dest.join("go").join("bin").join(exe("go"));
            if !go.is_file() {
                fetch_and_extract(&go_url(os, arch), &dest)?;
            }
            if !go.is_file() {
                return Err("после распаковки Go не найден".into());
            }
            let env = [
                ("GOCACHE", cache(data, "go-build")),
                ("GOPATH", cache(data, "go-path")),
                ("GOTMPDIR", cache(data, "go-tmp")),
            ];
            for (_, dir) in &env {
                let _ = fs::create_dir_all(dir);
            }
            let text = lang_text(
                "Go",
                &["go"],
                "Go",
                "#00add8",
                &go,
                &["{compiler}", "build", "-o", "{out}", "{file}"],
                &["{out}"],
                &env,
            );
            Ok(Installed {
                compiler: go,
                langs: vec![("go.lang".into(), text)],
            })
        }
        "python" => {
            let dest = tc.join("python-sdk");
            let py = if os == "windows" {
                dest.join("python").join("python.exe")
            } else {
                dest.join("python").join("bin").join("python3")
            };
            if !py.is_file() {
                fetch_and_extract(&python_url(os, arch), &dest)?;
            }
            if !py.is_file() {
                return Err("после распаковки Python не найден".into());
            }
            let env = [
                ("PYTHONPYCACHEPREFIX", cache(data, "pycache")),
                ("PYTHONDONTWRITEBYTECODE", "1".to_string()),
            ];
            let text = lang_text(
                "Python",
                &["py"],
                "Py",
                "#3776ab",
                &py,
                &[],
                &["{compiler}", "{file}"],
                &env,
            );
            Ok(Installed {
                compiler: py,
                langs: vec![("python.lang".into(), text)],
            })
        }
        "node" => {
            let dest = tc.join("node-sdk");
            let folder = format!(
                "node-v{NODE_VERSION}-{}-{}",
                match os {
                    "windows" => "win",
                    "macos" => "darwin",
                    _ => "linux",
                },
                if arch == "aarch64" { "arm64" } else { "x64" }
            );
            let root = dest.join("node");
            let node = if os == "windows" {
                root.join("node.exe")
            } else {
                root.join("bin").join("node")
            };
            if !node.is_file() {
                fetch_and_extract(&node_url(os, arch), &dest)?;
                let _ = fs::remove_dir_all(&root);
                fs::rename(dest.join(&folder), &root).map_err(|e| format!("Node: {e}"))?;
            }
            if !node.is_file() {
                return Err("после распаковки Node.js не найден".into());
            }
            let env = [("npm_config_cache", cache(data, "npm"))];
            let text = lang_text(
                "JavaScript",
                &["js", "mjs"],
                "JS",
                "#e5c03a",
                &node,
                &[],
                &["{compiler}", "{file}"],
                &env,
            );
            Ok(Installed {
                compiler: node,
                langs: vec![("javascript.lang".into(), text)],
            })
        }
        "rust" => {
            let dest = tc.join("rust-sdk");
            let (rustup_home, cargo_home) = (dest.join("rustup"), dest.join("cargo"));
            let rustc = cargo_home.join("bin").join(exe("rustc"));
            if !rustc.is_file() {
                fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
                let init = dest.join(exe("rustup-init"));
                downloads::download(&rustup_url(os, arch), &init)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&init, fs::Permissions::from_mode(0o755));
                }
                let mut cmd = Command::new(&init);
                confine::child_env(&mut cmd, &dest);
                cmd.args([
                    "-y",
                    "--no-modify-path",
                    "--profile",
                    "minimal",
                    "--default-toolchain",
                    "stable",
                ])
                .env("RUSTUP_HOME", &rustup_home)
                .env("CARGO_HOME", &cargo_home);
                if os == "windows" {
                    cmd.args(["--default-host", "x86_64-pc-windows-gnu"]);
                }
                run_checked(&mut cmd, "rustup")?;
            }
            if !rustc.is_file() {
                return Err("после установки rustc не найден".into());
            }
            let mut build: Vec<String> = vec![
                "{compiler}".into(),
                "{file}".into(),
                "-o".into(),
                "{out}".into(),
            ];
            if os == "windows" {
                // На Windows (gnu) линковщиком служит Zig
                let zig = downloads::install_zig(&tc)?;
                let wrapper = dest.join("zigcc.cmd");
                fs::write(&wrapper, format!("@\"{}\" cc %*\r\n", zig.display()))
                    .map_err(|e| e.to_string())?;
                build.push("-C".into());
                build.push(format!("linker={}", wrapper.display()));
            }
            let build_refs: Vec<&str> = build.iter().map(String::as_str).collect();
            let env = [
                ("RUSTUP_HOME", rustup_home.display().to_string()),
                ("CARGO_HOME", cargo_home.display().to_string()),
            ];
            let text = lang_text(
                "Rust",
                &["rs"],
                "Rs",
                "#de7a45",
                &rustc,
                &build_refs,
                &["{out}"],
                &env,
            );
            Ok(Installed {
                compiler: rustc,
                langs: vec![("rust.lang".into(), text)],
            })
        }
        "csharp" => {
            let dest = tc.join("dotnet-sdk");
            let dotnet = dest.join(exe("dotnet"));
            if !dotnet.is_file() {
                fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
                let mut cmd;
                if os == "windows" {
                    let script = dest.join("dotnet-install.ps1");
                    downloads::download("https://dot.net/v1/dotnet-install.ps1", &script)?;
                    cmd = Command::new("powershell");
                    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                        .arg(&script)
                        .args(["-InstallDir"])
                        .arg(&dest)
                        .args(["-Channel", "10.0", "-NoPath"]);
                } else {
                    let script = dest.join("dotnet-install.sh");
                    downloads::download("https://dot.net/v1/dotnet-install.sh", &script)?;
                    cmd = Command::new("bash");
                    cmd.arg(&script).arg("--install-dir").arg(&dest).args([
                        "--channel",
                        "10.0",
                        "--no-path",
                    ]);
                }
                confine::child_env(&mut cmd, &dest);
                cmd.env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
                    .env("DOTNET_CLI_HOME", cache(data, "dotnet"));
                run_checked(&mut cmd, "dotnet-install")?;
            }
            if !dotnet.is_file() {
                return Err("после установки dotnet не найден".into());
            }
            let env = [
                ("DOTNET_CLI_HOME", cache(data, "dotnet")),
                ("DOTNET_NOLOGO", "1".to_string()),
                ("DOTNET_CLI_TELEMETRY_OPTOUT", "1".to_string()),
                ("NUGET_PACKAGES", cache(data, "nuget")),
            ];
            let text = lang_text(
                "C#",
                &["cs"],
                "C#",
                "#844fb0",
                &dotnet,
                &[],
                &["{compiler}", "run", "{file}"],
                &env,
            );
            Ok(Installed {
                compiler: dotnet,
                langs: vec![("csharp.lang".into(), text)],
            })
        }
        other => Err(format!("неизвестный язык: {other}")),
    }
}

// Установка и запись описаний в IC-data/languages
pub fn install_and_register(id: &str, data: &Path) -> Result<PathBuf, String> {
    let installed = install(id, data)?;
    let folder = data.join("languages");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    for (name, text) in &installed.langs {
        fs::write(folder.join(name), text).map_err(|e| e.to_string())?;
    }
    Ok(installed.compiler)
}

// Установлен ли язык: есть описание и существует компилятор
pub fn is_installed(id: &str, data: &Path) -> bool {
    let file = match id {
        "node" => "javascript.lang",
        "csharp" => "csharp.lang",
        other => return check(&data.join("languages").join(format!("{other}.lang"))),
    };
    check(&data.join("languages").join(file))
}

fn check(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| langs::parse(&t, path).ok())
        .and_then(|l| l.compiler)
        .is_some_and(|c| c.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_follow_official_patterns() {
        assert_eq!(
            go_url("linux", "x86_64"),
            "https://go.dev/dl/go1.23.4.linux-amd64.tar.gz"
        );
        assert_eq!(
            go_url("windows", "x86_64"),
            "https://go.dev/dl/go1.23.4.windows-amd64.zip"
        );
        assert_eq!(
            go_url("macos", "aarch64"),
            "https://go.dev/dl/go1.23.4.darwin-arm64.tar.gz"
        );
        assert_eq!(
            node_url("linux", "x86_64"),
            "https://nodejs.org/dist/v22.12.0/node-v22.12.0-linux-x64.tar.xz"
        );
        assert!(node_url("windows", "x86_64").ends_with("win-x64.zip"));
        assert!(python_url("linux", "x86_64")
            .contains("cpython-3.12.8+20241206-x86_64-unknown-linux-gnu-install_only"));
        assert!(rustup_url("windows", "x86_64").ends_with("x86_64-pc-windows-gnu/rustup-init.exe"));
        assert!(rustup_url("macos", "aarch64").contains("aarch64-apple-darwin"));
    }

    #[test]
    fn language_files_parse_with_env_and_empty_build() {
        let text = lang_text(
            "Python",
            &["py"],
            "Py",
            "#3776ab",
            Path::new("/opt/python"),
            &[],
            &["{compiler}", "{file}"],
            &[("PYTHONDONTWRITEBYTECODE", "1".to_string())],
        );
        let l = langs::parse(&text, Path::new("/d/languages/python.lang")).unwrap();
        assert!(l.build.is_empty());
        assert_eq!(l.run, vec!["{compiler}", "{file}"]);
        assert_eq!(
            l.env,
            vec![("PYTHONDONTWRITEBYTECODE".to_string(), "1".to_string())]
        );
        assert!(l.matches("a.py"));
    }

    #[test]
    fn every_tool_has_a_known_id() {
        let data = std::env::temp_dir().join("ic_tools_unknown");
        confine::allow(&data);
        assert!(install("nope", &data).is_err());
        assert_eq!(TOOLS.len(), 8);
        assert!(!is_installed("python", &data));
    }

    #[test]
    fn archive_extraction_works_with_tar() {
        let dir = std::env::temp_dir().join(format!("ic_tools_tar_{}", std::process::id()));
        let src = dir.join("src");
        fs::create_dir_all(src.join("inner")).unwrap();
        fs::write(src.join("inner/file.txt"), b"hi").unwrap();
        confine::allow(&dir);
        let archive = dir.join("a.tar.gz");
        let made = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&src)
            .arg(".")
            .status();
        if made.is_ok_and(|s| s.success()) {
            let out = dir.join("out");
            fs::create_dir_all(&out).unwrap();
            extract(&archive, &out).unwrap();
            assert_eq!(fs::read(out.join("inner/file.txt")).unwrap(), b"hi");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    // Реальная загрузка Python с GitHub: cargo test -p ic-build -- --ignored
    #[test]
    #[ignore]
    fn python_installs_and_runs() {
        let data = std::env::temp_dir().join(format!("ic_tools_py_{}", std::process::id()));
        fs::create_dir_all(&data).unwrap();
        confine::allow(&data);
        let compiler = install_and_register("python", &data).unwrap();
        let out = Command::new(&compiler)
            .args(["-c", "print(6*7)"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "42");
        assert!(is_installed("python", &data));
        let _ = fs::remove_dir_all(&data);
    }
}

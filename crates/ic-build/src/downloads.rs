use crate::confine;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub const REPO: &str = "lucovisa/Intelligent-Compiler";
pub const ZIG_VERSION: &str = "0.13.0";

// "v0.2.1" → [0, 2, 1]
pub fn parse_version(s: &str) -> Option<[u32; 3]> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let mut parts = s.split('.').map(|p| {
        p.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    Some([parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?])
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

// Тег из адреса вида .../releases/tag/v0.2.0
pub fn tag_from_url(url: &str) -> Option<String> {
    let tag = url.split("/releases/tag/").nth(1)?;
    let tag = tag.split(['?', '#', '/']).next()?;
    (!tag.is_empty()).then(|| tag.to_string())
}

// Имя архива в релизе (как в CI)
pub fn asset_name() -> &'static str {
    if cfg!(windows) {
        "ic-windows.zip"
    } else if cfg!(target_os = "macos") {
        "ic-macos.zip"
    } else {
        "ic-linux.zip"
    }
}

fn curl(dir: &Path) -> Command {
    let mut cmd = Command::new("curl");
    // Temp и кэши — внутрь папки загрузки
    confine::child_env(&mut cmd, dir);
    cmd
}

pub fn download(url: &str, to: &Path) -> Result<(), String> {
    confine::check(to)?;
    let dir = to.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let status = curl(dir)
        .args(["-fL", "--proto", "=https", "--tlsv1.2", "-m", "900", "-o"])
        .arg(to)
        .arg(url)
        .status()
        .map_err(|e| format!("curl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("не удалось скачать {url} ({status})"))
    }
}

// Последний релиз: Ok(None), если релизов ещё нет
pub fn latest_tag() -> Result<Option<String>, String> {
    let sink = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let out = Command::new("curl")
        .args(["-sIL", "--proto", "=https", "-m", "20", "-o", sink, "-w", "%{url_effective}"])
        .arg(format!("https://github.com/{REPO}/releases/latest"))
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err("нет связи с GitHub".into());
    }
    Ok(tag_from_url(&String::from_utf8_lossy(&out.stdout)))
}

pub fn download_update(tag: &str, dir: &Path) -> Result<PathBuf, String> {
    let to = dir.join(asset_name());
    download(
        &format!("https://github.com/{REPO}/releases/download/{tag}/{}", asset_name()),
        &to,
    )?;
    Ok(to)
}

// Достаём `ic` из zip и подменяем текущий файл; старый остаётся как *.old
pub fn install_update(zip_path: &Path, exe: &Path) -> Result<(), String> {
    confine::check(exe)?;
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;
    let want = if cfg!(windows) { "ic.exe" } else { "ic" };
    let mut index = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().rsplit('/').next().unwrap_or("").to_string();
        if name == want {
            index = Some(i);
            break;
        }
    }
    let index = index.ok_or_else(|| format!("в архиве нет {want}"))?;
    let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;

    let mut staged = exe.as_os_str().to_owned();
    staged.push(".new");
    let staged = PathBuf::from(staged);
    {
        let mut out = fs::File::create(&staged).map_err(|e| e.to_string())?;
        io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    let mut old = exe.as_os_str().to_owned();
    old.push(".old");
    let old = PathBuf::from(old);
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old).map_err(|e| format!("не удалось заменить программу: {e}"))?;
    if let Err(e) = fs::rename(&staged, exe) {
        let _ = fs::rename(&old, exe);
        return Err(format!("не удалось поставить новую версию: {e}"));
    }
    Ok(())
}

// Удаляем *.old после обновления
pub fn cleanup_old_exe() {
    if let Ok(exe) = std::env::current_exe() {
        let mut old = exe.into_os_string();
        old.push(".old");
        let _ = fs::remove_file(old);
    }
}

fn zig_target() -> (&'static str, &'static str) {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    (os, arch)
}

pub fn zig_exe(dest: &Path) -> PathBuf {
    dest.join("zig").join(if cfg!(windows) { "zig.exe" } else { "zig" })
}

// Zig нужен для C++: `zig c++` — это встроенный clang
pub fn install_zig(dest: &Path) -> Result<PathBuf, String> {
    confine::check(dest)?;
    let exe = zig_exe(dest);
    if exe.is_file() {
        return Ok(exe);
    }
    fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let (os, arch) = zig_target();
    let name = format!("zig-{os}-{arch}-{ZIG_VERSION}");
    let ext = if cfg!(windows) { "zip" } else { "tar.xz" };
    let archive = dest.join(format!("zig-download.{ext}"));
    download(
        &format!("https://ziglang.org/download/{ZIG_VERSION}/{name}.{ext}"),
        &archive,
    )?;
    let status = curl_free_tar(&archive, dest)?;
    let _ = fs::remove_file(&archive);
    if !status {
        return Err("не удалось распаковать Zig (нужен tar с поддержкой xz)".into());
    }
    let _ = fs::remove_dir_all(dest.join("zig"));
    fs::rename(dest.join(&name), dest.join("zig")).map_err(|e| e.to_string())?;
    if exe.is_file() {
        Ok(exe)
    } else {
        Err("после распаковки Zig не найден".into())
    }
}

fn curl_free_tar(archive: &Path, dest: &Path) -> Result<bool, String> {
    let mut cmd = Command::new("tar");
    confine::child_env(&mut cmd, dest);
    cmd.arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .map(|s| s.success())
        .map_err(|e| format!("tar: {e}"))
}

// Описание языка C++ для IC-data/languages/cpp.lang
pub fn cpp_lang_text(zig: &Path) -> String {
    format!(
        "name = \"C++\"\nextensions = [\"cpp\", \"cc\", \"cxx\"]\nbadge = \"C++\"\ncolor = \"#2f76c8\"\n\
         compiler = \"{}\"\nbuild = [\"{{compiler}}\", \"c++\", \"{{file}}\", \"-o\", \"{{out}}\"]\nrun = [\"{{out}}\"]\n",
        zig.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(parse_version("v0.2.1"), Some([0, 2, 1]));
        assert_eq!(parse_version("1.4"), Some([1, 4, 0]));
        assert!(is_newer("0.1.0", "v0.2.0"));
        assert!(is_newer("0.9.0", "0.10.0"));
        assert!(!is_newer("0.2.0", "v0.2.0"));
        assert!(!is_newer("0.3.0", "v0.2.9"));
        assert!(!is_newer("0.1.0", "latest"));
    }

    #[test]
    fn tag_is_parsed_from_redirect_url() {
        assert_eq!(
            tag_from_url("https://github.com/a/b/releases/tag/v0.2.0"),
            Some("v0.2.0".to_string())
        );
        assert_eq!(tag_from_url("https://github.com/a/b/releases"), None);
    }

    #[test]
    fn update_replaces_exe_and_keeps_old() {
        let dir = std::env::temp_dir().join(format!("ic_update_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        confine::allow(&dir);
        let exe = dir.join("ic");
        fs::write(&exe, b"OLD").unwrap();
        let zip_path = dir.join("ic-test.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
            w.start_file("pkg/ic", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"NEW").unwrap();
            w.start_file("pkg/ic.exe", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"NEW").unwrap();
            w.finish().unwrap();
        }
        if !cfg!(windows) {
            install_update(&zip_path, &exe).unwrap();
            assert_eq!(fs::read(&exe).unwrap(), b"NEW");
            assert_eq!(fs::read(dir.join("ic.old")).unwrap(), b"OLD");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_outside_allowed_paths_is_refused() {
        let dir = std::env::temp_dir().join(format!("ic_update_deny_{}", std::process::id()));
        assert!(install_update(&dir.join("x.zip"), &dir.join("ic")).is_err());
    }

    #[test]
    fn cpp_preset_parses_as_language() {
        let text = cpp_lang_text(Path::new("/opt/zig/zig"));
        let lang = crate::langs::parse(&text, Path::new("/d/languages/cpp.lang")).unwrap();
        assert_eq!(lang.name, "C++");
        assert!(lang.matches("main.cpp"));
        assert_eq!(lang.compiler, Some(PathBuf::from("/opt/zig/zig")));
    }
}

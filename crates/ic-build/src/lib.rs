pub mod confine;
pub mod langs;
pub mod scaffold;
mod settings;

pub use settings::ProjectSettings;

use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

const MAGIC: &[u8; 8] = b"ICGAME01";
const SKIP_DIRS: [&str; 5] = [".ic", "target", ".git", "build", ".cargo"];
const SKIP_FILES: &[&str] = &["ic", "ic.exe", "ic-settings.txt", "ic-error.log"];

fn walk(
    root: &Path,
    cur: &Path,
    skip: Option<&Path>,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    let entries = fs::read_dir(cur).map_err(|e| format!("{}: {e}", cur.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(root, &path, skip, out)?;
            }
            continue;
        }
        if SKIP_FILES.contains(&name.as_str()) || fs::canonicalize(&path).ok().as_deref() == skip {
            continue;
        }
        let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
        let rel = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        out.push((rel, path));
    }
    Ok(())
}

pub fn collect(project: &Path, runtime: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let runtime_canon = fs::canonicalize(runtime).ok();
    let mut files = Vec::new();
    walk(project, project, runtime_canon.as_deref(), &mut files)?;
    Ok(files)
}

pub fn write_game(
    entries: &[(String, Vec<u8>)],
    runtime: &[u8],
    out: &Path,
) -> Result<u64, String> {
    let mut payload: Vec<u8> = Vec::new();
    for (rel, data) in entries {
        payload.extend((rel.len() as u32).to_le_bytes());
        payload.extend(rel.as_bytes());
        payload.extend((data.len() as u64).to_le_bytes());
        payload.extend(data);
    }

    let mut exe: Vec<u8> = Vec::with_capacity(runtime.len() + payload.len() + 16);
    exe.extend(runtime);
    exe.extend(&payload);
    exe.extend((payload.len() as u64).to_le_bytes());
    exe.extend(MAGIC);

    // Запись только в X или в путь, выбранный пользователем
    confine::check(out)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(out, &exe).map_err(|e| format!("{}: {e}", out.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(out, fs::Permissions::from_mode(0o755));
    }
    Ok(exe.len() as u64)
}

pub fn pack_entries(
    entries: &[(String, Vec<u8>)],
    runtime: &Path,
    out: &Path,
) -> Result<u64, String> {
    let rt = fs::read(runtime).map_err(|e| format!("{}: {e}", runtime.display()))?;
    write_game(entries, &rt, out)
}

pub fn pack(project: &Path, runtime: &Path, out: &Path) -> Result<usize, String> {
    if !project.join("main.lua").is_file() {
        return Err(format!("в {} нет main.lua", project.display()));
    }
    let mut entries = Vec::new();
    for (rel, path) in collect(project, runtime)? {
        let data = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        entries.push((rel, data));
    }
    pack_entries(&entries, runtime, out)?;
    Ok(entries.len())
}

pub fn read_embedded(exe: &Path) -> Option<HashMap<String, Vec<u8>>> {
    let mut f = fs::File::open(exe).ok()?;
    let total = f.metadata().ok()?.len();
    if total < 16 {
        return None;
    }
    f.seek(SeekFrom::Start(total - 16)).ok()?;
    let mut footer = [0u8; 16];
    f.read_exact(&mut footer).ok()?;
    if &footer[8..] != MAGIC {
        return None;
    }
    let len = u64::from_le_bytes(footer[..8].try_into().ok()?);
    if len > total - 16 {
        return None;
    }
    f.seek(SeekFrom::Start(total - 16 - len)).ok()?;
    let mut payload = vec![0u8; len as usize];
    f.read_exact(&mut payload).ok()?;

    let mut map = HashMap::new();
    let mut i = 0usize;
    while i < payload.len() {
        let pl = u32::from_le_bytes(payload.get(i..i + 4)?.try_into().ok()?) as usize;
        i += 4;
        let path = String::from_utf8(payload.get(i..i + pl)?.to_vec()).ok()?;
        i += pl;
        let dl = u64::from_le_bytes(payload.get(i..i + 8)?.try_into().ok()?) as usize;
        i += 8;
        let data = payload.get(i..i + dl)?.to_vec();
        i += dl;
        map.insert(path, data);
    }
    Some(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_roundtrip() {
        let base = std::env::temp_dir().join(format!("ic_build_test_{}", std::process::id()));
        let proj = base.join("game");
        confine::allow(&base);
        fs::create_dir_all(proj.join("lib")).unwrap();
        fs::write(proj.join("main.lua"), "print(1)").unwrap();
        fs::write(proj.join("lib/util.lua"), "return {}").unwrap();
        let runtime = base.join("runtime.bin");
        fs::write(&runtime, b"FAKE-RUNTIME").unwrap();

        let out = proj.join("build").join("game.bin");
        let n = pack(&proj, &runtime, &out).unwrap();
        assert_eq!(n, 2);

        let files = read_embedded(&out).unwrap();
        assert_eq!(files["main.lua"], b"print(1)".to_vec());
        assert!(files.contains_key("lib/util.lua"));
        assert!(read_embedded(&runtime).is_none());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn write_game_with_embedded_runtime_bytes() {
        let base = std::env::temp_dir().join(format!("ic_build_bytes_{}", std::process::id()));
        let out = base.join("g.bin");
        confine::allow(&base);
        let entries = vec![("main.lua".to_string(), b"x = 1".to_vec())];
        let size = write_game(&entries, b"RT", &out).unwrap();
        assert_eq!(size, fs::metadata(&out).unwrap().len());
        assert_eq!(read_embedded(&out).unwrap()["main.lua"], b"x = 1".to_vec());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn pack_requires_main_lua() {
        let base = std::env::temp_dir().join(format!("ic_build_empty_{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        assert!(pack(&base, &base.join("x"), &base.join("out")).is_err());
        let _ = fs::remove_dir_all(&base);
    }
}

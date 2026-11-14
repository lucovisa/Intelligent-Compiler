//! Ограничитель путей: IC сам ничего не создаёт за пределами папки X
//! (папка с исполняемым файлом), кроме путей, явно выбранных пользователем.

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::Mutex,
};

// Пути, которые пользователь выбрал сам (проект, папка вывода)
static ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

// Папка X — рядом с исполняемым файлом
pub fn base_dir() -> Option<PathBuf> {
    env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

// Абсолютный путь без `.` и `..` (без обращения к диску)
fn normalize(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        env::current_dir().unwrap_or_default().join(p)
    };
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

// Реальный путь: ближайший существующий предок раскрывается (симлинки),
// несуществующий хвост дописывается как есть
fn resolve(p: &Path) -> PathBuf {
    let norm = normalize(p);
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = norm.as_path();
    loop {
        if let Ok(real) = fs::canonicalize(cur) {
            let mut res = real;
            for part in tail.iter().rev() {
                res.push(part);
            }
            return res;
        }
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                cur = parent;
            }
            _ => return norm,
        }
    }
}

// Пользователь явно выбрал этот путь: разрешаем писать внутри него
pub fn allow(path: &Path) {
    let real = resolve(path);
    let mut roots = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    if !roots.contains(&real) {
        roots.push(real);
    }
}

pub fn is_allowed(path: &Path) -> bool {
    let real = resolve(path);
    if let Some(base) = base_dir() {
        if real.starts_with(resolve(&base)) {
            return true;
        }
    }
    let roots = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    roots.iter().any(|r| real.starts_with(r))
}

// Проверка перед записью
pub fn check(path: &Path) -> Result<(), String> {
    if is_allowed(path) {
        Ok(())
    } else {
        Err(format!(
            "{}: путь вне папки IC и не выбран пользователем — запись отменена",
            path.display()
        ))
    }
}

// Кэши, настройки, временные файлы и кэш Zig — внутрь папки X
pub fn confine_env(base: &Path) -> Result<(), String> {
    let cache = base.join("cache");
    let config = base.join("settings");
    let data = base.join("data");
    let tmp = base.join("tmp");
    for d in [&cache, &config, &data, &tmp] {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }

    let set = |k: &str, v: &std::ffi::OsStr| env::set_var(k, v);
    // Кэши графики и XDG
    set("XDG_CACHE_HOME", cache.as_os_str());
    set("XDG_CONFIG_HOME", config.as_os_str());
    set("XDG_DATA_HOME", data.as_os_str());
    set("MESA_SHADER_CACHE_DIR", cache.as_os_str());
    set("__GL_SHADER_DISK_CACHE_PATH", cache.as_os_str());
    set("MESA_SHADER_CACHE_MAX_SIZE", std::ffi::OsStr::new("256M"));
    set("__GL_SHADER_DISK_CACHE_SIZE", std::ffi::OsStr::new("268435456"));
    // Временные файлы
    for k in ["TMPDIR", "TEMP", "TMP"] {
        set(k, tmp.as_os_str());
    }
    // Кэш Zig
    set("ZIG_GLOBAL_CACHE_DIR", cache.join("zig-global").as_os_str());
    set("ZIG_LOCAL_CACHE_DIR", cache.join("zig-local").as_os_str());
    Ok(())
}

// Окружение дочернего процесса (curl, tar, rustup, cargo, zig) — внутрь `dir`
pub fn child_env<'a>(cmd: &'a mut Command, dir: &Path) -> &'a mut Command {
    let tmp = dir.join("tmp");
    let zig = dir.join("zig-cache");
    let _ = fs::create_dir_all(&tmp);
    let _ = fs::create_dir_all(&zig);
    for k in ["TMPDIR", "TEMP", "TMP"] {
        cmd.env(k, &tmp);
    }
    cmd.env("ZIG_GLOBAL_CACHE_DIR", zig.join("global"))
        .env("ZIG_LOCAL_CACHE_DIR", zig.join("local"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        env::temp_dir().join(format!("ic_confine_{name}_{}", std::process::id()))
    }

    #[test]
    fn base_dir_is_allowed() {
        let base = base_dir().unwrap();
        assert!(is_allowed(&base.join("cache").join("x.bin")));
    }

    #[test]
    fn outside_denied_until_user_chooses() {
        let dir = scratch("choose");
        assert!(check(&dir.join("a.txt")).is_err());
        allow(&dir);
        assert!(check(&dir.join("sub").join("a.txt")).is_ok());
    }

    #[test]
    fn dotdot_cannot_escape() {
        let dir = scratch("dotdot");
        allow(&dir);
        assert!(check(&dir.join("..").join("evil.txt")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_cannot_escape() {
        let dir = scratch("link");
        let outside = scratch("link_outside");
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(&outside).unwrap();
        allow(&dir);
        std::os::unix::fs::symlink(&outside, dir.join("l")).unwrap();
        assert!(check(&dir.join("l").join("f.txt")).is_err());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&outside);
    }
}

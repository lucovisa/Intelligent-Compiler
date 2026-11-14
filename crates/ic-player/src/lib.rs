use ic_build::ProjectSettings;
use ic_lua::LuaGame;
use ic_render::{run, Config};
use std::{collections::HashMap, env, fs, path::Path};

pub fn make_config(s: &ProjectSettings, lua_title: Option<String>) -> Config {
    let title: &'static str = Box::leak(
        lua_title
            .unwrap_or_else(|| s.title.clone())
            .into_boxed_str(),
    );
    Config {
        title,
        width: s.width,
        height: s.height,
        tick_rate: s.tick_rate,
        clear_color: s.clear_color(),
        ..Default::default()
    }
}

pub fn run_embedded(files: HashMap<String, Vec<u8>>) -> Result<(), String> {
    let settings = files
        .get(ProjectSettings::FILE)
        .map(|b| ProjectSettings::parse(&String::from_utf8_lossy(b)).0)
        .unwrap_or_default();
    let game = LuaGame::from_files(files)?;
    let cfg = make_config(&settings, game.title_opt());
    run(game, cfg);
    Ok(())
}

pub fn confine_caches() -> Result<(), String> {
    let exe = env::current_exe().map_err(|e| format!("не удалось определить папку IC: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "не удалось определить папку IC".to_string())?;
    let cache = dir.join("cache");
    let config = dir.join("settings");
    let data = dir.join("data");
    fs::create_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    fs::create_dir_all(&config).map_err(|e| format!("{}: {e}", config.display()))?;
    fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;

    let set = |k: &str, v: &std::ffi::OsStr| env::set_var(k, v);
    // Кэши графики и XDG — рядом с исполняемым файлом.
    set("XDG_CACHE_HOME", cache.as_os_str());
    set("XDG_CONFIG_HOME", config.as_os_str());
    set("XDG_DATA_HOME", data.as_os_str());
    set("MESA_SHADER_CACHE_DIR", cache.as_os_str());
    set("__GL_SHADER_DISK_CACHE_PATH", cache.as_os_str());
    set("MESA_SHADER_CACHE_MAX_SIZE", std::ffi::OsStr::new("256M"));
    set(
        "__GL_SHADER_DISK_CACHE_SIZE",
        std::ffi::OsStr::new("268435456"),
    );
    Ok(())
}

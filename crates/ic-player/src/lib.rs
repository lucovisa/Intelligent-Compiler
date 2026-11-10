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

pub fn confine_caches() {
    if let Some(dir) = env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        let cache = dir.join("cache");
        let _ = fs::create_dir_all(&cache);
        let set = |k: &str, v: &std::ffi::OsStr| {
            if env::var_os(k).is_none() {
                env::set_var(k, v);
            }
        };
        set("MESA_SHADER_CACHE_DIR", cache.as_os_str());
        set("__GL_SHADER_DISK_CACHE_PATH", cache.as_os_str());
        set("MESA_SHADER_CACHE_MAX_SIZE", std::ffi::OsStr::new("256M"));
        set(
            "__GL_SHADER_DISK_CACHE_SIZE",
            std::ffi::OsStr::new("268435456"),
        );
    }
}

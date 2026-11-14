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
    // Кэши, настройки и temp — внутрь папки IC
    ic_build::confine::confine_env(dir)?;
    Ok(())
}

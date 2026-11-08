//! Lua-слой IC: игра пишется на Lua, тяжёлое (рендер, цикл) живёт в Rust.
//!
//! Lua API (глобальная таблица `ic`):
//!   колбэки, которые определяет игра:
//!     ic.load()            -- один раз при старте
//!     ic.fixed_update(dt)  -- постоянный шаг (физика)
//!     ic.update(dt)        -- каждый кадр (логика + рисование)
//!   функции движка:
//!     ic.rect(x, y, w, h, r, g, b [, a [, rotation]])
//!     ic.key_down("w"), ic.key_pressed("space")
//!     ic.screen() -> w, h      ic.time() -> секунды
//!     ic.camera(x, y, height)  ic.quit()
//!   настройка: ic.title = "Моя игра"
#![allow(clippy::type_complexity)]

use ic_core::{Context, Game, Key, Sprite, Vec2, KEY_COUNT};
use mlua::{Function, Lua, Table, Value};
use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

#[derive(Default)]
struct Shared {
    sprites: Vec<Sprite>,
    down: [bool; KEY_COUNT],
    pressed: [bool; KEY_COUNT],
    screen: (f32, f32),
    elapsed: f32,
    camera: (f32, f32, f32),
    quit: bool,
}

fn parse_key(name: &str) -> Option<Key> {
    Some(match name.to_ascii_lowercase().as_str() {
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "w" => Key::W,
        "a" => Key::A,
        "s" => Key::S,
        "d" => Key::D,
        "space" => Key::Space,
        "escape" | "esc" => Key::Escape,
        "enter" | "return" => Key::Enter,
        _ => return None,
    })
}

fn register_api(lua: &Lua, shared: &Rc<RefCell<Shared>>) -> mlua::Result<()> {
    let ic = lua.create_table()?;

    let sh = shared.clone();
    ic.set(
        "rect",
        lua.create_function(
            move |_, a: (f32, f32, f32, f32, f32, f32, f32, Option<f32>, Option<f32>)| {
                let (x, y, w, h, r, g, b, alpha, rot) = a;
                sh.borrow_mut().sprites.push(Sprite {
                    pos: Vec2::new(x, y),
                    size: Vec2::new(w, h),
                    rotation: rot.unwrap_or(0.0),
                    color: [r, g, b, alpha.unwrap_or(1.0)],
                });
                Ok(())
            },
        )?,
    )?;

    let sh = shared.clone();
    ic.set(
        "key_down",
        lua.create_function(move |_, name: String| {
            let k = parse_key(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("неизвестная клавиша '{name}'"))
            })?;
            Ok(sh.borrow().down[k as usize])
        })?,
    )?;

    let sh = shared.clone();
    ic.set(
        "key_pressed",
        lua.create_function(move |_, name: String| {
            let k = parse_key(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("неизвестная клавиша '{name}'"))
            })?;
            Ok(sh.borrow().pressed[k as usize])
        })?,
    )?;

    let sh = shared.clone();
    ic.set(
        "screen",
        lua.create_function(move |_, ()| Ok(sh.borrow().screen))?,
    )?;

    let sh = shared.clone();
    ic.set(
        "time",
        lua.create_function(move |_, ()| Ok(sh.borrow().elapsed))?,
    )?;

    let sh = shared.clone();
    ic.set(
        "camera",
        lua.create_function(move |_, (x, y, h): (f32, f32, f32)| {
            sh.borrow_mut().camera = (x, y, h);
            Ok(())
        })?,
    )?;

    let sh = shared.clone();
    ic.set(
        "quit",
        lua.create_function(move |_, ()| {
            sh.borrow_mut().quit = true;
            Ok(())
        })?,
    )?;

    lua.globals().set("ic", ic)
}

/// require("модуль") ищет в файлах, вшитых в собранную игру.
fn install_embedded_searcher(lua: &Lua, files: Rc<HashMap<String, Vec<u8>>>) -> mlua::Result<()> {
    let searcher = lua.create_function(move |lua, name: String| {
        let base = name.replace('.', "/");
        for path in [format!("{base}.lua"), format!("{base}/init.lua")] {
            if let Some(src) = files.get(&path) {
                let f = lua
                    .load(src.as_slice())
                    .set_name(format!("@{path}"))
                    .into_function()?;
                return Ok(Value::Function(f));
            }
        }
        Ok(Value::String(lua.create_string(format!(
            "\n\tнет встроенного модуля '{name}'"
        ))?))
    })?;
    let package: Table = lua.globals().get("package")?;
    let searchers: Table = package.get("searchers")?;
    let insert: Function = lua.globals().get::<Table>("table")?.get("insert")?;
    insert.call::<()>((searchers, 2, searcher))
}

// #компиляция — проверка Lua-кода без запуска игры

/// Имена, которые движок даёт в таблице `ic` (функции + колбэки + настройки).
const API: [&str; 11] = [
    "rect",
    "key_down",
    "key_pressed",
    "screen",
    "time",
    "camera",
    "quit",
    "title",
    "load",
    "update",
    "fixed_update",
];

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub is_error: bool,
    /// Уже содержит «файл:строка: …».
    pub message: String,
}

/// Проверяет все .lua: ошибки синтаксиса (это настоящий разбор кода Lua) и подозрительные вызовы `ic.xxx()`.
pub fn check_sources(files: &[(String, Vec<u8>)]) -> Vec<Diagnostic> {
    let lua = Lua::new();
    let mut out = Vec::new();
    for (path, data) in files {
        if !path.ends_with(".lua") {
            continue;
        }
        match lua
            .load(data.as_slice())
            .set_name(format!("@{path}"))
            .into_function()
        {
            Ok(_) => out.extend(api_warnings(path, &String::from_utf8_lossy(data))),
            Err(e) => out.push(Diagnostic {
                is_error: true,
                message: e.to_string(),
            }),
        }
    }
    out
}

/// Ищет вызовы `ic.имя(...)`, которых нет в API и которые игра не определяла сама.
fn api_warnings(path: &str, src: &str) -> Vec<Diagnostic> {
    let b = src.as_bytes();
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut defined: Vec<String> = Vec::new();
    let mut calls: Vec<(usize, String)> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            if src[i..].starts_with("--[[") {
                i = src[i..].find("]]").map(|e| i + e + 2).unwrap_or(b.len());
            } else {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
        } else if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && is_id(b[i]) {
                i += 1;
            }
            let after_dot = start > 0 && b[start - 1] == b'.';
            if &src[start..i] == "ic" && !after_dot && b.get(i) == Some(&b'.') {
                let n0 = i + 1;
                let mut n1 = n0;
                while n1 < b.len() && is_id(b[n1]) {
                    n1 += 1;
                }
                if n1 > n0 {
                    let name = src[n0..n1].to_string();
                    let mut k = n1;
                    while k < b.len() && (b[k] == b' ' || b[k] == b'\t') {
                        k += 1;
                    }
                    let next = b.get(k).copied();
                    let after = b.get(k + 1).copied();
                    let is_fn_def = src[..start].trim_end().ends_with("function");
                    if is_fn_def || (next == Some(b'=') && after != Some(b'=')) {
                        defined.push(name);
                    } else if next == Some(b'(') {
                        calls.push((start, name));
                    }
                    i = n1;
                }
            }
        } else {
            i += 1;
        }
    }
    calls
        .into_iter()
        .filter(|(_, n)| !API.contains(&n.as_str()) && !defined.contains(n))
        .map(|(pos, n)| {
            let line = src[..pos].matches('\n').count() + 1;
            Diagnostic {
                is_error: false,
                message: format!("{path}:{line}: неизвестная функция ic.{n} (опечатка?)"),
            }
        })
        .collect()
}

fn fatal(msg: &str) {
    eprintln!("\n[IC] Ошибка в Lua-скрипте:\n{msg}\n\nНажми Enter, чтобы закрыть...");
    if cfg!(windows) {
        // без консоли ошибку иначе не увидеть — пишем рядом с программой
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        {
            let _ = std::fs::write(dir.join("ic-error.log"), msg);
        }
    }
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
}

pub struct LuaGame {
    lua: Lua,
    shared: Rc<RefCell<Shared>>,
    dead: bool,
}

impl LuaGame {
    /// Загружает и выполняет скрипт (верхний уровень). Колбэки вызываются позже.
    pub fn new(script: &Path) -> Result<Self, String> {
        let src = std::fs::read_to_string(script)
            .map_err(|e| format!("не могу прочитать {}: {e}", script.display()))?;

        let lua = Lua::new();
        let shared = Rc::new(RefCell::new(Shared::default()));
        register_api(&lua, &shared).map_err(|e| e.to_string())?;

        // require("module") ищет рядом со скриптом
        let dir = script
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let package: Table = lua.globals().get("package").map_err(|e| e.to_string())?;
        package
            .set(
                "path",
                format!("{d}/?.lua;{d}/?/init.lua", d = dir.display()),
            )
            .map_err(|e| e.to_string())?;

        let name = format!(
            "@{}",
            script
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("main.lua")
        );
        lua.load(src.as_str())
            .set_name(name)
            .exec()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            lua,
            shared,
            dead: false,
        })
    }

    /// Загружает игру из файлов, вшитых в собранный exe (см. ic-build).
    pub fn from_files(files: HashMap<String, Vec<u8>>) -> Result<Self, String> {
        let main = files
            .get("main.lua")
            .cloned()
            .ok_or_else(|| "в сборке нет main.lua".to_string())?;

        let lua = Lua::new();
        let shared = Rc::new(RefCell::new(Shared::default()));
        register_api(&lua, &shared).map_err(|e| e.to_string())?;
        install_embedded_searcher(&lua, Rc::new(files)).map_err(|e| e.to_string())?;
        lua.load(main.as_slice())
            .set_name("@main.lua")
            .exec()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            lua,
            shared,
            dead: false,
        })
    }

    /// Заголовок, если игра задала `ic.title` (иначе берём из ic.toml).
    pub fn title_opt(&self) -> Option<String> {
        self.lua
            .globals()
            .get::<Table>("ic")
            .ok()
            .and_then(|t| t.get::<String>("title").ok())
    }

    /// Заголовок окна из `ic.title` (или «IC Game»).
    pub fn title(&self) -> String {
        self.title_opt().unwrap_or_else(|| "IC Game".to_string())
    }

    fn sync_in(&self, ctx: &mut Context) {
        let mut sh = self.shared.borrow_mut();
        for k in Key::ALL {
            sh.down[k as usize] = ctx.input.is_down(k);
            sh.pressed[k as usize] = ctx.input.just_pressed(k);
        }
        sh.screen = (ctx.screen_size.x, ctx.screen_size.y);
        sh.elapsed = ctx.time.elapsed;
        sh.camera = (ctx.camera.center.x, ctx.camera.center.y, ctx.camera.height);
        // без копирования: меняем местами буферы спрайтов
        std::mem::swap(&mut sh.sprites, &mut ctx.draw.sprites);
    }

    fn sync_out(&self, ctx: &mut Context) {
        let mut sh = self.shared.borrow_mut();
        std::mem::swap(&mut sh.sprites, &mut ctx.draw.sprites);
        ctx.camera.center = Vec2::new(sh.camera.0, sh.camera.1);
        ctx.camera.height = sh.camera.2;
        if sh.quit {
            ctx.quit();
        }
    }

    fn call(&mut self, ctx: &mut Context, name: &str, dt: Option<f32>) {
        if self.dead {
            return;
        }
        let Ok(ic) = self.lua.globals().get::<Table>("ic") else {
            return;
        };
        let Ok(f) = ic.get::<Function>(name) else {
            return;
        };

        self.sync_in(ctx);
        let res = match dt {
            Some(d) => f.call::<()>(d),
            None => f.call::<()>(()),
        };
        self.sync_out(ctx);

        if let Err(e) = res {
            fatal(&format!("в ic.{name}: {e}"));
            self.dead = true;
            ctx.quit();
        }
    }
}

impl Game for LuaGame {
    fn init(&mut self, ctx: &mut Context) {
        self.call(ctx, "load", None);
    }
    fn fixed_update(&mut self, ctx: &mut Context) {
        let dt = ctx.time.fixed_delta;
        self.call(ctx, "fixed_update", Some(dt));
    }
    fn update(&mut self, ctx: &mut Context) {
        let dt = ctx.time.delta;
        self.call(ctx, "update", Some(dt));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ic_core::Engine;

    #[test]
    fn lua_draws_and_quits() {
        let dir = std::env::temp_dir().join("ic_lua_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("main.lua");
        std::fs::write(
            &path,
            r#"
            ic.title = "test"
            local n = 0
            function ic.update(dt)
                n = n + 1
                ic.rect(0, 0, 1, 1, 1, 0, 0)
                if n == 3 then ic.quit() end
            end
            "#,
        )
        .unwrap();

        let game = LuaGame::new(&path).unwrap();
        assert_eq!(game.title(), "test");

        let mut e = Engine::new(game, 60);
        e.start();
        for _ in 0..100 {
            if e.should_quit() {
                break;
            }
            e.frame();
        }
        assert!(e.should_quit());
        assert_eq!(e.ctx.draw.sprites.len(), 1);
    }
}

#[cfg(test)]
mod check_tests {
    use super::*;

    fn one(src: &str) -> Vec<Diagnostic> {
        check_sources(&[("main.lua".to_string(), src.as_bytes().to_vec())])
    }

    #[test]
    fn syntax_error_is_reported() {
        let d = one("local x = = 1");
        assert!(d.iter().any(|x| x.is_error));
    }

    #[test]
    fn valid_code_has_no_diagnostics() {
        assert!(one("function ic.update(dt)\n  ic.rect(0,0,1,1,1,1,1)\nend").is_empty());
    }

    #[test]
    fn unknown_api_call_is_a_warning() {
        let d =
            one("function ic.update(dt)\n  ic.rectt(0,0,1,1,1,1,1)\n  ic.rect(0,0,1,1,1,1,1)\nend");
        assert_eq!(d.len(), 1);
        assert!(!d[0].is_error);
        assert!(d[0].message.contains("ic.rectt") && d[0].message.contains(":2:"));
    }

    #[test]
    fn user_defined_ic_function_is_fine() {
        assert!(one("function ic.helper() end\nic.helper()").is_empty());
    }
}

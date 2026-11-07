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
use mlua::{Function, Lua, Table};
use std::{cell::RefCell, path::Path, rc::Rc};

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

fn fatal(msg: &str) {
    eprintln!("\n[IC] Ошибка в Lua-скрипте:\n{msg}\n\nНажми Enter, чтобы закрыть...");
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

    /// Заголовок окна из `ic.title`.
    pub fn title(&self) -> String {
        self.lua
            .globals()
            .get::<Table>("ic")
            .ok()
            .and_then(|t| t.get::<String>("title").ok())
            .unwrap_or_else(|| "IC Game".to_string())
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

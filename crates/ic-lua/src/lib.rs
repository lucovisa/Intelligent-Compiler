#![allow(clippy::type_complexity)]

mod atlas;

use atlas::AtlasData;
use ic_core::{AtlasImage, Context, Game, Key, Sprite, Vec2, KEY_COUNT};
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
    mouse: (f32, f32),
    mouse_down: [bool; 3],
    mouse_pressed: [bool; 3],
    wheel: f32,
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

fn mouse_button(v: &Value) -> usize {
    match v {
        Value::Integer(i) => (*i as usize).saturating_sub(1).min(2),
        Value::Number(n) => (*n as usize).saturating_sub(1).min(2),
        Value::String(s) => match s.to_str().map(|s| s.to_ascii_lowercase()).as_deref() {
            Ok("right") => 1,
            Ok("middle") => 2,
            _ => 0,
        },
        _ => 0,
    }
}

type Reader = Rc<dyn Fn(&str) -> Option<String>>;

const SCENE_RUNTIME: &str = include_str!("scene_runtime.lua");

fn register_api(
    lua: &Lua,
    shared: &Rc<RefCell<Shared>>,
    atlas: &Rc<AtlasData>,
    reader: Reader,
) -> mlua::Result<()> {
    let ic = lua.create_table()?;

    // Чтение файлов проекта для сцен
    ic.set(
        "_read",
        lua.create_function(move |_, path: String| Ok(reader(&path)))?,
    )?;

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
                    uv: [0.0; 4],
                });
                Ok(())
            },
        )?,
    )?;

    let (sh, at) = (shared.clone(), atlas.clone());
    ic.set(
        "image",
        lua.create_function(
            move |_,
                  a: (
                String,
                f32,
                f32,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
            )| {
                let (name, x, y, w, h, rot, r, g, b, alpha) = a;
                let info = at.images.get(&name).ok_or_else(|| {
                    mlua::Error::RuntimeError(format!(
                        "нет картинки '{name}' (положи PNG в проект)"
                    ))
                })?;
                sh.borrow_mut().sprites.push(Sprite {
                    pos: Vec2::new(x, y),
                    size: Vec2::new(w.unwrap_or(info.w as f32), h.unwrap_or(info.h as f32)),
                    rotation: rot.unwrap_or(0.0),
                    color: [
                        r.unwrap_or(1.0),
                        g.unwrap_or(1.0),
                        b.unwrap_or(1.0),
                        alpha.unwrap_or(1.0),
                    ],
                    uv: info.uv,
                });
                Ok(())
            },
        )?,
    )?;

    let at = atlas.clone();
    ic.set(
        "image_size",
        lua.create_function(move |_, name: String| {
            let info = at
                .images
                .get(&name)
                .ok_or_else(|| mlua::Error::RuntimeError(format!("нет картинки '{name}'")))?;
            Ok((info.w, info.h))
        })?,
    )?;

    let (sh, at) = (shared.clone(), atlas.clone());
    ic.set(
        "text",
        lua.create_function(
            move |_,
                  a: (
                String,
                f32,
                f32,
                f32,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<String>,
            )| {
                let (text, x, y, size, r, g, b, alpha, align) = a;
                let (quads, width) = atlas::layout(&at, &text, size);
                let start = match align.as_deref() {
                    Some("left") => x,
                    Some("right") => x - width,
                    _ => x - width / 2.0,
                };
                let color = [
                    r.unwrap_or(1.0),
                    g.unwrap_or(1.0),
                    b.unwrap_or(1.0),
                    alpha.unwrap_or(1.0),
                ];
                let mut sh = sh.borrow_mut();
                for q in quads {
                    sh.sprites.push(Sprite {
                        pos: Vec2::new(start + q.dx, y + q.dy),
                        size: Vec2::new(q.w, q.h),
                        rotation: 0.0,
                        color,
                        uv: q.uv,
                    });
                }
                Ok(width)
            },
        )?,
    )?;

    let at = atlas.clone();
    ic.set(
        "text_width",
        lua.create_function(move |_, (text, size): (String, f32)| {
            Ok(atlas::layout(&at, &text, size).1)
        })?,
    )?;

    let sh = shared.clone();
    ic.set(
        "mouse_screen",
        lua.create_function(move |_, ()| Ok(sh.borrow().mouse))?,
    )?;

    let sh = shared.clone();
    ic.set(
        "mouse",
        lua.create_function(move |_, ()| {
            let s = sh.borrow();
            let (sw, sh_) = (s.screen.0.max(1.0), s.screen.1.max(1.0));
            let half_y = s.camera.2 * 0.5;
            let half_x = half_y * sw / sh_;
            let x = s.camera.0 + (s.mouse.0 / sw - 0.5) * 2.0 * half_x;
            let y = s.camera.1 - (s.mouse.1 / sh_ - 0.5) * 2.0 * half_y;
            Ok((x, y))
        })?,
    )?;

    let sh = shared.clone();
    ic.set(
        "mouse_down",
        lua.create_function(move |_, b: Option<Value>| {
            let i = b.as_ref().map_or(0, mouse_button);
            Ok(sh.borrow().mouse_down[i])
        })?,
    )?;

    let sh = shared.clone();
    ic.set(
        "mouse_pressed",
        lua.create_function(move |_, b: Option<Value>| {
            let i = b.as_ref().map_or(0, mouse_button);
            Ok(sh.borrow().mouse_pressed[i])
        })?,
    )?;

    let sh = shared.clone();
    ic.set(
        "wheel",
        lua.create_function(move |_, ()| Ok(sh.borrow().wheel))?,
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

    lua.globals().set("ic", ic)?;
    lua.load(SCENE_RUNTIME).set_name("=ic-scene").exec()
}

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

const API: [&str; 24] = [
    "node",
    "world",
    "scene_load",
    "scene_unload",
    "rect",
    "image",
    "image_size",
    "text",
    "text_width",
    "mouse",
    "mouse_screen",
    "mouse_down",
    "mouse_pressed",
    "wheel",
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
    pub message: String,
}

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

// Значок окна из PNG: RGBA, не крупнее 256×256
pub fn decode_icon(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let (w, h, rgba) = atlas::decode_png(bytes).ok()?;
    if w <= 256 && h <= 256 {
        return Some((rgba, w, h));
    }
    let k = (w.max(h) as f32 / 256.0).ceil() as u32;
    let (nw, nh) = ((w / k).max(1), (h / k).max(1));
    let mut out = Vec::with_capacity((nw * nh * 4) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let o = (((y * k) * w + x * k) * 4) as usize;
            out.extend_from_slice(&rgba[o..o + 4]);
        }
    }
    Some((out, nw, nh))
}

// Компиляция Lua в байт-код (Lua 5.4): в игру попадает уже скомпилированный код, без исходников
pub fn compile_bytecode(files: &[(String, Vec<u8>)]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let lua = Lua::new();
    let mut out = Vec::with_capacity(files.len());
    for (path, data) in files {
        if !path.ends_with(".lua") {
            out.push((path.clone(), data.clone()));
            continue;
        }
        let function = lua
            .load(data.as_slice())
            .set_name(format!("@{path}"))
            .into_function()
            .map_err(|e| e.to_string())?;
        // Отладочные данные оставляем: номера строк в ошибках не пропадают
        out.push((path.clone(), function.dump(false)));
    }
    Ok(out)
}

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
    atlas_image: Option<AtlasImage>,
}

impl LuaGame {
    pub fn new(script: &Path) -> Result<Self, String> {
        let src = std::fs::read_to_string(script)
            .map_err(|e| format!("не могу прочитать {}: {e}", script.display()))?;

        let lua = Lua::new();
        let shared = Rc::new(RefCell::new(Shared::default()));
        let dir = script
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let built = atlas::build(&atlas::collect_disk_images(dir));
        for w in &built.warnings {
            eprintln!("[IC] {w}");
        }
        let atlas_data = Rc::new(built.data);
        let root = dir.to_path_buf();
        let reader: Reader = Rc::new(move |p: &str| {
            let rel = Path::new(p);
            if rel.is_absolute()
                || rel
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return None;
            }
            std::fs::read_to_string(root.join(rel)).ok()
        });
        register_api(&lua, &shared, &atlas_data, reader).map_err(|e| e.to_string())?;

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
            atlas_image: Some(built.image),
        })
    }

    pub fn from_files(files: HashMap<String, Vec<u8>>) -> Result<Self, String> {
        let main = files
            .get("main.lua")
            .cloned()
            .ok_or_else(|| "в сборке нет main.lua".to_string())?;

        let lua = Lua::new();
        let shared = Rc::new(RefCell::new(Shared::default()));
        let mut images: Vec<(String, Vec<u8>)> = files
            .iter()
            .filter(|(k, _)| k.to_ascii_lowercase().ends_with(".png"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        images.sort_by(|a, b| a.0.cmp(&b.0));
        let built = atlas::build(&images);
        for w in &built.warnings {
            eprintln!("[IC] {w}");
        }
        let atlas_data = Rc::new(built.data);
        let files = Rc::new(files);
        let embedded = files.clone();
        let reader: Reader = Rc::new(move |p: &str| {
            embedded
                .get(p)
                .map(|b| String::from_utf8_lossy(b).into_owned())
        });
        register_api(&lua, &shared, &atlas_data, reader).map_err(|e| e.to_string())?;
        install_embedded_searcher(&lua, files).map_err(|e| e.to_string())?;
        lua.load(main.as_slice())
            .set_name("@main.lua")
            .exec()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            lua,
            shared,
            dead: false,
            atlas_image: Some(built.image),
        })
    }

    pub fn title_opt(&self) -> Option<String> {
        self.lua
            .globals()
            .get::<Table>("ic")
            .ok()
            .and_then(|t| t.get::<String>("title").ok())
    }

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
        sh.mouse = (ctx.input.mouse_pos.x, ctx.input.mouse_pos.y);
        sh.mouse_down = ctx.input.mouse_down;
        sh.mouse_pressed = ctx.input.mouse_pressed;
        sh.wheel = ctx.input.wheel;
        sh.elapsed = ctx.time.elapsed;
        sh.camera = (ctx.camera.center.x, ctx.camera.center.y, ctx.camera.height);
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
        // Сцена: загрузка, физика и рисование идут до кода игры
        let hook = match name {
            "load" => "_scene_boot",
            "fixed_update" => "_scene_fixed",
            "update" => "_scene_draw",
            _ => "",
        };
        let hook_fn = ic.get::<Function>(hook).ok();
        let f = ic.get::<Function>(name).ok();
        if hook_fn.is_none() && f.is_none() {
            return;
        }

        self.sync_in(ctx);
        let run = |f: &Function| match dt {
            Some(d) => f.call::<()>(d),
            None => f.call::<()>(()),
        };
        let mut res = Ok(());
        if let Some(h) = &hook_fn {
            res = run(h);
        }
        if res.is_ok() {
            if let Some(f) = &f {
                res = run(f);
            }
        }
        self.sync_out(ctx);

        if let Err(e) = res {
            fatal(&format!("в ic.{name}: {e}"));
            self.dead = true;
            ctx.quit();
        }
    }
}

impl Game for LuaGame {
    fn atlas(&mut self) -> Option<AtlasImage> {
        self.atlas_image.take()
    }
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

    fn scene_game(main: &str, scene: &str) -> Engine<LuaGame> {
        let mut files = HashMap::new();
        files.insert("main.lua".to_string(), main.as_bytes().to_vec());
        files.insert("main.ic".to_string(), scene.as_bytes().to_vec());
        let game = LuaGame::from_files(files).unwrap();
        let mut e = Engine::new(game, 60);
        e.start();
        e
    }

    const SCENE: &str = "ic-scene|5\n\
node|ColorRect|Floor|0|-100|400|20|255|255|255|0|255|1|0||0|24||900|0\n\
node|Body|Ball|0|50|20|20|255|0|0|0|255|1|0||0|24||900|0\n\
node|ColorRect|Parent|100|0|40|40|0|255|0|0|255|1|0||0|24||900|0\n\
node|Label|Kid|10|5|10|10|0|0|255|0|255|1|0||Hi|24|Parent|900|0\n\
node|FPS|Counter|-200|200|10|10|255|255|255|0|255|1|0||0|24||900|0\n";

    #[test]
    fn main_scene_loads_and_draws_without_lua_code() {
        let mut e = scene_game("", SCENE);
        for _ in 0..3 {
            e.frame();
        }
        assert!(e.ctx.draw.sprites.len() >= 3);
    }

    #[test]
    fn body_falls_and_rests_on_solid() {
        let scene = SCENE.replace("ColorRect|Floor", "Solid|Floor");
        let mut e = scene_game("", &scene);
        e.frame();
        {
            let ic: mlua::Table = e.game.lua.globals().get("ic").unwrap();
            let step: mlua::Function = ic.get("_scene_fixed").unwrap();
            for _ in 0..300 {
                step.call::<()>(1.0_f32 / 60.0).unwrap();
            }
        }
        let ball_y: f32 = {
            let ic: mlua::Table = e.game.lua.globals().get("ic").unwrap();
            let f: mlua::Function = ic.get("node").unwrap();
            let ball: mlua::Table = f.call("Ball").unwrap();
            ball.get("y").unwrap()
        };
        assert!(ball_y < 50.0 && ball_y > -100.0, "y = {ball_y}");
    }

    #[test]
    fn child_follows_parent_position() {
        let mut e = scene_game("", SCENE);
        e.frame();
        let ic: mlua::Table = e.game.lua.globals().get("ic").unwrap();
        let node: mlua::Function = ic.get("node").unwrap();
        let world: mlua::Function = ic.get("world").unwrap();
        let kid: mlua::Table = node.call("Kid").unwrap();
        let (x, y): (f32, f32) = world.call(kid).unwrap();
        assert_eq!((x, y), (110.0, 5.0));
    }

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

    #[test]
    fn bytecode_runs_and_keeps_line_numbers() {
        let src = vec![(
            "main.lua".to_string(),
            b"function ic.load() end\nfunction ic.update(dt)\n    local x = nil\n    return x.field\nend\n".to_vec(),
        )];
        let compiled = compile_bytecode(&src).unwrap();
        assert!(compiled[0].1.starts_with(b"\x1bLua"));
        let mut files = HashMap::new();
        files.insert("main.lua".to_string(), compiled[0].1.clone());
        let game = LuaGame::from_files(files);
        assert!(game.is_ok(), "{:?}", game.err());
    }

    #[test]
    fn bytecode_keeps_non_lua_files_and_reports_syntax_errors() {
        let ok = compile_bytecode(&[("a.txt".to_string(), b"hello".to_vec())]).unwrap();
        assert_eq!(ok[0].1, b"hello");
        let err = compile_bytecode(&[("b.lua".to_string(), b"local = =".to_vec())]);
        assert!(err.is_err());
    }
}

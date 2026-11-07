//! `ic` — рантайм движка.
//!   Режим А (по умолчанию): игры на Lua, ничего скачивать не нужно.
//!   Режим Б (по желанию):   игры на Rust, компилятор качается локально в ./.ic/
mod rust_mode;

use ic_lua::LuaGame;
use ic_render::{run, Config};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::exit,
};

const HELP: &str = "\
IC — игровой движок

  ic                         запустить main.lua
  ic <папка|файл.lua>        запустить указанную игру
  ic new <имя>               новая игра на Lua (по умолчанию)
  ic new <имя> --rust        новая игра на Rust (режим Б)
  ic build [папка] [-o файл] собрать игру в один самостоятельный exe
  ic setup                   меню выбора режима
  ic rust setup              скачать Rust в ./.ic/ (режим Б)
  ic rust build | run        собрать / запустить Rust-игру
";

const LUA_TEMPLATE: &str = r#"ic.title = "My IC Game"

local x, y = 0, 0

function ic.update(dt)
    if ic.key_pressed("escape") then ic.quit() end

    if ic.key_down("d") then x = x + 300 * dt end
    if ic.key_down("a") then x = x - 300 * dt end
    if ic.key_down("w") then y = y + 300 * dt end
    if ic.key_down("s") then y = y - 300 * dt end

    ic.rect(x, y, 60, 60, 1, 0.5, 0.1, 1, ic.time())
end
"#;

const RUST_MAIN: &str = r#"use ic_render::{run, Config, Context, Game, Key, Vec2};

struct MyGame {
    pos: Vec2,
}

impl Game for MyGame {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.just_pressed(Key::Escape) {
            ctx.quit();
        }
        let speed = 300.0 * ctx.time.delta;
        if ctx.input.is_down(Key::D) { self.pos.x += speed; }
        if ctx.input.is_down(Key::A) { self.pos.x -= speed; }
        if ctx.input.is_down(Key::W) { self.pos.y += speed; }
        if ctx.input.is_down(Key::S) { self.pos.y -= speed; }
        ctx.draw.rect(self.pos, Vec2::new(60.0, 60.0), [1.0, 0.5, 0.1, 1.0]);
    }
}

fn main() {
    run(MyGame { pos: Vec2::ZERO }, Config { title: "My IC Game", ..Default::default() });
}
"#;

fn die(msg: &str) -> ! {
    eprintln!("[IC] {msg}\n\nНажми Enter, чтобы закрыть...");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    exit(1)
}

fn or_die<T>(r: Result<T, String>) -> T {
    r.unwrap_or_else(|e| die(&e))
}

fn ask(prompt: &str) -> String {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    s.trim().to_string()
}

fn write_if_absent(path: &Path, content: &str) -> Result<(), String> {
    if path.exists() {
        println!("уже есть: {}", path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("создан: {}", path.display());
    Ok(())
}

fn init_lua(dir: &Path) {
    or_die(write_if_absent(&dir.join("main.lua"), LUA_TEMPLATE));
    println!("Запуск: ic {}", dir.display());
}

fn init_rust(dir: &Path) {
    let raw = if dir == Path::new(".") {
        env::current_dir()
            .ok()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
    } else {
        dir.file_name().map(|n| n.to_string_lossy().into_owned())
    };
    let mut name: String = raw
        .unwrap_or_default()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        name = format!("game_{name}");
    }

    let manifest = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [dependencies]\nic-render = {{ git = \"https://github.com/lucovisa/IC\" }}\n\n\
         [profile.dev]\nopt-level = 1\n\n[profile.dev.package.\"*\"]\nopt-level = 3\n\n\
         [profile.release]\nlto = \"fat\"\ncodegen-units = 1\n"
    );
    or_die(write_if_absent(&dir.join("Cargo.toml"), &manifest));
    or_die(write_if_absent(&dir.join("src/main.rs"), RUST_MAIN));
    or_die(write_if_absent(&dir.join(".gitignore"), "/target\n/.ic\n"));
}

fn setup_menu() {
    println!(
        "IC — выбор режима для этой папки\n\n\
         \x20 1) Lua   (по умолчанию) — ничего скачивать не нужно\n\
         \x20 2) Rust  (по желанию)   — скачает компилятор (~1 ГБ) в ./.ic/ рядом\n"
    );
    match ask("Выбор [1]: ").as_str() {
        "2" => {
            init_rust(Path::new("."));
            if ask("Скачать Rust сейчас в ./.ic/ ? [y/N]: ").eq_ignore_ascii_case("y")
            {
                or_die(rust_mode::setup(Path::new(".")));
            } else {
                println!("Позже: ic rust setup");
            }
        }
        _ => {
            init_lua(Path::new("."));
            println!("Готово. Запусти ic ещё раз — игра стартует.");
        }
    }
    let _ = ask("\nНажми Enter, чтобы закрыть...");
}

fn resolve(arg: &str) -> PathBuf {
    let p = PathBuf::from(arg);
    if p.is_dir() {
        let m = p.join("main.lua");
        if m.is_file() {
            return m;
        }
        die(&format!("в папке '{arg}' нет main.lua"));
    }
    if p.is_file() {
        return p;
    }
    die(&format!("не найдено: {arg}"))
}

fn find_default() -> Option<PathBuf> {
    let local = PathBuf::from("main.lua");
    if local.is_file() {
        return Some(local);
    }
    let exe_dir = env::current_exe().ok()?.parent()?.to_path_buf();
    let m = exe_dir.join("main.lua");
    m.is_file().then_some(m)
}

fn start(script: PathBuf) {
    // рабочая папка = папка игры, чтобы относительные пути (assets/…) работали
    let file = script
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("main.lua"));
    if let Some(dir) = script.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = env::set_current_dir(dir) {
            die(&format!("не могу открыть папку игры: {e}"));
        }
    }
    let game = or_die(LuaGame::new(&file));
    let title: &'static str = Box::leak(game.title().into_boxed_str());
    run(
        game,
        Config {
            title,
            ..Default::default()
        },
    );
}

fn build_cmd(dir: &Path, out: Option<&Path>) {
    let runtime = or_die(env::current_exe().map_err(|e| e.to_string()));
    let name = fs::canonicalize(dir)
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "game".to_string());
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name
    };
    let out = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| dir.join("build").join(file));
    let n = or_die(ic_build::pack(dir, &runtime, &out));
    println!("Собрано: {} ({n} файлов)", out.display());
}

fn main() {
    // собранная игра: внутри этого exe уже лежат скрипты — просто запускаем их
    if let Some(files) = env::current_exe()
        .ok()
        .and_then(|e| ic_build::read_embedded(&e))
    {
        let game = or_die(LuaGame::from_files(files));
        let title: &'static str = Box::leak(game.title().into_boxed_str());
        run(
            game,
            Config {
                title,
                ..Default::default()
            },
        );
        return;
    }

    let args: Vec<String> = env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        [] => match find_default() {
            Some(p) => start(p),
            None => setup_menu(),
        },
        ["help" | "-h" | "--help"] => print!("{HELP}"),
        ["setup"] => setup_menu(),
        ["new"] => die("ic new <имя> [--rust]"),
        ["new", name, "--rust"] | ["new", "--rust", name] => {
            init_rust(Path::new(name));
            println!("Дальше: cd {name} && ic rust setup");
        }
        ["new", name] => init_lua(Path::new(name)),
        ["rust", "setup"] => or_die(rust_mode::setup(Path::new("."))),
        ["rust", "build"] => or_die(rust_mode::cargo(Path::new("."), &["build", "--release"])),
        ["rust", "run"] => or_die(rust_mode::cargo(Path::new("."), &["run", "--release"])),
        ["build"] => build_cmd(Path::new("."), None),
        ["build", dir] => build_cmd(Path::new(dir), None),
        ["build", dir, "-o", out] => build_cmd(Path::new(dir), Some(Path::new(out))),
        [path] => start(resolve(path)),
        _ => print!("{HELP}"),
    }
}

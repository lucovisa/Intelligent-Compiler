#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod rust_mode;

use ic_build::scaffold::{self, Dimension, Generator, Kind, NewProject};
use ic_lua::LuaGame;
use ic_render::run;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::exit,
    time::Instant,
};

static RUNTIME: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.bin"));

const HELP: &str = "\
IC — игровой движок

  ic                         открыть окно программы (редактор проектов)
  ic <папка|файл.lua>        запустить указанную игру
  ic new <имя> [параметры]   новый проект на Lua
        --app | --game       программа или игра (по умолчанию игра)
        --dim 2d|3d|custom   тип игры
        --label <текст>      название своего типа
        --add scene,fps,physics,camera
                             добавить заготовки
        --no-default-code    не вставлять демо-код
        --rust               проект на Rust (режим Б)
  ic check [папка]           проверить Lua-код без запуска
  ic build [папка] [-o файл] [--target windows|linux|macos]
                             проверить и собрать игру в один самостоятельный файл
  ic setup                   меню выбора режима
  ic rust setup              скачать Rust в ./.ic/ (режим Б)
  ic rust build | run        собрать / запустить Rust-игру
";

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
    if cfg!(windows) {
        if let Some(dir) = env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
        {
            let _ = fs::write(dir.join("ic-error.log"), msg);
        }
    }
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

fn init_lua_with(dir: &Path, opts: &NewProject) {
    let made = or_die(scaffold::create(dir, opts));
    for f in &made {
        println!("создан: {f}");
    }
    if opts.dimension == Dimension::D3 {
        println!("3D пока в разработке: проект помечен как 3D, но рисует в 2D.");
    }
    println!("Запуск: ic {}", dir.display());
}

fn init_lua(dir: &Path) {
    init_lua_with(dir, &NewProject::default());
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
            println!("Готово. Запустить игру: ic .");
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

fn start(script: PathBuf) {
    let file = script
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("main.lua"));
    if let Some(dir) = script.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = env::set_current_dir(dir) {
            die(&format!("не могу открыть папку игры: {e}"));
        }
    }
    let (settings, warnings) = ic_build::ProjectSettings::load(Path::new("."));
    for w in warnings {
        eprintln!("[IC] {w}");
    }
    let game = or_die(LuaGame::new(&file));
    let cfg = ic_player::make_config(&settings, game.title_opt());
    run(game, cfg);
}

enum Runtime {
    Embedded(&'static [u8]),
    File(PathBuf),
}

fn host_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn resolve_runtime(target: &str) -> Result<(Runtime, &'static str), String> {
    let host = host_os();
    let t = if target.is_empty() || target == "this" {
        host
    } else {
        target
    };
    if !["windows", "linux", "macos"].contains(&t) {
        return Err(format!(
            "неизвестная платформа «{t}» (windows, linux, macos)"
        ));
    }
    let ext = if t == "windows" { ".exe" } else { "" };
    let me = env::current_exe().map_err(|e| e.to_string())?;
    if t == host {
        if !RUNTIME.is_empty() {
            return Ok((Runtime::Embedded(RUNTIME), ext));
        }
        return Ok((Runtime::File(me), ext));
    }
    let dir = me.parent().map(Path::to_path_buf).unwrap_or_default();
    let p = dir.join("templates").join(format!("ic-{t}{ext}"));
    if p.is_file() {
        Ok((Runtime::File(p), ext))
    } else {
        Err(format!(
            "нет шаблона {} — положи туда файл ic для {t} (из релиза для этой системы)",
            p.display()
        ))
    }
}

fn compile(dir: &Path, out: Option<&Path>, pack: bool, target: Option<&str>) {
    // Проект и явный путь вывода выбраны пользователем
    ic_build::confine::allow(dir);
    if let Some(parent) = out.and_then(Path::parent) {
        ic_build::confine::allow(parent);
    }
    let t0 = Instant::now();
    let total = if pack { 4 } else { 3 };
    let exe = or_die(env::current_exe().map_err(|e| e.to_string()));
    println!(
        "[IC] {}: {}",
        if pack {
            "Сборка"
        } else {
            "Проверка"
        },
        dir.display()
    );

    if !dir.join("main.lua").is_file() {
        println!("[IC] Ошибка: в {} нет main.lua", dir.display());
        exit(1);
    }

    let files = or_die(ic_build::collect(dir, &exe));
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for (rel, path) in &files {
        let data = or_die(fs::read(path).map_err(|e| format!("{}: {e}", path.display())));
        entries.push((rel.clone(), data));
    }
    println!(
        "[1/{total}] Читаю проект ........ файлов: {}",
        entries.len()
    );

    let (proj_settings, toml_warnings) = ic_build::ProjectSettings::load(dir);
    let diags = ic_lua::check_sources(&entries);
    let errors = diags.iter().filter(|d| d.is_error).count();
    for d in &diags {
        println!(
            "  {}: {}",
            if d.is_error {
                "ошибка"
            } else {
                "предупреждение"
            },
            d.message
        );
    }
    for w in &toml_warnings {
        println!("  предупреждение: {w}");
    }
    if errors > 0 {
        println!("[IC] Остановлено: ошибок — {errors}");
        exit(1);
    }
    println!("[2/{total}] Проверяю Lua ........ ok");

    let warnings = diags.len() + toml_warnings.len();
    println!("[3/{total}] Анализ .............. предупреждений: {warnings}");

    if !pack {
        println!("[IC] Готово за {:.2} с", t0.elapsed().as_secs_f32());
        return;
    }

    let name = fs::canonicalize(dir)
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "game".to_string());
    let target_name = target
        .map(str::to_string)
        .unwrap_or_else(|| proj_settings.target.clone());
    let (runtime, ext) = or_die(resolve_runtime(&target_name));
    let file = format!("{name}{ext}");
    let out = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| dir.join("build").join(file));
    let size = or_die(match &runtime {
        Runtime::Embedded(bytes) => ic_build::write_game(&entries, bytes, &out),
        Runtime::File(path) => ic_build::pack_entries(&entries, path, &out),
    });
    println!(
        "[4/{total}] Упаковываю .......... {} ({:.1} МБ)",
        out.display(),
        size as f64 / 1_048_576.0
    );
    println!("[IC] Готово за {:.2} с", t0.elapsed().as_secs_f32());
}

fn build_cmd(dir: &Path, out: Option<&Path>, target: Option<&str>) {
    compile(dir, out, true, target);
}

fn build_args(rest: &[&str]) {
    let mut dir = PathBuf::from(".");
    let mut out: Option<PathBuf> = None;
    let mut target: Option<String> = None;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match *a {
            "-o" => out = it.next().map(|p| PathBuf::from(*p)),
            "--target" => target = it.next().map(|t| t.to_string()),
            other => dir = PathBuf::from(other),
        }
    }
    build_cmd(&dir, out.as_deref(), target.as_deref());
}

fn check_cmd(dir: &Path) {
    compile(dir, None, false, None);
}

fn new_args(rest: &[&str]) {
    let usage = "ic new <имя> [--app|--game] [--dim 2d|3d|custom] [--label текст] [--add scene,fps,physics,camera] [--no-default-code] [--rust]";
    let mut name: Option<&str> = None;
    let mut rust = false;
    let mut opts = NewProject::default();
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match *a {
            "--rust" => rust = true,
            "--app" => opts.kind = Kind::App,
            "--game" => opts.kind = Kind::Game,
            "--no-default-code" => opts.default_code = false,
            "--dim" => match it.next().and_then(|v| Dimension::parse(v)) {
                Some(d) => opts.dimension = d,
                None => die("--dim принимает 2d, 3d или custom"),
            },
            "--label" => opts.label = it.next().map(|s| s.to_string()).unwrap_or_default(),
            "--add" => {
                let list = it.next().copied().unwrap_or("");
                for g in list.split(',').filter(|s| !s.is_empty()) {
                    match Generator::parse(g) {
                        Some(g) => opts.generators.push(g),
                        None => die(&format!(
                            "неизвестная заготовка «{g}» (scene, fps, physics, camera)"
                        )),
                    }
                }
            }
            other => name = Some(other),
        }
    }
    let Some(name) = name else { die(usage) };
    if rust {
        init_rust(Path::new(name));
        println!("Дальше: cd {name} && ic rust setup");
    } else {
        init_lua_with(Path::new(name), &opts);
    }
}

fn confine_caches() {
    or_die(ic_player::confine_caches());
}

#[cfg(windows)]
fn attach_parent_console() {
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(n: u32) -> *mut std::ffi::c_void;
        fn AttachConsole(pid: u32) -> i32;
        fn SetStdHandle(n: u32, h: *mut std::ffi::c_void) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;
    const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;

    unsafe {
        let cur = GetStdHandle(STD_OUTPUT_HANDLE);
        if !cur.is_null() && cur as isize != -1 {
            return;
        }
        if AttachConsole(u32::MAX) == 0 {
            return;
        }
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
            let h = f.as_raw_handle();
            SetStdHandle(STD_OUTPUT_HANDLE, h);
            SetStdHandle(STD_ERROR_HANDLE, h);
            std::mem::forget(f);
        }
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

fn main() {
    confine_caches();
    if env::args().len() > 1 {
        attach_parent_console();
    }

    if let Some(files) = env::current_exe()
        .ok()
        .and_then(|e| ic_build::read_embedded(&e))
    {
        or_die(ic_player::run_embedded(files));
        return;
    }

    let args: Vec<String> = env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        [] => {
            if let Err(e) = ic_gui::run() {
                die(&e.to_string());
            }
        }
        ["help" | "-h" | "--help"] => print!("{HELP}"),
        ["setup"] => setup_menu(),
        ["new", rest @ ..] => new_args(rest),
        ["rust", "setup"] => or_die(rust_mode::setup(Path::new("."))),
        ["rust", "build"] => or_die(rust_mode::cargo(Path::new("."), &["build", "--release"])),
        ["rust", "run"] => or_die(rust_mode::cargo(Path::new("."), &["run", "--release"])),
        ["check"] => check_cmd(Path::new(".")),
        ["check", dir] => check_cmd(Path::new(dir)),
        ["build", rest @ ..] => build_args(rest),
        [path] => start(resolve(path)),
        _ => print!("{HELP}"),
    }
}

#[cfg(test)]
mod tests {
    use ic_build::scaffold::{Generator, DEMO};

    fn check(name: &str, code: &str) {
        let d = ic_lua::check_sources(&[(format!("{name}.lua"), code.as_bytes().to_vec())]);
        let msgs: Vec<String> = d.iter().map(|x| x.message.clone()).collect();
        assert!(d.is_empty(), "{name}: {msgs:?}");
    }

    #[test]
    fn templates_are_valid_lua() {
        check("demo", DEMO);
        for g in Generator::ALL {
            check(g.key(), g.code());
        }
    }
}

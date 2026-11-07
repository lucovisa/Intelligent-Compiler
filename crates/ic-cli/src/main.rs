//! `ic` — CLI движка: new / run / build / check
use std::{
    env, fs,
    path::Path,
    process::{exit, Command},
};

const HELP: &str = "\
IC — Intelligent Compiler (game engine)

  ic new <name>     создать проект
  ic run            собрать и запустить (dev)
  ic build          release-сборка (LTO)
  ic check          fmt + clippy + test
";

const MAIN_TEMPLATE: &str = r#"use ic_render::{run, Config, Context, Game, Key, Vec2};

struct MyGame { pos: Vec2 }

impl Game for MyGame {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.just_pressed(Key::Escape) { ctx.quit(); }
        if ctx.input.is_down(Key::D) { self.pos.x += 300.0 * ctx.time.delta; }
        if ctx.input.is_down(Key::A) { self.pos.x -= 300.0 * ctx.time.delta; }
        ctx.draw.rect(self.pos, Vec2::new(60.0, 60.0), [1.0, 0.5, 0.1, 1.0]);
    }
}

fn main() {
    run(MyGame { pos: Vec2::ZERO }, Config { title: "My IC Game", ..Default::default() });
}
"#;

fn cargo(args: &[&str]) {
    let status = Command::new("cargo")
        .args(args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("не удалось запустить cargo: {e}");
            exit(1)
        });
    if !status.success() {
        exit(status.code().unwrap_or(1));
    }
}

fn new_project(name: &str) {
    let root = Path::new(name);
    if root.exists() {
        eprintln!("'{name}' уже существует");
        exit(1);
    }
    fs::create_dir_all(root.join("src")).expect("mkdir");
    let manifest = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [dependencies]\n# TODO: заменить на git/crates.io, когда опубликуешь\n\
         ic-render = {{ git = \"https://github.com/YOUR_NAME/IC\" }}\n\n\
         [profile.dev]\nopt-level = 1\n\n[profile.release]\nlto = \"fat\"\ncodegen-units = 1\n"
    );
    fs::write(root.join("Cargo.toml"), manifest).expect("write");
    fs::write(root.join("src/main.rs"), MAIN_TEMPLATE).expect("write");
    println!("Проект '{name}' создан. cd {name} && ic run");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("new") => match args.get(1) {
            Some(n) => new_project(n),
            None => {
                eprintln!("ic new <name>");
                exit(1)
            }
        },
        Some("run") => cargo(&["run"]),
        Some("build") => cargo(&["build", "--release"]),
        Some("check") => {
            cargo(&["fmt", "--check"]);
            cargo(&["clippy", "--all-targets", "--", "-D", "warnings"]);
            cargo(&["test"]);
        }
        _ => print!("{HELP}"),
    }
}

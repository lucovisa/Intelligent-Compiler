//! IC — графический интерфейс: редактор проектов, запуск и сборка.
//! Сначала экран «Новый проект» (название + папка), потом редактор кода проекта.
//! Настройки лежат рядом с программой (ic-settings.txt), проекты — в выбранной папке,
//! в систему ничего не пишется.
mod i18n;

use eframe::egui::{self, Color32, FontId, Margin, RichText, Rounding, Stroke};
use i18n::{tr, Lang, K};
use ic_build::ProjectSettings;
use std::{
    env, fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{channel, Receiver, Sender},
    thread::{self, JoinHandle},
};

const ICON_RGBA: &[u8] = include_bytes!("../assets/icon.rgba"); // 256x256

// ───────────────────────── #пути ─────────────────────────

fn exe_dir() -> Option<PathBuf> {
    env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// Сама программа: запуск игр и сборка идут через неё же (`ic <папка>`, `ic build …`).
fn ic_path() -> Option<PathBuf> {
    env::current_exe().ok()
}

/// Относительные пути считаются от папки программы, а не от того места, откуда её запустили.
fn resolve_dir(input: &str) -> PathBuf {
    let p = PathBuf::from(input.trim());
    if p.is_absolute() { p } else { exe_dir().unwrap_or_default().join(p) }
}

/// Кэши шейдеров драйверов (Mesa / NVIDIA) по умолчанию лежат в ~/.cache — переносим рядом.
fn confine_caches() {
    if let Some(dir) = exe_dir() {
        let cache = dir.join("cache");
        let _ = fs::create_dir_all(&cache);
        let set = |k: &str, v: &std::ffi::OsStr| {
            if env::var_os(k).is_none() {
                env::set_var(k, v);
            }
        };
        set("MESA_SHADER_CACHE_DIR", cache.as_os_str());
        set("__GL_SHADER_DISK_CACHE_PATH", cache.as_os_str());
        // потолок размера кэша: драйвер сам вытесняет старое
        set("MESA_SHADER_CACHE_MAX_SIZE", std::ffi::OsStr::new("256M"));
        set("__GL_SHADER_DISK_CACHE_SIZE", std::ffi::OsStr::new("268435456"));
    }
}

fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // без мигающего консольного окна
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

// ───────────────────────── #настройки ─────────────────────────

struct Settings {
    dark: bool,
    lang: Lang,
    recent: Vec<String>,
}

impl Settings {
    fn path() -> PathBuf {
        exe_dir().unwrap_or_default().join("ic-settings.txt")
    }

    fn system_lang() -> Lang {
        let v = env::var("LC_ALL").or_else(|_| env::var("LANG")).unwrap_or_default();
        if v.starts_with("ru") { Lang::Ru } else { Lang::En }
    }

    fn load() -> Self {
        let mut s = Settings { dark: true, lang: Self::system_lang(), recent: Vec::new() };
        if let Ok(text) = fs::read_to_string(Self::path()) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match (k.trim(), v.trim()) {
                        ("theme", v) => s.dark = v != "light",
                        ("lang", v) => {
                            if let Some(l) = Lang::from_code(v) {
                                s.lang = l;
                            }
                        }
                        ("recent", v) if !v.is_empty() => s.recent.push(v.to_string()),
                        _ => {}
                    }
                }
            }
        }
        s
    }

    fn save(&self) {
        let theme = if self.dark { "dark" } else { "light" };
        let mut out = format!("theme={theme}\nlang={}\n", self.lang.code());
        for r in &self.recent {
            out.push_str(&format!("recent={r}\n"));
        }
        let _ = fs::write(Self::path(), out);
    }

    fn push_recent(&mut self, dir: &Path) {
        let d = dir.to_string_lossy().into_owned();
        self.recent.retain(|r| *r != d);
        self.recent.insert(0, d);
        self.recent.truncate(6);
    }
}

// ───────────────────────── #стиль ─────────────────────────
// Спокойная палитра в духе классических редакторов: серые панели, тонкие рамки,
// небольшие скругления, один сдержанный синий акцент.

struct Palette {
    panel: Color32,
    editor: Color32,
    idle: Color32,
    hover: Color32,
    border: Color32,
    accent: Color32,
}

fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            panel: Color32::from_rgb(0x25, 0x25, 0x26),
            editor: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            idle: Color32::from_rgb(0x3A, 0x3D, 0x41),
            hover: Color32::from_rgb(0x45, 0x49, 0x4E),
            border: Color32::from_rgb(0x3C, 0x3C, 0x3C),
            accent: Color32::from_rgb(0x0E, 0x63, 0x9C),
        }
    } else {
        Palette {
            panel: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            editor: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            idle: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            hover: Color32::from_rgb(0xD8, 0xD8, 0xD8),
            border: Color32::from_rgb(0xD0, 0xD0, 0xD0),
            accent: Color32::from_rgb(0x00, 0x78, 0xD4),
        }
    }
}

fn setup_style(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (egui::TextStyle::Heading, FontId::new(22.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Body, FontId::new(15.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Button, FontId::new(15.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Small, FontId::new(12.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Monospace, FontId::new(14.0, egui::FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.interact_size.y = 28.0;

    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.editor;
    v.faint_bg_color = p.idle;
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.inactive.bg_fill = p.idle;
    v.widgets.inactive.weak_bg_fill = p.idle;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.weak_bg_fill = p.accent;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(4.0);
    }
    style.visuals = v;
    ctx.set_style(style);
}

fn button_on(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    ui.add_enabled(on, egui::Button::new(text))
}

fn primary_on(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    let accent = palette(ui.visuals().dark_mode).accent;
    ui.add_enabled(on, egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(accent))
}

fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    button_on(ui, true, text)
}

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    primary_on(ui, true, text)
}

// ───────────────────────── #тема ─────────────────────────
// Одна кнопка: солнце плавно превращается в луну (и обратно) по клику.

fn theme_toggle(ui: &mut egui::Ui, dark: &mut bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 28.0), egui::Sense::click());
    let resp = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = ui.ctx().animate_bool_with_time(resp.id, *dark, 0.25);

    let v = ui.style().interact(&resp).clone();
    let fill = v.weak_bg_fill;
    let col = ui.visuals().text_color();
    let painter = ui.painter().with_clip_rect(rect);

    painter.rect_filled(rect, 4.0, fill);
    let c = rect.center();
    let r = 4.5 + 2.5 * t; // солнце поменьше, луна побольше
    painter.circle_filled(c, r, col);

    // луна: «откусываем» круг цветом кнопки
    if t > 0.01 {
        let bite = egui::vec2(16.0, -16.0) * (1.0 - t) + egui::vec2(3.5, -3.5) * t;
        painter.circle_filled(c + bite, 5.5, fill);
    }
    // лучи солнца, затухают и поворачиваются
    let k = 1.0 - t;
    if k > 0.02 {
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0 + t * 1.4;
            let d = egui::vec2(a.cos(), a.sin());
            painter.line_segment(
                [c + d * (r + 2.5), c + d * (r + 2.5 + 3.0 * k)],
                Stroke::new(1.6, col.gamma_multiply(k)),
            );
        }
    }
    painter.rect_stroke(rect, 4.0, v.bg_stroke);

    if resp.clicked() {
        *dark = !*dark;
        true
    } else {
        false
    }
}

// ───────────────────────── #редактор ─────────────────────────

const LUA_KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// Простая подсветка Lua: комментарии, строки, числа, ключевые слова, `ic`.
fn highlight(src: &str, font: FontId, dark: bool, lua: bool) -> egui::text::LayoutJob {
    let rgb = Color32::from_rgb;
    let (kw, st, cm, nm, bi, tx) = if dark {
        (rgb(0xC5, 0x86, 0xC0), rgb(0xCE, 0x91, 0x78), rgb(0x6A, 0x99, 0x55), rgb(0xB5, 0xCE, 0xA8), rgb(0x4F, 0xC1, 0xFF), rgb(0xD4, 0xD4, 0xD4))
    } else {
        (rgb(0xAF, 0x00, 0xDB), rgb(0xA3, 0x15, 0x15), rgb(0x00, 0x80, 0x00), rgb(0x09, 0x86, 0x58), rgb(0x00, 0x70, 0xC1), rgb(0x1F, 0x1F, 0x1F))
    };

    let mut job = egui::text::LayoutJob::default();
    if !lua {
        // не Lua (например ic.toml) — без подсветки
        job.append(src, 0.0, egui::text::TextFormat { font_id: font, color: tx, ..Default::default() });
        return job;
    }
    let mut push = |s: &str, color: Color32| {
        if !s.is_empty() {
            job.append(s, 0.0, egui::text::TextFormat { font_id: font.clone(), color, ..Default::default() });
        }
    };
    let special = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'"' || c == b'\'' || c == b'-';

    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let c = b[i];
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            if src[i..].starts_with("--[[") {
                i = src[i..].find("]]").map(|e| i + e + 2).unwrap_or(b.len());
            } else {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            push(&src[start..i], cm);
        } else if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = i.min(b.len());
            if i < b.len() && b[i] == c {
                i += 1;
            }
            push(&src[start..i], st);
        } else if c.is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.') {
                i += 1;
            }
            push(&src[start..i], nm);
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let w = &src[start..i];
            let color = if LUA_KEYWORDS.contains(&w) {
                kw
            } else if w == "ic" {
                bi
            } else {
                tx
            };
            push(w, color);
        } else {
            i += 1;
            while i < b.len() && !special(b[i]) {
                i += 1;
            }
            push(&src[start..i], tx);
        }
    }
    job
}

struct FileEntry {
    path: PathBuf,
    name: String,
    depth: usize,
    is_dir: bool,
}

fn scan(dir: &Path, depth: usize, out: &mut Vec<FileEntry>) {
    if depth > 4 || out.len() > 500 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut items: Vec<_> = rd.filter_map(Result::ok).collect();
    items.sort_by_key(|e| (!e.path().is_dir(), e.file_name().to_string_lossy().to_lowercase()));
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || ["build", "target", "cache"].contains(&name.as_str()) {
            continue;
        }
        let path = e.path();
        let is_dir = path.is_dir();
        out.push(FileEntry { path: path.clone(), name, depth, is_dir });
        if is_dir {
            scan(&path, depth + 1, out);
        }
    }
}

/// Проекты (папки с main.lua) внутри папки `dir`.
fn scan_projects(dir: &Path) -> Vec<String> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<String> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().join("main.lua").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

/// Что редактируется в окне «Настройки»: имя проекта (папка + готовый файл) и ic.toml.
struct SettingsDraft {
    name: String,
    s: ProjectSettings,
}

const TARGETS: [&str; 4] = ["this", "windows", "linux", "macos"];

fn target_label(lang: Lang, t: &str) -> &'static str {
    match t {
        "windows" => "Windows (.exe)",
        "linux" => "Linux",
        "macos" => "macOS",
        _ => tr(lang, K::TargetThis),
    }
}

struct Project {
    dir: PathBuf,
    name: String,
    files: Vec<FileEntry>,
    open: Option<PathBuf>,
    text: String,
    dirty: bool,
    note: Option<String>,
}

fn load_file(proj: &mut Project, path: PathBuf, lang: Lang) {
    match fs::read_to_string(&path) {
        Ok(t) => {
            proj.text = t;
            proj.open = Some(path);
            proj.dirty = false;
            proj.note = None;
        }
        Err(_) => proj.note = Some(tr(lang, K::NotText).to_string()),
    }
}

// ───────────────────────── #запуск ─────────────────────────

fn pump<R: Read + Send + 'static>(r: Option<R>, tx: Sender<String>, ctx: egui::Context) -> Option<JoinHandle<()>> {
    r.map(|r| {
        thread::spawn(move || {
            for line in BufReader::new(r).lines().map_while(Result::ok) {
                let _ = tx.send(line);
                ctx.request_repaint();
            }
        })
    })
}

enum Action {
    Create,
    OpenTyped,
    OpenRecent(String),
    Save,
    Run,
    Build,
    CloseProject,
    OpenFile(PathBuf),
    ClearLog,
    OpenSettings,
    SaveSettings,
    CloseSettings,
    Check,
    PickProject(String),
}

struct IcApp {
    settings: Settings,
    // экран «Новый проект»
    name: String,
    location: String,
    start_error: Option<String>,
    // открытый проект
    project: Option<Project>,
    // список проектов в выбранной папке (обновляется при смене пути)
    found: Vec<String>,
    found_for: String,
    // #настройки проекта — черновик, пока открыто окно
    settings_draft: Option<SettingsDraft>,
    // журнал процессов
    log: Vec<String>,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl IcApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        setup_style(&cc.egui_ctx, settings.dark);
        let (tx, rx) = channel();
        let location = exe_dir().unwrap_or_default().join("projects").to_string_lossy().into_owned();
        Self { settings, name: "my_game".into(), location, start_error: None, project: None, found: Vec::new(), found_for: String::new(), settings_draft: None, log: Vec::new(), tx, rx }
    }

    // #создание проекта — делегируем `ic new`, чтобы шаблон был один на всех
    fn create_project(&mut self) {
        let lang = self.settings.lang;
        let name = self.name.trim().to_string();
        if name.is_empty() || name.contains(['/', '\\']) {
            self.start_error = Some(tr(lang, K::ErrName).to_string());
            return;
        }
        let dir = resolve_dir(&self.location).join(&name);
        let Some(ic) = ic_path() else {
            self.start_error = Some(tr(lang, K::IcMissing).to_string());
            return;
        };
        let mut cmd = Command::new(ic);
        cmd.arg("new").arg(&dir).current_dir(exe_dir().unwrap_or_default()).stdin(Stdio::null());
        hide_console(&mut cmd);
        match cmd.output() {
            Ok(o) if o.status.success() => self.open_project(dir),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                self.start_error = Some(format!("{}: {err}", tr(lang, K::ErrCreate)));
            }
            Err(e) => self.start_error = Some(format!("{}: {e}", tr(lang, K::ErrCreate))),
        }
    }

    fn open_project(&mut self, dir: PathBuf) {
        let lang = self.settings.lang;
        if !dir.join("main.lua").is_file() {
            self.start_error = Some(tr(lang, K::ErrNotProject).to_string());
            return;
        }
        let mut files = Vec::new();
        scan(&dir, 0, &mut files);
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "project".into());
        let mut proj = Project { dir: dir.clone(), name, files, open: None, text: String::new(), dirty: false, note: None };
        load_file(&mut proj, dir.join("main.lua"), lang);
        self.project = Some(proj);
        self.start_error = None;
        self.settings.push_recent(&dir);
        self.settings.save();
    }

    fn open_file(&mut self, path: PathBuf) {
        self.save();
        let lang = self.settings.lang;
        if let Some(p) = self.project.as_mut() {
            load_file(p, path, lang);
        }
    }

    fn save(&mut self) {
        let Some(p) = self.project.as_mut() else { return };
        if !p.dirty {
            return;
        }
        if let Some(path) = &p.open {
            match fs::write(path, &p.text) {
                Ok(()) => p.dirty = false,
                Err(e) => self.log.push(format!("save error: {e}")),
            }
        }
    }

    fn project_dir(&self) -> Option<String> {
        self.project.as_ref().map(|p| p.dir.to_string_lossy().into_owned())
    }

    /// Запускает `ic <args>` и стримит вывод в журнал.
    fn run_ic(&mut self, ctx: &egui::Context, args: Vec<String>) {
        let Some(ic) = ic_path() else {
            self.log.push(tr(self.settings.lang, K::IcMissing).to_string());
            return;
        };
        self.log.push(format!("> ic {}", args.join(" ")));
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let cwd = exe_dir().unwrap_or_default();
        thread::spawn(move || {
            let mut cmd = Command::new(ic);
            cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
            hide_console(&mut cmd);
            match cmd.spawn() {
                Err(e) => {
                    let _ = tx.send(format!("error: {e}"));
                }
                Ok(mut child) => {
                    let pumps = [
                        pump(child.stdout.take(), tx.clone(), ctx.clone()),
                        pump(child.stderr.take(), tx.clone(), ctx.clone()),
                    ];
                    for h in pumps.into_iter().flatten() {
                        let _ = h.join();
                    }
                    if let Ok(status) = child.wait() {
                        let _ = tx.send(format!("[{status}]"));
                    }
                }
            }
            ctx.request_repaint();
        });
    }

    fn handle(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Create => self.create_project(),
            Action::OpenTyped => {
                let dir = resolve_dir(&self.location).join(self.name.trim());
                self.open_project(dir);
            }
            Action::OpenRecent(p) => self.open_project(PathBuf::from(p)),
            Action::Save => self.save(),
            Action::Run => {
                self.save();
                if let Some(dir) = self.project_dir() {
                    self.run_ic(ctx, vec![dir]);
                }
            }
            // #компиляция — `ic build <проект>` кладёт один exe в <проект>/build/
            Action::Build => {
                self.save();
                if let Some(dir) = self.project_dir() {
                    self.run_ic(ctx, vec!["build".into(), dir]);
                }
            }
            Action::CloseProject => {
                self.save();
                self.project = None;
                self.start_error = None;
            }
            Action::OpenFile(p) => self.open_file(p),
            Action::ClearLog => self.log.clear(),
            Action::Check => {
    self.save();
    if let Some(dir) = self.project_dir() {
        self.run_ic(ctx, vec!["check".into(), dir]);
    }
}
// #настройки проекта — ic.toml
Action::OpenSettings => {
    if let Some((dir, name)) = self.project.as_ref().map(|p| (p.dir.clone(), p.name.clone())) {
        self.settings_draft = Some(SettingsDraft { name, s: ProjectSettings::load(&dir).0 });
    }
}
Action::SaveSettings => {
    let lang = self.settings.lang;
    let Some(draft) = self.settings_draft.take() else { return };
    let Some(proj) = self.project.as_mut() else { return };
    let new_name = draft.name.trim().to_string();
    if new_name.is_empty() || new_name.contains(['/', '\\']) {
        self.log.push(tr(lang, K::ErrName).to_string());
        self.settings_draft = Some(draft);
        return;
    }
    if let Err(e) = draft.s.save(&proj.dir) {
        self.log.push(e);
        self.settings_draft = Some(draft);
        return;
    }
    // название проекта = имя папки = имя готового файла: переименовываем папку
    if new_name != proj.name {
        let old = proj.dir.clone();
        let new = old.with_file_name(&new_name);
        if new.exists() {
            self.log.push(format!("{}: {}", tr(lang, K::ErrRename), new.display()));
            self.settings_draft = Some(draft);
            return;
        }
        if let Err(e) = fs::rename(&old, &new) {
            self.log.push(format!("{}: {e}", tr(lang, K::ErrRename)));
            self.settings_draft = Some(draft);
            return;
        }
        if let Some(o) = proj.open.clone() {
            if let Ok(rel) = o.strip_prefix(&old) {
                proj.open = Some(new.join(rel));
            }
        }
        proj.dir = new.clone();
        proj.name = new_name;
        let old_s = old.to_string_lossy().into_owned();
        self.settings.recent.retain(|r| *r != old_s);
        self.settings.push_recent(&new);
        self.settings.save();
    }
    proj.files.clear();
    scan(&proj.dir, 0, &mut proj.files);
    let toml = proj.dir.join(ProjectSettings::FILE);
    if proj.open.as_ref() == Some(&toml) && !proj.dirty {
        load_file(proj, toml, lang);
    }
    self.log.push(tr(lang, K::SettingsSaved).to_string());
}
Action::CloseSettings => self.settings_draft = None,
Action::PickProject(n) => self.name = n,
        }
    }
}

impl eframe::App for IcApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // #журнал — забираем вывод процессов
        while let Ok(line) = self.rx.try_recv() {
            self.log.push(line);
        }
        if self.log.len() > 1000 {
            let n = self.log.len() - 1000;
            self.log.drain(..n);
        }

        let mut action: Option<Action> = None;
        let mut changed = false;
        let has_project = self.project.is_some();
        let lang = self.settings.lang;
        let dark = self.settings.dark;
        let p = palette(dark);
        let theme_tip = format!("{}: {}", tr(lang, K::Theme), tr(lang, if dark { K::Light } else { K::Dark }));

        // #шапка
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::none().fill(p.panel).inner_margin(Margin::symmetric(12.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(proj) = &self.project {
                        ui.label(RichText::new(proj.name.as_str()).weak());
                        ui.add_space(10.0);
                        if button(ui, tr(lang, K::Save)).on_hover_text("Ctrl+S").clicked() {
                            action = Some(Action::Save);
                        }
                        if primary_button(ui, tr(lang, K::Run)).on_hover_text("F5").clicked() {
                            action = Some(Action::Run);
                        }
                        if button(ui, tr(lang, K::Check)).on_hover_text(tr(lang, K::CheckHint)).clicked() {
                            action = Some(Action::Check);
                        }
                        if button(ui, tr(lang, K::Build)).on_hover_text(tr(lang, K::BuildHint)).clicked() {
                            action = Some(Action::Build);
                        }
                        if button(ui, tr(lang, K::ProjSettings)).clicked() {
                            action = Some(Action::OpenSettings);
                        }
                        if button(ui, tr(lang, K::Close)).clicked() {
                            action = Some(Action::CloseProject);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme_toggle(ui, &mut self.settings.dark, &theme_tip) {
                            changed = true;
                        }
                        egui::ComboBox::from_id_source("lang").selected_text(lang.label()).show_ui(ui, |ui| {
                            for l in Lang::ALL {
                                if ui.selectable_value(&mut self.settings.lang, l, l.label()).clicked() {
                                    changed = true;
                                }
                            }
                        });
                        ui.label(RichText::new(tr(lang, K::Language)).weak());
                    });
                });
            });

        if changed {
            setup_style(ctx, self.settings.dark);
            self.settings.save();
        }
        let lang = self.settings.lang;
        let dark = self.settings.dark;
        let p = palette(dark);

        // горячие клавиши
        if has_project {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
                action = Some(Action::Save);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
                action = Some(Action::Run);
            }
        }

        if has_project {
            // #журнал — нижняя панель
            egui::TopBottomPanel::bottom("log")
                .resizable(true)
                .default_height(150.0)
                .min_height(60.0)
                .frame(egui::Frame::none().fill(p.panel).inner_margin(Margin::symmetric(12.0, 8.0)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr(lang, K::Log)).small().weak());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(tr(lang, K::ClearLog)).clicked() {
                                action = Some(Action::ClearLog);
                            }
                        });
                    });
                    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                        for line in &self.log {
                            ui.monospace(line);
                        }
                    });
                });

            // #файлы — боковая панель
            egui::SidePanel::left("files")
                .resizable(true)
                .default_width(190.0)
                .frame(egui::Frame::none().fill(p.panel).inner_margin(Margin::symmetric(10.0, 8.0)))
                .show(ctx, |ui| {
                    ui.label(RichText::new(tr(lang, K::Files)).small().weak());
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if let Some(proj) = &self.project {
                            for f in &proj.files {
                                ui.horizontal(|ui| {
                                    ui.add_space(f.depth as f32 * 12.0);
                                    if f.is_dir {
                                        ui.label(RichText::new(format!("{}/", f.name)).weak());
                                    } else {
                                        let sel = proj.open.as_deref() == Some(f.path.as_path());
                                        if ui.selectable_label(sel, f.name.as_str()).clicked() {
                                            action = Some(Action::OpenFile(f.path.clone()));
                                        }
                                    }
                                });
                            }
                        }
                    });
                });

            // #редактор — код файла с номерами строк и подсветкой
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(p.editor).inner_margin(Margin::same(10.0)))
                .show(ctx, |ui| {
                    let Some(proj) = self.project.as_mut() else { return };
                    ui.horizontal(|ui| {
                        let title = proj
                            .open
                            .as_ref()
                            .and_then(|f| f.strip_prefix(&proj.dir).ok())
                            .map(|r| r.display().to_string())
                            .unwrap_or_default();
                        ui.label(RichText::new(title).weak());
                        if proj.dirty {
                            ui.label(
                                RichText::new(format!("● {}", tr(lang, K::Unsaved))).color(Color32::from_rgb(0xE5, 0xA0, 0x3B)),
                            );
                        }
                    });
                    if let Some(note) = &proj.note {
                        ui.label(note.as_str());
                    }
                    if proj.open.is_none() {
                        return;
                    }

                    let font = egui::TextStyle::Monospace.resolve(ui.style());
                    let is_lua = proj.open.as_ref().and_then(|p| p.extension()).is_some_and(|e| e == "lua");
                    let weak = ui.visuals().weak_text_color();
                    let lines = proj.text.split('\n').count().max(1);
                    let width = lines.to_string().len();
                    let nums = (1..=lines).map(|i| format!("{i:>width$}")).collect::<Vec<_>>().join("\n");

                    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                        let mut job = highlight(text, font.clone(), dark, is_lua);
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(job))
                    };

                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.add_space(2.0);
                                ui.label(RichText::new(nums).monospace().color(weak));
                            });
                            let resp = ui.add(
                                egui::TextEdit::multiline(&mut proj.text)
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .frame(false)
                                    .layouter(&mut layouter),
                            );
                            if resp.changed() {
                                proj.dirty = true;
                            }
                        });
                    });
                });
        } else {
            // #старт — сначала спрашиваем название и папку
            let name_ok = {
                let n = self.name.trim();
                !n.is_empty() && !n.contains(['/', '\\'])
            };
            let exists = name_ok && resolve_dir(&self.location).join(self.name.trim()).join("main.lua").is_file();
            if self.location != self.found_for {
                self.found = scan_projects(&resolve_dir(&self.location));
                self.found_for = self.location.clone();
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(p.editor).inner_margin(Margin::same(24.0)))
                .show(ctx, |ui| {
                    let top = (ui.available_height() * 0.12).min(90.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(top);
                        ui.allocate_ui_with_layout(egui::vec2(480.0, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.heading(tr(lang, K::StartTitle));
                            ui.label(RichText::new(tr(lang, K::StartSub)).weak());
                            ui.add_space(10.0);

                            ui.label(tr(lang, K::ProjectName));
                            let r = ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(f32::INFINITY));
                            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && name_ok && !exists {
                                action = Some(Action::Create);
                            }

                            ui.label(tr(lang, K::Location));
                            ui.add(egui::TextEdit::singleline(&mut self.location).desired_width(f32::INFINITY));
                            let full = resolve_dir(&self.location).join(self.name.trim());
                            ui.label(RichText::new(format!("→ {}", full.display())).small().weak());

                            if exists {
                                ui.colored_label(Color32::from_rgb(0x4E, 0xC9, 0x7B), tr(lang, K::FoundProject));
                            } else if name_ok {
                                ui.label(RichText::new(tr(lang, K::WillCreate)).weak());
                            }
                            if let Some(err) = &self.start_error {
                                ui.colored_label(Color32::from_rgb(0xF4, 0x47, 0x47), err.as_str());
                            }

                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                // «Создать» — только для нового имени, «Открыть» — только если проект уже есть
                                if primary_on(ui, name_ok && !exists, tr(lang, K::Create)).clicked() {
                                    action = Some(Action::Create);
                                }
                                if button_on(ui, exists, tr(lang, K::OpenExisting)).on_disabled_hover_text(tr(lang, K::OpenHint)).clicked() {
                                    action = Some(Action::OpenTyped);
                                }
                            });

                            // подсказка: что нужно, чтобы открыть существующий проект
                            if !exists {
                                ui.label(RichText::new(tr(lang, K::OpenHint)).small().weak());
                            }
                            if !self.found.is_empty() {
                                ui.add_space(10.0);
                                ui.label(RichText::new(tr(lang, K::FoundList)).small().weak());
                                for n in &self.found {
                                    if ui.selectable_label(n.as_str() == self.name.trim(), n.as_str()).clicked() {
                                        action = Some(Action::PickProject(n.clone()));
                                    }
                                }
                            }

                            if !self.settings.recent.is_empty() {
                                ui.add_space(14.0);
                                ui.label(RichText::new(tr(lang, K::Recent)).small().weak());
                                for r in &self.settings.recent {
                                    if ui.link(r.as_str()).clicked() {
                                        action = Some(Action::OpenRecent(r.clone()));
                                    }
                                }
                            }
                        });
                    });
                });
        }

        // #настройки проекта — окно с полями ic.toml и пояснением для пользователя
        if let Some(draft) = self.settings_draft.as_mut() {
            let mut open = true;
            egui::Window::new(tr(lang, K::ProjSettings))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(RichText::new(tr(lang, K::ProjSettingsNote)).small().weak());
                    ui.add_space(8.0);
                    egui::Grid::new("project_settings").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                        ui.label(tr(lang, K::SetName));
                        ui.text_edit_singleline(&mut draft.name);
                        ui.end_row();

                        ui.label(tr(lang, K::SetTitle));
                        ui.text_edit_singleline(&mut draft.s.title);
                        ui.end_row();

                        ui.label(tr(lang, K::SetWindow));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut draft.s.width).clamp_range(320..=7680).suffix(" px"));
                            ui.label("×");
                            ui.add(egui::DragValue::new(&mut draft.s.height).clamp_range(240..=4320).suffix(" px"));
                        });
                        ui.end_row();

                        ui.label(tr(lang, K::SetTick));
                        ui.add(egui::DragValue::new(&mut draft.s.tick_rate).clamp_range(10..=480).suffix(" Hz"));
                        ui.end_row();

                        ui.label(tr(lang, K::SetBackground));
                        ui.color_edit_button_srgb(&mut draft.s.background);
                        ui.end_row();

                        ui.label(tr(lang, K::SetTarget));
                        egui::ComboBox::from_id_source("build_target")
                            .selected_text(target_label(lang, &draft.s.target))
                            .show_ui(ui, |ui| {
                                for t in TARGETS {
                                    ui.selectable_value(&mut draft.s.target, t.to_string(), target_label(lang, t));
                                }
                            });
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr(lang, K::SetNameHint)).small().weak());
                    ui.label(RichText::new(tr(lang, K::TargetHint)).small().weak());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if primary_button(ui, tr(lang, K::Save)).clicked() {
                            action = Some(Action::SaveSettings);
                        }
                        if button(ui, tr(lang, K::Cancel)).clicked() {
                            action = Some(Action::CloseSettings);
                        }
                    });
                });
            if !open {
                action = Some(Action::CloseSettings);
            }
        }
        if let Some(a) = action {
            self.handle(ctx, a);
        }
    }
}

pub fn run() -> eframe::Result<()> {
    confine_caches();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 680.0])
            .with_min_inner_size([880.0, 480.0])
            .with_title("IC")
            .with_icon(egui::IconData { rgba: ICON_RGBA.to_vec(), width: 256, height: 256 }),
        ..Default::default()
    };
    eframe::run_native("IC", options, Box::new(|cc| Ok(Box::new(IcApp::new(cc)))))
}

mod i18n;

use eframe::egui::{self, Color32, FontId, Margin, RichText, Rounding, Stroke};
use i18n::{tr, Lang, K};
use ic_build::scaffold::{self, Dimension, Generator, Kind, NewProject};
use ic_build::ProjectSettings;
use std::{
    collections::HashSet,
    env, fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{channel, Receiver, Sender},
    thread::{self, JoinHandle},
};

const ICON_RGBA: &[u8] = include_bytes!("../assets/icon.rgba");

fn exe_dir() -> Option<PathBuf> {
    env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

fn ic_path() -> Option<PathBuf> {
    env::current_exe().ok()
}

fn resolve_dir(input: &str) -> PathBuf {
    let p = PathBuf::from(input.trim());
    if p.is_absolute() {
        p
    } else {
        exe_dir().unwrap_or_default().join(p)
    }
}

// #защита-проекта — проверяет, что путь и его реальные родительские каталоги
// остаются внутри корня проекта. Символические ссылки за пределы проекта запрещены.
fn project_path_is_safe(project_dir: &Path, path: &Path) -> bool {
    let Ok(root) = fs::canonicalize(project_dir) else {
        return false;
    };
    if !path.starts_with(project_dir) {
        return false;
    }

    if path.exists() {
        return fs::canonicalize(path)
            .map(|real| real.starts_with(&root))
            .unwrap_or(false);
    }

    let Some(parent) = path.parent() else {
        return false;
    };
    fs::canonicalize(parent)
        .map(|real_parent| real_parent.starts_with(&root))
        .unwrap_or(false)
}

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
        set("MESA_SHADER_CACHE_MAX_SIZE", std::ffi::OsStr::new("256M"));
        set(
            "__GL_SHADER_DISK_CACHE_SIZE",
            std::ffi::OsStr::new("268435456"),
        );
    }
}

fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

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
        let v = env::var("LC_ALL")
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default();
        if v.starts_with("ru") {
            Lang::Ru
        } else {
            Lang::En
        }
    }

    fn load() -> Self {
        let mut s = Settings {
            dark: true,
            lang: Self::system_lang(),
            recent: Vec::new(),
        };
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

// #стиль — общие цвета, шрифты и состояния элементов интерфейса.
fn setup_style(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (
            egui::TextStyle::Heading,
            FontId::new(22.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            FontId::new(15.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            FontId::new(15.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            FontId::new(12.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            FontId::new(14.0, egui::FontFamily::Monospace),
        ),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.interact_size.y = 28.0;

    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
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
    ui.add_enabled(
        on,
        egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(accent),
    )
}

fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    button_on(ui, true, text)
}

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    primary_on(ui, true, text)
}

fn theme_toggle(ui: &mut egui::Ui, dark: &mut bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 28.0), egui::Sense::click());
    let resp = resp
        .on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = ui.ctx().animate_bool_with_time(resp.id, *dark, 0.25);

    let v = ui.style().interact(&resp).clone();
    let fill = v.weak_bg_fill;
    let col = ui.visuals().text_color();
    let painter = ui.painter().with_clip_rect(rect);

    painter.rect_filled(rect, 4.0, fill);
    let c = rect.center();
    let r = 4.5 + 2.5 * t;
    painter.circle_filled(c, r, col);

    if t > 0.01 {
        let bite = egui::vec2(16.0, -16.0) * (1.0 - t) + egui::vec2(3.5, -3.5) * t;
        painter.circle_filled(c + bite, 5.5, fill);
    }
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

const LUA_KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

fn highlight(src: &str, font: FontId, dark: bool, lua: bool) -> egui::text::LayoutJob {
    let rgb = Color32::from_rgb;
    let (kw, st, cm, nm, bi, tx) = if dark {
        (
            rgb(0xC5, 0x86, 0xC0),
            rgb(0xCE, 0x91, 0x78),
            rgb(0x6A, 0x99, 0x55),
            rgb(0xB5, 0xCE, 0xA8),
            rgb(0x4F, 0xC1, 0xFF),
            rgb(0xD4, 0xD4, 0xD4),
        )
    } else {
        (
            rgb(0xAF, 0x00, 0xDB),
            rgb(0xA3, 0x15, 0x15),
            rgb(0x00, 0x80, 0x00),
            rgb(0x09, 0x86, 0x58),
            rgb(0x00, 0x70, 0xC1),
            rgb(0x1F, 0x1F, 0x1F),
        )
    };

    let mut job = egui::text::LayoutJob::default();
    if !lua {
        job.append(
            src,
            0.0,
            egui::text::TextFormat {
                font_id: font,
                color: tx,
                ..Default::default()
            },
        );
        return job;
    }
    let mut push = |s: &str, color: Color32| {
        if !s.is_empty() {
            job.append(
                s,
                0.0,
                egui::text::TextFormat {
                    font_id: font.clone(),
                    color,
                    ..Default::default()
                },
            );
        }
    };
    let special =
        |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'"' || c == b'\'' || c == b'-';

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
    items.sort_by_key(|e| {
        (
            !e.path().is_dir(),
            e.file_name().to_string_lossy().to_lowercase(),
        )
    });
    for e in items {
        // Не показываем и не обходим symlink: он может вести за пределы проекта.
        if e.file_type().map(|kind| kind.is_symlink()).unwrap_or(true) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || ["build", "target", "cache"].contains(&name.as_str()) {
            continue;
        }
        let path = e.path();
        let is_dir = path.is_dir();
        out.push(FileEntry {
            path: path.clone(),
            name,
            depth,
            is_dir,
        });
        if is_dir {
            scan(&path, depth + 1, out);
        }
    }
}

fn scan_projects(dir: &Path) -> Vec<String> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<String> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().join("main.lua").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    NewFile,
    NewFolder,
    Rename,
    Delete,
}

struct Prompt {
    kind: PromptKind,
    path: PathBuf,
    text: String,
    error: Option<String>,
    focus: bool,
}

struct NewDialog {
    kind: Kind,
    dimension: Dimension,
    label: String,
    gens: [bool; 4],
    default_code: bool,
}

impl Default for NewDialog {
    fn default() -> Self {
        Self {
            kind: Kind::Game,
            dimension: Dimension::D2,
            label: String::new(),
            gens: [false; 4],
            default_code: true,
        }
    }
}

fn generator_label(lang: Lang, g: Generator) -> &'static str {
    match g {
        Generator::Scene => tr(lang, K::GenScene),
        Generator::Fps => tr(lang, K::GenFps),
        Generator::Physics => tr(lang, K::GenPhysics),
        Generator::Camera => tr(lang, K::GenCamera),
    }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', '\\']) && s != "." && s != ".."
}

fn kind_label(lang: Lang, s: &ProjectSettings) -> String {
    if s.kind == "app" {
        return tr(lang, K::KindApp).to_string();
    }
    let game = tr(lang, K::KindGame);
    match s.dimension.as_str() {
        "3d" => format!("{game} 3D"),
        "custom" if !s.label.is_empty() => format!("{game}: {}", s.label),
        "custom" => game.to_string(),
        _ => format!("{game} 2D"),
    }
}

fn file_color(name: &str) -> Color32 {
    match name.rsplit('.').next().unwrap_or("") {
        "lua" => Color32::from_rgb(0x51, 0xA0, 0xCF),
        "toml" => Color32::from_rgb(0xC0, 0x7A, 0x3A),
        "md" => Color32::from_rgb(0x6C, 0xB6, 0xFF),
        "json" => Color32::from_rgb(0xE5, 0xC0, 0x5A),
        _ => Color32::from_rgb(0x8A, 0x8F, 0x98),
    }
}

fn explorer_rows(
    ui: &mut egui::Ui,
    proj: &Project,
    dark: bool,
    lang: Lang,
    dragging_file: &mut Option<PathBuf>,
    action: &mut Option<Action>,
) {
    let pal = palette(dark);
    let row_h = 22.0;
    let indent = 14.0;
    let text_color = ui.visuals().text_color();
    let folder_color = Color32::from_rgb(0xDC, 0xB6, 0x7A);
    let mut skip: Option<usize> = None;

    for f in &proj.files {
        if let Some(d) = skip {
            if f.depth > d {
                continue;
            }
            skip = None;
        }
        let collapsed = f.is_dir && proj.collapsed.contains(&f.path);
        if collapsed {
            skip = Some(f.depth);
        }

        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_h),
            egui::Sense::click_and_drag(),
        );
        let selected = proj.open.as_deref() == Some(f.path.as_path()) || proj.selected == f.path;
        {
            let painter = ui.painter();
            if selected {
                painter.rect_filled(rect, 0.0, pal.accent.gamma_multiply(0.5));
            } else if resp.hovered() {
                painter.rect_filled(rect, 0.0, pal.hover.gamma_multiply(0.7));
            }
            let base_x = rect.left() + 12.0;
            for lvl in 0..f.depth {
                let gx = base_x + lvl as f32 * indent + 6.0;
                painter.line_segment(
                    [egui::pos2(gx, rect.top()), egui::pos2(gx, rect.bottom())],
                    Stroke::new(1.0, pal.border),
                );
            }
            let x = base_x + f.depth as f32 * indent;
            let cy = rect.center().y;
            let color = if selected { Color32::WHITE } else { text_color };
            if f.is_dir {
                let c = egui::pos2(x + 6.0, cy);
                let pts = if collapsed {
                    vec![
                        egui::pos2(c.x - 2.5, c.y - 4.0),
                        egui::pos2(c.x - 2.5, c.y + 4.0),
                        egui::pos2(c.x + 3.0, c.y),
                    ]
                } else {
                    vec![
                        egui::pos2(c.x - 4.0, c.y - 2.5),
                        egui::pos2(c.x + 4.0, c.y - 2.5),
                        egui::pos2(c.x, c.y + 3.0),
                    ]
                };
                painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
                let body = egui::Rect::from_min_size(
                    egui::pos2(x + 18.0, cy - 4.0),
                    egui::vec2(14.0, 10.0),
                );
                painter.rect_filled(body, 2.0, folder_color);
                let tab =
                    egui::Rect::from_min_size(egui::pos2(x + 18.0, cy - 6.0), egui::vec2(6.0, 3.0));
                painter.rect_filled(tab, 1.0, folder_color);
            } else {
                let body = egui::Rect::from_min_size(
                    egui::pos2(x + 20.0, cy - 6.5),
                    egui::vec2(10.0, 13.0),
                );
                painter.rect_filled(body, 2.0, file_color(&f.name));
            }
            painter.text(
                egui::pos2(x + 38.0, cy),
                egui::Align2::LEFT_CENTER,
                &f.name,
                egui::FontId::proportional(14.0),
                color,
            );
        }

        if resp.clicked() {
            *action = Some(if f.is_dir {
                Action::ToggleDir(f.path.clone())
            } else {
                Action::OpenFile(f.path.clone())
            });
        }
        // #перетаскивание — файлы можно перетаскивать на папки внутри проекта.
        if resp.drag_started() {
            *dragging_file = Some(f.path.clone());
        }
        if f.is_dir
            && resp.hovered()
            && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
        {
            if let Some(source) = dragging_file.take() {
                if source != f.path && !f.path.starts_with(&source) {
                    *action = Some(Action::MovePath(source, f.path.clone()));
                }
            }
        }
        resp.context_menu(|ui| {
            let base = if f.is_dir {
                f.path.clone()
            } else {
                f.path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| proj.dir.clone())
            };
            if ui.button(tr(lang, K::NewFile)).clicked() {
                *action = Some(Action::PromptOpen(PromptKind::NewFile, base.clone()));
                ui.close_menu();
            }
            if ui.button(tr(lang, K::NewFolder)).clicked() {
                *action = Some(Action::PromptOpen(PromptKind::NewFolder, base.clone()));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(tr(lang, K::Rename)).clicked() {
                *action = Some(Action::PromptOpen(PromptKind::Rename, f.path.clone()));
                ui.close_menu();
            }
            if ui.button(tr(lang, K::Delete)).clicked() {
                *action = Some(Action::PromptOpen(PromptKind::Delete, f.path.clone()));
                ui.close_menu();
            }
        });
    }
}

struct Project {
    dir: PathBuf,
    name: String,
    files: Vec<FileEntry>,
    // #вкладки — список открытых документов; активный текст хранится в open/text.
    tabs: Vec<PathBuf>,
    open: Option<PathBuf>,
    text: String,
    dirty: bool,
    note: Option<String>,
    collapsed: HashSet<PathBuf>,
    selected: PathBuf,
    kind_label: String,
}

fn load_file(proj: &mut Project, path: PathBuf, lang: Lang) {
    match fs::read_to_string(&path) {
        Ok(t) => {
            proj.text = t;
            proj.open = Some(path.clone());
            if !proj.tabs.contains(&path) {
                proj.tabs.push(path);
            }
            proj.dirty = false;
            proj.note = None;
        }
        Err(_) => proj.note = Some(tr(lang, K::NotText).to_string()),
    }
}

// #сцены — простой текстовый формат сцены, независимый от Lua-кода проекта.
#[derive(Clone)]
struct SceneNode {
    name: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [u8; 3],
}

struct SceneEditor {
    open: bool,
    path: Option<PathBuf>,
    nodes: Vec<SceneNode>,
    selected: Option<usize>,
    dirty: bool,
    status: Option<String>,
    drag_origin: Option<(usize, f32, f32)>,
}

impl Default for SceneEditor {
    fn default() -> Self {
        Self {
            open: false,
            path: None,
            nodes: Vec::new(),
            selected: None,
            dirty: false,
            status: None,
            drag_origin: None,
        }
    }
}

impl SceneEditor {
    fn open_project(&mut self, project_dir: &Path) {
        let dir = project_dir.join("scenes");
        if !dir.exists() {
            let _ = fs::create_dir(&dir);
        }
        if !dir.is_dir() || !project_path_is_safe(project_dir, &dir) {
            self.open = false;
            self.path = None;
            self.status = Some("Scene folder is outside the project".into());
            return;
        }
        let path = dir.join("main.ic-scene");
        self.nodes.clear();
        self.selected = None;
        self.dirty = false;
        self.status = None;
        self.drag_origin = None;
        if let Ok(text) = fs::read_to_string(&path) {
            for line in text
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            {
                let parts: Vec<_> = line.split('|').collect();
                if parts.len() != 9 || parts[0] != "node" {
                    continue;
                }
                let parsed = (
                    parts[2].parse::<f32>(),
                    parts[3].parse::<f32>(),
                    parts[4].parse::<f32>(),
                    parts[5].parse::<f32>(),
                    parts[6].parse::<u8>(),
                    parts[7].parse::<u8>(),
                    parts[8].parse::<u8>(),
                );
                if let (Ok(x), Ok(y), Ok(width), Ok(height), Ok(r), Ok(g), Ok(b)) = parsed {
                    self.nodes.push(SceneNode {
                        name: parts[1].replace("%7C", "|"),
                        x,
                        y,
                        width,
                        height,
                        color: [r, g, b],
                    });
                }
            }
        }
        self.path = Some(path);
        self.open = true;
    }

    fn save(&mut self) -> Result<(), String> {
        let Some(path) = self.path.as_ref() else {
            return Err("Scene path is not set".into());
        };
        if let Some(project) = path.parent().and_then(Path::parent) {
            if !project_path_is_safe(project, path) {
                return Err("Scene path is outside the project".into());
            }
        }
        let mut out = String::from("# IC 2D scene v1\n# node|name|x|y|width|height|r|g|b\n");
        for node in &self.nodes {
            let name = node.name.replace('|', "%7C").replace('\n', " ");
            out.push_str(&format!(
                "node|{}|{:.2}|{:.2}|{:.2}|{:.2}|{}|{}|{}\n",
                name,
                node.x,
                node.y,
                node.width,
                node.height,
                node.color[0],
                node.color[1],
                node.color[2]
            ));
        }
        fs::write(path, out).map_err(|e| format!("scene save error: {e}"))?;
        self.dirty = false;
        self.status = Some("Scene saved".into());
        Ok(())
    }

    fn add_node(&mut self) {
        let n = self.nodes.len();
        self.nodes.push(SceneNode {
            name: format!(
                "Node2D{}",
                if n == 0 {
                    String::new()
                } else {
                    (n + 1).to_string()
                }
            ),
            x: 60.0 + n as f32 * 18.0,
            y: 60.0 + n as f32 * 18.0,
            width: 96.0,
            height: 64.0,
            color: [72, 132, 196],
        });
        self.selected = Some(self.nodes.len() - 1);
        self.dirty = true;
    }
}

fn rebase_path(path: &Path, old_root: &Path, new_root: &Path) -> PathBuf {
    match path.strip_prefix(old_root) {
        Ok(rel) if rel.as_os_str().is_empty() => new_root.to_path_buf(),
        Ok(rel) => new_root.join(rel),
        Err(_) => path.to_path_buf(),
    }
}

fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let original = Path::new(name);
    let stem = original
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("asset");
    let ext = original.extension().and_then(|v| v.to_str()).unwrap_or("");
    let mut candidate = dir.join(name);
    let mut index = 2;
    while candidate.exists() {
        let file = if ext.is_empty() {
            format!("{stem}_{index}")
        } else {
            format!("{stem}_{index}.{ext}")
        };
        candidate = dir.join(file);
        index += 1;
    }
    candidate
}

fn pump<R: Read + Send + 'static>(
    r: Option<R>,
    tx: Sender<String>,
    ctx: egui::Context,
) -> Option<JoinHandle<()>> {
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
    SelectTab(PathBuf),
    CloseTab(PathBuf),
    MovePath(PathBuf, PathBuf),
    OpenScene,
    ClearLog,
    OpenSettings,
    SaveSettings,
    CloseSettings,
    Check,
    PickProject(String),
    ConfirmCreate,
    CancelCreate,
    AddGenerator(Generator),
    PromptOpen(PromptKind, PathBuf),
    PromptOk,
    PromptCancel,
    ToggleDir(PathBuf),
}

struct IcApp {
    settings: Settings,
    name: String,
    location: String,
    start_error: Option<String>,
    project: Option<Project>,
    found: Vec<String>,
    found_for: String,
    settings_draft: Option<SettingsDraft>,
    new_dialog: Option<NewDialog>,
    prompt: Option<Prompt>,
    log: Vec<String>,
    tx: Sender<String>,
    rx: Receiver<String>,
    dragging_file: Option<PathBuf>,
    quick_open: bool,
    quick_filter: String,
    scene_editor: SceneEditor,
}

impl IcApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        setup_style(&cc.egui_ctx, settings.dark);
        let (tx, rx) = channel();
        let location = exe_dir()
            .unwrap_or_default()
            .join("projects")
            .to_string_lossy()
            .into_owned();
        Self {
            settings,
            name: "my_game".into(),
            location,
            start_error: None,
            project: None,
            found: Vec::new(),
            found_for: String::new(),
            settings_draft: None,
            new_dialog: None,
            prompt: None,
            log: Vec::new(),
            tx,
            rx,
            dragging_file: None,
            quick_open: false,
            quick_filter: String::new(),
            scene_editor: SceneEditor::default(),
        }
    }

    fn begin_create(&mut self) {
        let lang = self.settings.lang;
        if !valid_name(self.name.trim()) {
            self.start_error = Some(tr(lang, K::ErrName).to_string());
            return;
        }
        self.start_error = None;
        self.new_dialog = Some(NewDialog::default());
    }

    fn create_project(&mut self) {
        let lang = self.settings.lang;
        let Some(d) = self.new_dialog.take() else {
            return;
        };
        let dir = resolve_dir(&self.location).join(self.name.trim());
        let generators: Vec<Generator> = if d.kind == Kind::Game {
            Generator::ALL
                .iter()
                .zip(d.gens)
                .filter(|(_, on)| *on)
                .map(|(g, _)| *g)
                .collect()
        } else {
            Vec::new()
        };
        let opts = NewProject {
            kind: d.kind,
            dimension: d.dimension,
            label: d.label.trim().to_string(),
            generators,
            default_code: d.default_code,
        };
        match scaffold::create(&dir, &opts) {
            Ok(_) => self.open_project(dir),
            Err(e) => self.start_error = Some(format!("{}: {e}", tr(lang, K::ErrCreate))),
        }
    }

    fn prompt_ok(&mut self) {
        let lang = self.settings.lang;
        let Some(pr) = self.prompt.take() else { return };
        let Some(proj) = self.project.as_mut() else {
            return;
        };
        let text = pr.text.trim().to_string();
        let mut err: Option<String> = None;
        let mut opened: Option<PathBuf> = None;

        match pr.kind {
            PromptKind::NewFile | PromptKind::NewFolder => {
                if !valid_name(&text) {
                    err = Some(tr(lang, K::ErrName).to_string());
                } else {
                    let target = pr.path.join(&text);
                    if !project_path_is_safe(&proj.dir, &target) {
                        err = Some("Path is outside the project or uses a symbolic link".into());
                    } else if target.exists() {
                        err = Some(format!("{}: {text}", tr(lang, K::ErrExists)));
                    } else if pr.kind == PromptKind::NewFile {
                        match fs::write(&target, "") {
                            Ok(()) => opened = Some(target),
                            Err(e) => err = Some(e.to_string()),
                        }
                    } else if let Err(e) = fs::create_dir(&target) {
                        err = Some(e.to_string());
                    }
                }
            }
            PromptKind::Rename => {
                let new = pr.path.with_file_name(&text);
                if !valid_name(&text) {
                    err = Some(tr(lang, K::ErrName).to_string());
                } else if !project_path_is_safe(&proj.dir, &pr.path)
                    || !project_path_is_safe(&proj.dir, &new)
                {
                    err = Some("Path is outside the project or uses a symbolic link".into());
                } else if new.exists() {
                    err = Some(format!("{}: {text}", tr(lang, K::ErrExists)));
                } else if let Err(e) = fs::rename(&pr.path, &new) {
                    err = Some(e.to_string());
                } else {
                    if let Some(o) = proj.open.clone() {
                        if let Ok(rel) = o.strip_prefix(&pr.path) {
                            proj.open = Some(if rel.as_os_str().is_empty() {
                                new.clone()
                            } else {
                                new.join(rel)
                            });
                        }
                    }
                    for tab in &mut proj.tabs {
                        *tab = rebase_path(tab, &pr.path, &new);
                    }
                    proj.collapsed = proj
                        .collapsed
                        .iter()
                        .map(|path| rebase_path(path, &pr.path, &new))
                        .collect();
                    proj.selected = new;
                }
            }
            PromptKind::Delete => {
                if !project_path_is_safe(&proj.dir, &pr.path) {
                    err = Some("Refusing to delete a path outside the project".into());
                    // Keep the delete prompt open; no filesystem operation is performed.
                    self.prompt = Some(Prompt { error: err, ..pr });
                    return;
                }
                let r = if pr.path.is_dir() {
                    fs::remove_dir_all(&pr.path)
                } else {
                    fs::remove_file(&pr.path)
                };
                match r {
                    Ok(()) => {
                        proj.tabs.retain(|tab| !tab.starts_with(&pr.path));
                        if proj.open.as_ref().is_some_and(|o| o.starts_with(&pr.path)) {
                            proj.open = None;
                            proj.text.clear();
                            proj.dirty = false;
                            if let Some(next) = proj.tabs.last().cloned() {
                                load_file(proj, next, lang);
                            }
                        }
                        proj.selected = proj.dir.clone();
                    }
                    Err(e) => err = Some(e.to_string()),
                }
            }
        }

        if let Some(e) = err {
            self.prompt = Some(Prompt {
                error: Some(e),
                ..pr
            });
            return;
        }
        proj.files.clear();
        scan(&proj.dir, 0, &mut proj.files);
        if let Some(p) = opened {
            load_file(proj, p.clone(), lang);
            proj.selected = p;
        }
    }

    fn open_project(&mut self, dir: PathBuf) {
        let lang = self.settings.lang;
        self.scene_editor.open = false;
        let dir = match fs::canonicalize(&dir) {
            Ok(path) => path,
            Err(e) => {
                self.start_error = Some(format!("{}: {e}", tr(lang, K::ErrNotProject)));
                return;
            }
        };
        if !dir.join("main.lua").is_file() || !project_path_is_safe(&dir, &dir.join("main.lua")) {
            self.start_error = Some(tr(lang, K::ErrNotProject).to_string());
            return;
        }
        let mut files = Vec::new();
        scan(&dir, 0, &mut files);
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());
        let settings_path = dir.join(ProjectSettings::FILE);
        let settings = if project_path_is_safe(&dir, &settings_path) {
            ProjectSettings::load(&dir).0
        } else {
            ProjectSettings::default()
        };
        let kind_label = kind_label(lang, &settings);
        let mut proj = Project {
            dir: dir.clone(),
            name,
            files,
            tabs: Vec::new(),
            open: None,
            text: String::new(),
            dirty: false,
            note: None,
            collapsed: HashSet::new(),
            selected: dir.clone(),
            kind_label,
        };
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
            if !project_path_is_safe(&p.dir, &path) {
                self.log.push("open refused: file is outside the project".into());
                return;
            }
            load_file(p, path, lang);
            if let Some(o) = p.open.clone() {
                p.selected = o;
            }
        }
    }

    // #вкладки — переключение сохраняет текущий файл, закрытие не удаляет его с диска.
    fn close_tab(&mut self, path: PathBuf) {
        self.save();
        let lang = self.settings.lang;
        if let Some(proj) = self.project.as_mut() {
            let was_active = proj.open.as_ref() == Some(&path);
            proj.tabs.retain(|tab| tab != &path);
            if was_active {
                if let Some(next) = proj.tabs.last().cloned() {
                    load_file(proj, next, lang);
                } else {
                    proj.open = None;
                    proj.text.clear();
                    proj.dirty = false;
                    proj.note = None;
                }
            }
        }
    }

    fn move_project_file(&mut self, source: PathBuf, destination_dir: PathBuf) {
        self.save();
        let lang = self.settings.lang;
        let Some(proj) = self.project.as_mut() else {
            return;
        };
        if (!source.is_file() && !source.is_dir())
            || !destination_dir.is_dir()
            || !project_path_is_safe(&proj.dir, &source)
            || !project_path_is_safe(&proj.dir, &destination_dir)
            || destination_dir.starts_with(&source)
            || source.parent() == Some(destination_dir.as_path())
        {
            return;
        }
        let Some(name) = source.file_name() else {
            return;
        };
        let destination = destination_dir.join(name);
        if destination.exists() {
            self.log.push(format!(
                "{}: {}",
                tr(lang, K::ErrExists),
                destination.display()
            ));
            return;
        }
        if let Err(e) = fs::rename(&source, &destination) {
            self.log.push(format!("move error: {e}"));
            return;
        }
        for tab in &mut proj.tabs {
            *tab = rebase_path(tab, &source, &destination);
        }
        if let Some(open) = proj.open.clone() {
            if open.starts_with(&source) {
                proj.open = Some(rebase_path(&open, &source, &destination));
            }
        }
        proj.collapsed = proj
            .collapsed
            .iter()
            .map(|path| rebase_path(path, &source, &destination))
            .collect();
        proj.selected = rebase_path(&proj.selected, &source, &destination);
        proj.files.clear();
        scan(&proj.dir, 0, &mut proj.files);
    }

    fn save(&mut self) {
        let Some(p) = self.project.as_mut() else {
            return;
        };
        if !p.dirty {
            return;
        }
        if let Some(path) = &p.open {
            if !project_path_is_safe(&p.dir, path) {
                self.log.push("save refused: file is outside the project".into());
                return;
            }
            match fs::write(path, &p.text) {
                Ok(()) => p.dirty = false,
                Err(e) => self.log.push(format!("save error: {e}")),
            }
        }
    }

    fn project_dir(&self) -> Option<String> {
        self.project
            .as_ref()
            .map(|p| p.dir.to_string_lossy().into_owned())
    }

    // #компиляция — запуск CLI-команд IC и вывод их журнала в интерфейс.
    fn run_ic(&mut self, ctx: &egui::Context, args: Vec<String>) {
        let Some(ic) = ic_path() else {
            self.log
                .push(tr(self.settings.lang, K::IcMissing).to_string());
            return;
        };
        self.log.push(format!("> ic {}", args.join(" ")));
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let cwd = exe_dir().unwrap_or_default();
        thread::spawn(move || {
            let mut cmd = Command::new(ic);
            cmd.args(args)
                .current_dir(cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
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
            Action::Create => self.begin_create(),
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
            Action::Build => {
                self.save();
                if let Some(dir) = self.project_dir() {
                    self.run_ic(ctx, vec!["build".into(), dir]);
                }
            }
            Action::CloseProject => {
                self.save();
                if self.scene_editor.dirty {
                    let _ = self.scene_editor.save();
                }
                self.project = None;
                self.scene_editor.open = false;
                self.start_error = None;
            }
            Action::OpenFile(p) | Action::SelectTab(p) => self.open_file(p),
            Action::CloseTab(p) => self.close_tab(p),
            Action::MovePath(source, destination) => self.move_project_file(source, destination),
            Action::OpenScene => {
                if let Some(dir) = self.project.as_ref().map(|p| p.dir.clone()) {
                    self.scene_editor.open_project(&dir);
                    if let Some(proj) = self.project.as_mut() {
                        proj.files.clear();
                        scan(&proj.dir, 0, &mut proj.files);
                    }
                }
            }
            Action::ClearLog => self.log.clear(),
            Action::Check => {
                self.save();
                if let Some(dir) = self.project_dir() {
                    self.run_ic(ctx, vec!["check".into(), dir]);
                }
            }
            Action::OpenSettings => {
                if let Some((dir, name)) = self
                    .project
                    .as_ref()
                    .map(|p| (p.dir.clone(), p.name.clone()))
                {
                    let settings_path = dir.join(ProjectSettings::FILE);
                    let settings = if project_path_is_safe(&dir, &settings_path) {
                        ProjectSettings::load(&dir).0
                    } else {
                        ProjectSettings::default()
                    };
                    self.settings_draft = Some(SettingsDraft {
                        name,
                        s: settings,
                    });
                }
            }
            Action::SaveSettings => {
                let lang = self.settings.lang;
                let Some(draft) = self.settings_draft.take() else {
                    return;
                };
                let Some(proj) = self.project.as_mut() else {
                    return;
                };
                let new_name = draft.name.trim().to_string();
                if new_name.is_empty() || new_name.contains(['/', '\\']) {
                    self.log.push(tr(lang, K::ErrName).to_string());
                    self.settings_draft = Some(draft);
                    return;
                }
                if !project_path_is_safe(&proj.dir, &proj.dir.join(ProjectSettings::FILE)) {
                    self.log.push("Settings save refused: path is outside the project".into());
                    self.settings_draft = Some(draft);
                    return;
                }
                if let Err(e) = draft.s.save(&proj.dir) {
                    self.log.push(e);
                    self.settings_draft = Some(draft);
                    return;
                }
                if new_name != proj.name {
                    let old = proj.dir.clone();
                    let new = old.with_file_name(&new_name);
                    if new.exists() {
                        self.log
                            .push(format!("{}: {}", tr(lang, K::ErrRename), new.display()));
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
                    for tab in &mut proj.tabs {
                        *tab = rebase_path(tab, &old, &new);
                    }
                    proj.collapsed = proj
                        .collapsed
                        .iter()
                        .map(|path| rebase_path(path, &old, &new))
                        .collect();
                    if let Some(scene_path) = self.scene_editor.path.clone() {
                        if let Ok(rel) = scene_path.strip_prefix(&old) {
                            self.scene_editor.path = Some(new.join(rel));
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
                proj.kind_label = kind_label(lang, &draft.s);
                let toml = proj.dir.join(ProjectSettings::FILE);
                if proj.open.as_ref() == Some(&toml) && !proj.dirty {
                    load_file(proj, toml, lang);
                }
                self.log.push(tr(lang, K::SettingsSaved).to_string());
            }
            Action::CloseSettings => self.settings_draft = None,
            Action::PickProject(n) => self.name = n,
            Action::ConfirmCreate => self.create_project(),
            Action::CancelCreate => self.new_dialog = None,
            Action::AddGenerator(g) => {
                let lang = self.settings.lang;
                if let Some(proj) = self.project.as_mut() {
                    match scaffold::add_generator(&proj.dir, g) {
                        Ok(file) => {
                            self.log.push(format!(
                                "{}: {file} — {}",
                                tr(lang, K::GenAdded),
                                g.usage()
                            ));
                            proj.files.clear();
                            scan(&proj.dir, 0, &mut proj.files);
                        }
                        Err(e) => self.log.push(e),
                    }
                }
            }
            Action::PromptOpen(kind, path) => {
                let text = if kind == PromptKind::Rename {
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                self.prompt = Some(Prompt {
                    kind,
                    path,
                    text,
                    error: None,
                    focus: true,
                });
            }
            Action::PromptCancel => self.prompt = None,
            Action::PromptOk => self.prompt_ok(),
            Action::ToggleDir(p) => {
                if let Some(proj) = self.project.as_mut() {
                    if !proj.collapsed.remove(&p) {
                        proj.collapsed.insert(p.clone());
                    }
                    proj.selected = p;
                }
            }
        }
    }

    // #редактор-сцен — отдельное нативное окно с иерархией, холстом и инспектором.
    fn show_scene_editor(&mut self, ctx: &egui::Context, lang: Lang, dark: bool) {
        if !self.scene_editor.open {
            return;
        }
        let scene = &mut self.scene_editor;
        let scene_file_before = scene.path.as_ref().is_some_and(|path| path.is_file());
        let viewport_id = egui::ViewportId::from_hash_of("ic_scene_editor");
        let builder = egui::ViewportBuilder::default()
            .with_title(tr(lang, K::SceneEditor))
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([820.0, 520.0]);

        ctx.show_viewport_immediate(viewport_id, builder, |scene_ctx, _class| {
            if scene_ctx.input(|i| i.viewport().close_requested()) {
                if scene.dirty {
                    let _ = scene.save();
                }
                scene.open = false;
                return;
            }
            if scene_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
                if let Err(e) = scene.save() {
                    scene.status = Some(e);
                } else {
                    scene.status = Some(tr(lang, K::SceneSaved).to_string());
                }
            }
            let panel = palette(dark).panel;
            let canvas = palette(dark).editor;
            egui::TopBottomPanel::top("scene_toolbar")
                .frame(
                    egui::Frame::none()
                        .fill(panel)
                        .inner_margin(Margin::symmetric(12.0, 8.0)),
                )
                .show(scene_ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("IC  /  SCENE").strong());
                        ui.separator();
                        if primary_button(ui, tr(lang, K::AddSceneNode)).clicked() {
                            scene.add_node();
                        }
                        if button(ui, tr(lang, K::Save))
                            .on_hover_text("Ctrl+S")
                            .clicked()
                        {
                            if let Err(e) = scene.save() {
                                scene.status = Some(e);
                            } else {
                                scene.status = Some(tr(lang, K::SceneSaved).to_string());
                            }
                        }
                        if scene.dirty {
                            ui.label(
                                RichText::new("● unsaved")
                                    .color(Color32::from_rgb(0xE5, 0xA0, 0x3B)),
                            );
                        }
                        if let Some(status) = &scene.status {
                            ui.label(RichText::new(status).small().weak());
                        }
                    });
                });

            egui::SidePanel::left("scene_hierarchy")
                .resizable(true)
                .default_width(210.0)
                .width_range(150.0..=420.0)
                .frame(
                    egui::Frame::none()
                        .fill(panel)
                        .inner_margin(Margin::same(10.0)),
                )
                .show(scene_ctx, |ui| {
                    ui.label(RichText::new(tr(lang, K::Hierarchy)).small().strong());
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for index in 0..scene.nodes.len() {
                            let label = format!("◇  {}", scene.nodes[index].name);
                            if ui
                                .selectable_label(scene.selected == Some(index), label)
                                .clicked()
                            {
                                scene.selected = Some(index);
                            }
                        }
                    });
                    if scene.nodes.is_empty() {
                        ui.label(RichText::new(tr(lang, K::SceneEmpty)).small().weak());
                    }
                });

            egui::SidePanel::right("scene_inspector")
                .resizable(true)
                .default_width(250.0)
                .width_range(190.0..=480.0)
                .frame(
                    egui::Frame::none()
                        .fill(panel)
                        .inner_margin(Margin::same(10.0)),
                )
                .show(scene_ctx, |ui| {
                    ui.label(RichText::new(tr(lang, K::Inspector)).small().strong());
                    ui.separator();
                    if let Some(index) = scene.selected.filter(|i| *i < scene.nodes.len()) {
                        let node = &mut scene.nodes[index];
                        ui.label(tr(lang, K::NodeName));
                        if ui.text_edit_singleline(&mut node.name).changed() {
                            scene.dirty = true;
                        }
                        ui.add_space(8.0);
                        ui.label(tr(lang, K::Position));
                        ui.horizontal(|ui| {
                            ui.label("X");
                            if ui
                                .add(egui::DragValue::new(&mut node.x).speed(1.0))
                                .changed()
                            {
                                scene.dirty = true;
                            }
                            ui.label("Y");
                            if ui
                                .add(egui::DragValue::new(&mut node.y).speed(1.0))
                                .changed()
                            {
                                scene.dirty = true;
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(tr(lang, K::Size));
                        ui.horizontal(|ui| {
                            ui.label("W");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut node.width)
                                        .speed(1.0)
                                        .clamp_range(8.0..=2048.0),
                                )
                                .changed()
                            {
                                scene.dirty = true;
                            }
                            ui.label("H");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut node.height)
                                        .speed(1.0)
                                        .clamp_range(8.0..=2048.0),
                                )
                                .changed()
                            {
                                scene.dirty = true;
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(tr(lang, K::Color));
                        if ui.color_edit_button_srgb(&mut node.color).changed() {
                            scene.dirty = true;
                        }
                        ui.add_space(12.0);
                        if ui.button(tr(lang, K::DeleteSceneNode)).clicked() {
                            scene.nodes.remove(index);
                            scene.selected = None;
                            scene.dirty = true;
                        }
                    } else {
                        ui.label(RichText::new(tr(lang, K::SceneSelectNode)).small().weak());
                    }
                });

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(canvas)
                        .inner_margin(Margin::same(10.0)),
                )
                .show(scene_ctx, |ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 4.0, canvas);
                    let grid_color = if dark {
                        Color32::from_gray(55)
                    } else {
                        Color32::from_gray(220)
                    };
                    let mut gx = rect.left() + 24.0;
                    while gx < rect.right() {
                        painter.line_segment(
                            [egui::pos2(gx, rect.top()), egui::pos2(gx, rect.bottom())],
                            Stroke::new(1.0, grid_color),
                        );
                        gx += 32.0;
                    }
                    let mut gy = rect.top() + 24.0;
                    while gy < rect.bottom() {
                        painter.line_segment(
                            [egui::pos2(rect.left(), gy), egui::pos2(rect.right(), gy)],
                            Stroke::new(1.0, grid_color),
                        );
                        gy += 32.0;
                    }
                    let origin = rect.left_top() + egui::vec2(24.0, 24.0);
                    for index in 0..scene.nodes.len() {
                        let current = scene.nodes[index].clone();
                        let node_rect = egui::Rect::from_min_size(
                            origin + egui::vec2(current.x, current.y),
                            egui::vec2(current.width.max(8.0), current.height.max(8.0)),
                        );
                        let response = ui.interact(
                            node_rect,
                            egui::Id::new(("scene_node", index)),
                            egui::Sense::click_and_drag(),
                        );
                        if response.clicked() {
                            scene.selected = Some(index);
                        }
                        if response.drag_started() {
                            scene.selected = Some(index);
                            scene.drag_origin = Some((index, current.x, current.y));
                        }
                        if response.dragged() {
                            if let Some((drag_index, start_x, start_y)) = scene.drag_origin {
                                if drag_index == index {
                                    let delta = response.drag_delta();
                                    scene.nodes[index].x = start_x + delta.x;
                                    scene.nodes[index].y = start_y + delta.y;
                                    scene.dirty = true;
                                }
                            }
                        }
                        let node = &scene.nodes[index];
                        let fill = Color32::from_rgb(node.color[0], node.color[1], node.color[2]);
                        painter.rect_filled(node_rect, 3.0, fill.gamma_multiply(0.82));
                        painter.rect_stroke(
                            node_rect,
                            Rounding::same(3.0),
                            Stroke::new(
                                if scene.selected == Some(index) {
                                    2.0
                                } else {
                                    1.0
                                },
                                if scene.selected == Some(index) {
                                    Color32::WHITE
                                } else {
                                    fill
                                },
                            ),
                        );
                        painter.text(
                            node_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &node.name,
                            FontId::proportional(14.0),
                            Color32::WHITE,
                        );
                    }
                    if scene_ctx.input(|i| i.pointer.any_released()) {
                        scene.drag_origin = None;
                    }
                    painter.text(
                        rect.left_bottom() + egui::vec2(8.0, -8.0),
                        egui::Align2::LEFT_BOTTOM,
                        tr(lang, K::CanvasHint),
                        FontId::proportional(11.0),
                        scene_ctx.style().visuals.weak_text_color(),
                    );
                });
        });
        let scene_file_after = scene.path.as_ref().is_some_and(|path| path.is_file());
        if !scene_file_before && scene_file_after {
            if let Some(proj) = self.project.as_mut() {
                proj.files.clear();
                scan(&proj.dir, 0, &mut proj.files);
            }
        }
    }
}

impl eframe::App for IcApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(line) = self.rx.try_recv() {
            self.log.push(line);
        }
        // #импорт-файлов — системный drag-and-drop копирует файлы в assets/.
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            if let Some(proj) = self.project.as_mut() {
                let assets = proj.dir.join("assets");
                if !assets.exists() {
                    let _ = fs::create_dir(&assets);
                }
                if !assets.is_dir() || !project_path_is_safe(&proj.dir, &assets) {
                    self.log.push("Import refused: assets folder is outside the project".into());
                } else {
                for dropped in dropped_files {
                    let name = dropped
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| {
                            if dropped.name.is_empty() {
                                "dropped_asset.bin".into()
                            } else {
                                // Берём только имя файла, чтобы имя из drag-and-drop
                                // не могло подставить ../ или путь к другой папке.
                                dropped.name.replace('\\\\', "/")
                                    .rsplit('/')
                                    .next()
                                    .filter(|name| !name.is_empty() && *name != "." && *name != "..")
                                    .unwrap_or("dropped_asset.bin")
                                    .to_string()
                            }
                        });
                    let destination = unique_path(&assets, &name);
                    if !project_path_is_safe(&proj.dir, &destination) {
                        self.log.push("Import refused: destination is outside the project".into());
                        continue;
                    }
                    let result = if let Some(source) = dropped.path.as_ref() {
                        if source.is_file() {
                            fs::copy(source, &destination).map(|_| ())
                        } else {
                            continue;
                        }
                    } else if let Some(bytes) = dropped.bytes.as_ref() {
                        fs::write(&destination, bytes.as_ref())
                    } else {
                        continue;
                    };
                    match result {
                        Ok(()) => self
                            .log
                            .push(format!("Imported: {}", destination.display())),
                        Err(e) => self.log.push(format!("Import error: {e}")),
                    }
                }
                }
                proj.files.clear();
                scan(&proj.dir, 0, &mut proj.files);
            }
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
        let theme_tip = format!(
            "{}: {}",
            tr(lang, K::Theme),
            tr(lang, if dark { K::Light } else { K::Dark })
        );

        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::none()
                    .fill(p.panel)
                    .inner_margin(Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(proj) = &self.project {
                        ui.label(RichText::new(proj.name.as_str()).weak());
                        ui.add_space(10.0);
                        if button(ui, tr(lang, K::Save))
                            .on_hover_text("Ctrl+S")
                            .clicked()
                        {
                            action = Some(Action::Save);
                        }
                        if primary_button(ui, tr(lang, K::Run))
                            .on_hover_text("F5")
                            .clicked()
                        {
                            action = Some(Action::Run);
                        }
                        if button(ui, tr(lang, K::Check))
                            .on_hover_text(tr(lang, K::CheckHint))
                            .clicked()
                        {
                            action = Some(Action::Check);
                        }
                        if button(ui, tr(lang, K::Build))
                            .on_hover_text(tr(lang, K::BuildHint))
                            .clicked()
                        {
                            action = Some(Action::Build);
                        }
                        if button(ui, tr(lang, K::SceneEditor)).clicked() {
                            action = Some(Action::OpenScene);
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
                        egui::ComboBox::from_id_source("lang")
                            .selected_text(lang.label())
                            .show_ui(ui, |ui| {
                                for l in Lang::ALL {
                                    if ui
                                        .selectable_value(&mut self.settings.lang, l, l.label())
                                        .clicked()
                                    {
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

        if has_project {
            // #горячие-клавиши — основные команды редактора в стиле VS Code.
            let command = egui::Modifiers::COMMAND;
            let command_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
            if ctx.input_mut(|i| i.consume_key(command, egui::Key::S)) {
                action = Some(Action::Save);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
                action = Some(Action::Run);
            }
            if ctx.input_mut(|i| i.consume_key(command_shift, egui::Key::B)) {
                action = Some(Action::Build);
            }
            if ctx.input_mut(|i| i.consume_key(command, egui::Key::P)) {
                self.quick_filter.clear();
                self.quick_open = true;
            }
            if ctx.input_mut(|i| i.consume_key(command_shift, egui::Key::N)) {
                if let Some(proj) = self.project.as_ref() {
                    let mut base = proj.selected.clone();
                    if !base.is_dir() {
                        base = base
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| proj.dir.clone());
                    }
                    action = Some(Action::PromptOpen(PromptKind::NewFolder, base));
                }
            } else if ctx.input_mut(|i| i.consume_key(command, egui::Key::N)) {
                if let Some(proj) = self.project.as_ref() {
                    let mut base = proj.selected.clone();
                    if !base.is_dir() {
                        base = base
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| proj.dir.clone());
                    }
                    action = Some(Action::PromptOpen(PromptKind::NewFile, base));
                }
            }
            if ctx.input_mut(|i| i.consume_key(command, egui::Key::W)) {
                if let Some(path) = self.project.as_ref().and_then(|p| p.open.clone()) {
                    action = Some(Action::CloseTab(path));
                }
            }
            let previous_tab = ctx.input_mut(|i| i.consume_key(command_shift, egui::Key::Tab));
            let next_tab =
                !previous_tab && ctx.input_mut(|i| i.consume_key(command, egui::Key::Tab));
            if previous_tab || next_tab {
                if let Some(proj) = self.project.as_ref() {
                    if !proj.tabs.is_empty() {
                        let current = proj
                            .open
                            .as_ref()
                            .and_then(|path| proj.tabs.iter().position(|tab| tab == path))
                            .unwrap_or(0);
                        let next = if previous_tab {
                            (current + proj.tabs.len() - 1) % proj.tabs.len()
                        } else {
                            (current + 1) % proj.tabs.len()
                        };
                        action = Some(Action::SelectTab(proj.tabs[next].clone()));
                    }
                }
            }
        }

        if has_project {
            egui::TopBottomPanel::bottom("log")
                .resizable(true)
                .default_height(150.0)
                .min_height(60.0)
                .frame(
                    egui::Frame::none()
                        .fill(p.panel)
                        .inner_margin(Margin::symmetric(12.0, 8.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr(lang, K::Log)).small().weak());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(tr(lang, K::ClearLog)).clicked() {
                                action = Some(Action::ClearLog);
                            }
                        });
                    });
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for line in &self.log {
                                ui.monospace(line);
                            }
                        });
                });

            egui::SidePanel::left("files")
                .resizable(true)
                .default_width(240.0)
                .frame(
                    egui::Frame::none()
                        .fill(p.panel)
                        .inner_margin(Margin::symmetric(0.0, 8.0)),
                )
                .show(ctx, |ui| {
                    let Some(proj) = self.project.as_ref() else {
                        return;
                    };
                    let mut base = proj.selected.clone();
                    if !base.is_dir() {
                        base = base
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| proj.dir.clone());
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(tr(lang, K::Explorer)).small().strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(proj.name.as_str()).small().weak());
                        });
                    });
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(8.0);
                        if ui.small_button(tr(lang, K::NewFileShort)).clicked() {
                            action = Some(Action::PromptOpen(PromptKind::NewFile, base.clone()));
                        }
                        if ui.small_button(tr(lang, K::NewFolderShort)).clicked() {
                            action = Some(Action::PromptOpen(PromptKind::NewFolder, base.clone()));
                        }
                        ui.menu_button(tr(lang, K::AddGen), |ui| {
                            for g in Generator::ALL {
                                if ui.button(generator_label(lang, g)).clicked() {
                                    action = Some(Action::AddGenerator(g));
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(proj.kind_label.as_str()).small().weak());
                    });
                    ui.add_space(6.0);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            explorer_rows(
                                ui,
                                proj,
                                dark,
                                lang,
                                &mut self.dragging_file,
                                &mut action,
                            );
                            if ui.input(|i| i.pointer.any_released()) {
                                self.dragging_file = None;
                            }
                        });
                });
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(p.editor)
                        .inner_margin(Margin::same(10.0)),
                )
                .show(ctx, |ui| {
                    let Some(proj) = self.project.as_mut() else {
                        return;
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("IC / EDITOR").small().weak());
                        if proj.dirty {
                            ui.label(
                                RichText::new(format!("● {}", tr(lang, K::Unsaved)))
                                    .color(Color32::from_rgb(0xE5, 0xA0, 0x3B)),
                            );
                        }
                    });
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(p.panel)
                        .rounding(Rounding::same(5.0))
                        .inner_margin(Margin::symmetric(6.0, 4.0))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let tabs = proj.tabs.clone();
                                for tab in tabs {
                                    let label = tab
                                        .strip_prefix(&proj.dir)
                                        .unwrap_or(&tab)
                                        .display()
                                        .to_string();
                                    let active = proj.open.as_ref() == Some(&tab);
                                    if ui.selectable_label(active, label).clicked() {
                                        action = Some(Action::SelectTab(tab.clone()));
                                    }
                                    if ui
                                        .small_button("×")
                                        .on_hover_text("Close tab (Ctrl+W)")
                                        .clicked()
                                    {
                                        action = Some(Action::CloseTab(tab));
                                    }
                                }
                                if proj.tabs.is_empty() {
                                    ui.label(RichText::new("No file open").weak().small());
                                }
                            });
                        });
                    if let Some(note) = &proj.note {
                        ui.label(note.as_str());
                    }
                    if proj.open.is_none() {
                        return;
                    }

                    let font = egui::TextStyle::Monospace.resolve(ui.style());
                    let is_lua = proj
                        .open
                        .as_ref()
                        .and_then(|p| p.extension())
                        .is_some_and(|e| e == "lua");
                    let weak = ui.visuals().weak_text_color();
                    let lines = proj.text.split('\n').count().max(1);
                    let width = lines.to_string().len();
                    let nums = (1..=lines)
                        .map(|i| format!("{i:>width$}"))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                        let mut job = highlight(text, font.clone(), dark, is_lua);
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(job))
                    };

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
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
            let name_ok = {
                let n = self.name.trim();
                !n.is_empty() && !n.contains(['/', '\\'])
            };
            let exists = name_ok
                && resolve_dir(&self.location)
                    .join(self.name.trim())
                    .join("main.lua")
                    .is_file();
            if self.location != self.found_for {
                self.found = scan_projects(&resolve_dir(&self.location));
                self.found_for = self.location.clone();
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(p.editor)
                        .inner_margin(Margin::same(24.0)),
                )
                .show(ctx, |ui| {
                    let top = (ui.available_height() * 0.12).min(90.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(top);
                        ui.allocate_ui_with_layout(
                            egui::vec2(480.0, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.heading(tr(lang, K::StartTitle));
                                ui.label(RichText::new(tr(lang, K::StartSub)).weak());
                                ui.add_space(10.0);

                                ui.label(tr(lang, K::ProjectName));
                                let r = ui.add(
                                    egui::TextEdit::singleline(&mut self.name)
                                        .desired_width(f32::INFINITY),
                                );
                                if r.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    && name_ok
                                    && !exists
                                {
                                    action = Some(Action::Create);
                                }

                                ui.label(tr(lang, K::Location));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.location)
                                        .desired_width(f32::INFINITY),
                                );
                                let full = resolve_dir(&self.location).join(self.name.trim());
                                ui.label(
                                    RichText::new(format!("→ {}", full.display()))
                                        .small()
                                        .weak(),
                                );

                                if exists {
                                    ui.colored_label(
                                        Color32::from_rgb(0x4E, 0xC9, 0x7B),
                                        tr(lang, K::FoundProject),
                                    );
                                } else if name_ok {
                                    ui.label(RichText::new(tr(lang, K::WillCreate)).weak());
                                }
                                if let Some(err) = &self.start_error {
                                    ui.colored_label(
                                        Color32::from_rgb(0xF4, 0x47, 0x47),
                                        err.as_str(),
                                    );
                                }

                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    if primary_on(ui, name_ok && !exists, tr(lang, K::Create))
                                        .clicked()
                                    {
                                        action = Some(Action::Create);
                                    }
                                    if button_on(ui, exists, tr(lang, K::OpenExisting))
                                        .on_disabled_hover_text(tr(lang, K::OpenHint))
                                        .clicked()
                                    {
                                        action = Some(Action::OpenTyped);
                                    }
                                });

                                if !exists {
                                    ui.label(RichText::new(tr(lang, K::OpenHint)).small().weak());
                                }
                                if !self.found.is_empty() {
                                    ui.add_space(10.0);
                                    ui.label(RichText::new(tr(lang, K::FoundList)).small().weak());
                                    for n in &self.found {
                                        if ui
                                            .selectable_label(
                                                n.as_str() == self.name.trim(),
                                                n.as_str(),
                                            )
                                            .clicked()
                                        {
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
                            },
                        );
                    });
                });
        }

        if let Some(draft) = self.settings_draft.as_mut() {
            let mut open = true;
            egui::Window::new(tr(lang, K::ProjSettings))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(620.0)
                .min_width(540.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(520.0);
                    ui.label(RichText::new(tr(lang, K::ProjSettings)).heading());
                    ui.label(RichText::new(tr(lang, K::SettingsIntro)).small().weak());
                    ui.add_space(10.0);
                    let group_style = ui.style().clone();
                    ui.columns(2, |columns| {
                        egui::Frame::group(&group_style)
                            .rounding(Rounding::same(7.0))
                            .inner_margin(Margin::same(12.0))
                            .show(&mut columns[0], |ui| {
                                ui.label(RichText::new(tr(lang, K::ProjectGroup)).small().strong());
                                ui.add_space(6.0);
                                ui.label(tr(lang, K::SetName));
                                ui.add(
                                    egui::TextEdit::singleline(&mut draft.name)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.add_space(8.0);
                                ui.label(tr(lang, K::SetTitle));
                                ui.add(
                                    egui::TextEdit::singleline(&mut draft.s.title)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.add_space(8.0);
                                ui.label(RichText::new(tr(lang, K::SetNameHint)).small().weak());
                            });
                        egui::Frame::group(&group_style)
                            .rounding(Rounding::same(7.0))
                            .inner_margin(Margin::same(12.0))
                            .show(&mut columns[1], |ui| {
                                ui.label(RichText::new(tr(lang, K::WindowGroup)).small().strong());
                                ui.add_space(6.0);
                                ui.label(tr(lang, K::SetWindow));
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut draft.s.width)
                                            .clamp_range(320..=7680)
                                            .suffix(" px"),
                                    );
                                    ui.label("×");
                                    ui.add(
                                        egui::DragValue::new(&mut draft.s.height)
                                            .clamp_range(240..=4320)
                                            .suffix(" px"),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.label(tr(lang, K::SetTick));
                                ui.add(
                                    egui::DragValue::new(&mut draft.s.tick_rate)
                                        .clamp_range(10..=480)
                                        .suffix(" Hz"),
                                );
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.label(tr(lang, K::SetBackground));
                                    ui.color_edit_button_srgb(&mut draft.s.background);
                                });
                                ui.add_space(8.0);
                                ui.label(tr(lang, K::SetTarget));
                                egui::ComboBox::from_id_source("build_target")
                                    .selected_text(target_label(lang, &draft.s.target))
                                    .width(ui.available_width())
                                    .show_ui(ui, |ui| {
                                        for t in TARGETS {
                                            ui.selectable_value(
                                                &mut draft.s.target,
                                                t.to_string(),
                                                target_label(lang, t),
                                            );
                                        }
                                    });
                                ui.add_space(6.0);
                                ui.label(RichText::new(tr(lang, K::TargetHint)).small().weak());
                            });
                    });
                    ui.add_space(12.0);
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
        if self.quick_open {
            let mut open = true;
            egui::Window::new(tr(lang, K::QuickOpen))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(460.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new("Ctrl+P").small().weak());
                    ui.add(
                        egui::TextEdit::singleline(&mut self.quick_filter)
                            .hint_text("Search files…")
                            .desired_width(f32::INFINITY),
                    );
                    if let Some(proj) = self.project.as_ref() {
                        let filter = self.quick_filter.to_lowercase();
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .show(ui, |ui| {
                                for file in proj.files.iter().filter(|f| !f.is_dir) {
                                    let label = file
                                        .path
                                        .strip_prefix(&proj.dir)
                                        .unwrap_or(&file.path)
                                        .display()
                                        .to_string();
                                    if !filter.is_empty() && !label.to_lowercase().contains(&filter)
                                    {
                                        continue;
                                    }
                                    if ui
                                        .selectable_label(
                                            proj.open.as_ref() == Some(&file.path),
                                            label,
                                        )
                                        .clicked()
                                    {
                                        action = Some(Action::SelectTab(file.path.clone()));
                                        self.quick_open = false;
                                    }
                                }
                            });
                    }
                });
            if !open {
                self.quick_open = false;
            }
        }

        if let Some(dlg) = self.new_dialog.as_mut() {
            egui::Window::new(tr(lang, K::StartTitle))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.label(RichText::new(tr(lang, K::KindQuestion)).strong());
                    ui.radio_value(&mut dlg.kind, Kind::App, tr(lang, K::KindApp));
                    ui.label(RichText::new(tr(lang, K::KindAppHint)).small().weak());
                    ui.radio_value(&mut dlg.kind, Kind::Game, tr(lang, K::KindGame));
                    ui.label(RichText::new(tr(lang, K::KindGameHint)).small().weak());

                    if dlg.kind == Kind::Game {
                        ui.add_space(8.0);
                        ui.label(RichText::new(tr(lang, K::GameType)).strong());
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut dlg.dimension, Dimension::D2, "2D");
                            ui.add_enabled(
                                false,
                                egui::RadioButton::new(false, tr(lang, K::Dim3DSoon)),
                            );
                            ui.radio_value(
                                &mut dlg.dimension,
                                Dimension::Custom,
                                tr(lang, K::DimCustom),
                            );
                        });
                        if dlg.dimension == Dimension::Custom {
                            ui.horizontal(|ui| {
                                ui.label(tr(lang, K::CustomLabel));
                                ui.text_edit_singleline(&mut dlg.label);
                            });
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new(tr(lang, K::AddNow)).strong());
                        ui.horizontal_wrapped(|ui| {
                            for (i, g) in Generator::ALL.iter().enumerate() {
                                ui.checkbox(&mut dlg.gens[i], generator_label(lang, *g));
                            }
                        });
                    }

                    ui.add_space(8.0);
                    ui.checkbox(&mut dlg.default_code, tr(lang, K::DefaultCode));
                    ui.label(RichText::new(tr(lang, K::DefaultCodeHint)).small().weak());
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr(lang, K::LaterHint)).small().weak());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if primary_button(ui, tr(lang, K::CreateBtn)).clicked() {
                            action = Some(Action::ConfirmCreate);
                        }
                        if button(ui, tr(lang, K::Cancel)).clicked() {
                            action = Some(Action::CancelCreate);
                        }
                    });
                });
        }

        if let Some(pr) = self.prompt.as_mut() {
            let title = match pr.kind {
                PromptKind::NewFile => tr(lang, K::NewFile),
                PromptKind::NewFolder => tr(lang, K::NewFolder),
                PromptKind::Rename => tr(lang, K::Rename),
                PromptKind::Delete => tr(lang, K::Delete),
            };
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(360.0);
                    if pr.kind == PromptKind::Delete {
                        let name = pr
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        ui.label(format!("{} «{}»?", tr(lang, K::Delete), name));
                    } else {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut pr.text).desired_width(f32::INFINITY),
                        );
                        if pr.focus {
                            r.request_focus();
                            pr.focus = false;
                        }
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            action = Some(Action::PromptOk);
                        }
                    }
                    if let Some(e) = &pr.error {
                        ui.colored_label(Color32::from_rgb(0xF4, 0x47, 0x47), e.as_str());
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if primary_button(ui, "OK").clicked() {
                            action = Some(Action::PromptOk);
                        }
                        if button(ui, tr(lang, K::Cancel)).clicked() {
                            action = Some(Action::PromptCancel);
                        }
                    });
                });
        }
        if let Some(a) = action {
            self.handle(ctx, a);
        }
        self.show_scene_editor(ctx, lang, dark);
    }
}

pub fn run() -> eframe::Result<()> {
    confine_caches();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 680.0])
            .with_min_inner_size([880.0, 480.0])
            .with_title("IC")
            .with_icon(egui::IconData {
                rgba: ICON_RGBA.to_vec(),
                width: 256,
                height: 256,
            }),
        ..Default::default()
    };
    eframe::run_native("IC", options, Box::new(|cc| Ok(Box::new(IcApp::new(cc)))))
}

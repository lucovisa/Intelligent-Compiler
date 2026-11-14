mod i18n;
mod scene_ui;

use eframe::egui::{self, Color32, FontId, Margin, RichText, Rounding, Stroke};
use i18n::{tr, Lang, K};
use ic_build::scaffold::{self, Dimension, Generator, Kind, NewProject};
use ic_build::ProjectSettings;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        mpsc::{channel, Receiver, Sender},
        OnceLock,
    },
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

fn confine_caches() -> Result<(), String> {
    let dir = exe_dir().ok_or_else(|| "не удалось определить папку IC".to_string())?;
    // Кэши, настройки и temp — внутрь папки IC
    ic_build::confine::confine_env(&dir)?;
    Ok(())
}

fn system_prefers_dark() -> bool {
    static SYSTEM_DARK: OnceLock<bool> = OnceLock::new();
    *SYSTEM_DARK.get_or_init(system_prefers_dark_uncached)
}

fn system_prefers_dark_uncached() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("reg")
            .args([
                "query",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                "/v",
                "AppsUseLightTheme",
            ])
            .output();
        if let Ok(output) = output {
            let value = String::from_utf8_lossy(&output.stdout);
            if value.contains("0x0") {
                return true;
            }
            if value.contains("0x1") {
                return false;
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            return String::from_utf8_lossy(&output.stdout)
                .to_lowercase()
                .contains("dark");
        }
    }
    #[cfg(target_os = "linux")]
    {
        let home = env::var_os("HOME")
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let gsettings = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .unwrap_or_default();
        let candidates = [
            env::var("GTK_THEME").unwrap_or_default(),
            fs::read_to_string(format!("{home}/.config/gtk-3.0/settings.ini")).unwrap_or_default(),
            fs::read_to_string(format!("{home}/.config/gtk-4.0/settings.ini")).unwrap_or_default(),
            fs::read_to_string(format!("{home}/.config/kdeglobals")).unwrap_or_default(),
            env::var("COLORFGBG").unwrap_or_default(),
            gsettings,
        ];
        let value = candidates.join(" ").to_lowercase();
        value.contains("gtk-application-prefer-dark-theme=1")
            || value.contains("adwaita:dark")
            || value.contains("breezedark")
            || value.contains("prefer-dark")
            || value.ends_with(";0")
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
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
    theme_system: bool,
    lang: Lang,
    lang_explicit: bool,
    recent: Vec<String>,
    trusted: Vec<String>,
}

impl Settings {
    fn path() -> Option<PathBuf> {
        Some(
            exe_dir()?
                .join(ic_build::confine::DATA_DIR)
                .join("settings")
                .join("ic-settings.txt"),
        )
    }

    fn load() -> Self {
        let mut s = Settings {
            dark: system_prefers_dark(),
            theme_system: true,
            lang: Lang::system_default(),
            lang_explicit: false,
            recent: Vec::new(),
            trusted: Vec::new(),
        };
        let mut saved_lang = None;
        if let Some(path) = Self::path() {
            if let Ok(text) = fs::read_to_string(path) {
                for line in text.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        match (k.trim(), v.trim()) {
                            ("theme", "system") => {
                                s.theme_system = true;
                                s.dark = system_prefers_dark();
                            }
                            ("theme", "dark") => {
                                s.theme_system = false;
                                s.dark = true;
                            }
                            ("theme", "light") => {
                                s.theme_system = false;
                                s.dark = false;
                            }
                            ("lang", v) => saved_lang = Lang::from_code(v),
                            ("lang_explicit", "true") => s.lang_explicit = true,
                            ("recent", v) if !v.is_empty() => s.recent.push(v.to_string()),
                            ("trust", v) if !v.is_empty() => s.trusted.push(v.to_string()),
                            _ => {}
                        }
                    }
                }
            }
        }
        if s.lang_explicit {
            if let Some(lang) = saved_lang {
                s.lang = lang;
            }
        }
        s
    }

    fn save(&self) {
        let theme = if self.theme_system {
            "system"
        } else if self.dark {
            "dark"
        } else {
            "light"
        };
        let mut out = format!(
            "theme={theme}\nlang={}\nlang_explicit={}\n",
            self.lang.code(),
            self.lang_explicit
        );
        for r in &self.recent {
            out.push_str(&format!("recent={r}\n"));
        }
        for t in &self.trusted {
            out.push_str(&format!("trust={t}\n"));
        }
        if let Some(path) = Self::path() {
            let _ = fs::write(path, out);
        }
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
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(9.0, 4.0);
    style.spacing.interact_size.y = 25.0;

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
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    v.widgets.inactive.bg_fill = p.idle;
    v.widgets.inactive.weak_bg_fill = p.idle;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border);
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

fn theme_toggle(ui: &mut egui::Ui, dark: &mut bool, system: &mut bool, tip: &str) -> bool {
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
                Stroke::new(1.6_f32, col.gamma_multiply(k)),
            );
        }
    }
    painter.rect_stroke(rect, 4.0, v.bg_stroke);

    if resp.clicked() {
        if *system {
            *system = false;
            *dark = true;
        } else if *dark {
            *dark = false;
        } else {
            *system = true;
            *dark = system_prefers_dark();
        }
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
        .filter(|e| is_project_dir(&e.path()))
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

fn t2(lang: Lang, ru: &'static str, en: &'static str) -> &'static str {
    if lang == Lang::Ru {
        ru
    } else {
        en
    }
}

// Папка проекта: есть main.lua или ic.toml
fn is_project_dir(dir: &Path) -> bool {
    dir.join("main.lua").is_file() || dir.join(ProjectSettings::FILE).is_file()
}

struct LangIcon {
    badge: String,
    color: Color32,
    texture: Option<egui::TextureHandle>,
}

#[derive(Default)]
struct LangSet {
    list: Vec<ic_build::langs::Language>,
    icons: HashMap<String, LangIcon>,
}

// PNG-значок языка (до 1024x1024)
fn load_png_texture(ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
    let file = fs::File::open(path).ok()?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0 || h == 0 || w > 1024 || h > 1024 {
        return None;
    }
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        _ => return None,
    };
    Some(ctx.load_texture(
        path.display().to_string(),
        egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba),
        egui::TextureOptions::LINEAR,
    ))
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', '\\']) && s != "." && s != ".."
}

fn file_color(name: &str) -> Color32 {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "lua" => Color32::from_rgb(0x51, 0xA0, 0xCF),
        "rs" => Color32::from_rgb(0xDE, 0x6A, 0x35),
        "cpp" | "cc" | "cxx" | "hpp" | "h" => Color32::from_rgb(0x65, 0x9A, 0xD8),
        "c" => Color32::from_rgb(0x8A, 0xA6, 0xC1),
        "cs" => Color32::from_rgb(0x9B, 0x4F, 0x96),
        "zig" => Color32::from_rgb(0xF7, 0xA4, 0x1D),
        "toml" => Color32::from_rgb(0xC0, 0x7A, 0x3A),
        "md" | "mdx" => Color32::from_rgb(0x6C, 0xB6, 0xFF),
        "json" => Color32::from_rgb(0xE5, 0xC0, 0x5A),
        "js" | "jsx" | "mjs" => Color32::from_rgb(0xD7, 0xB9, 0x4A),
        "ts" | "tsx" => Color32::from_rgb(0x3F, 0x83, 0xC9),
        "py" => Color32::from_rgb(0x4B, 0x9B, 0x79),
        "html" | "htm" => Color32::from_rgb(0xE1, 0x7B, 0x45),
        "css" | "scss" => Color32::from_rgb(0xA5, 0x75, 0xC5),
        "yml" | "yaml" => Color32::from_rgb(0xB5, 0x78, 0xA5),
        "wgsl" | "glsl" | "vert" | "frag" => Color32::from_rgb(0xB0, 0x83, 0xD4),
        "png" | "jpg" | "jpeg" | "webp" | "svg" => Color32::from_rgb(0x64, 0xB5, 0x8A),
        _ => Color32::from_rgb(0x8A, 0x8F, 0x98),
    }
}

fn explorer_rows(
    ui: &mut egui::Ui,
    proj: &Project,
    dark: bool,
    lang: Lang,
    langs: &LangSet,
    dragging_file: &mut Option<PathBuf>,
    action: &mut Option<Action>,
) {
    let pal = palette(dark);
    let row_h = 20.0;
    let indent = 12.0;
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
                    Stroke::new(1.0_f32, pal.border),
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
                    egui::pos2(x + 18.0, cy - 7.0),
                    egui::vec2(22.0, 14.0),
                );
                let ext = f.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                let glyph = match ext.as_str() {
                    "cpp" | "cc" | "cxx" | "hpp" | "h" => "C++",
                    "rs" => "Rs",
                    "lua" => "Lu",
                    "js" | "jsx" | "mjs" => "JS",
                    "ts" | "tsx" => "TS",
                    "py" => "Py",
                    "json" => "{}",
                    "toml" => "T",
                    "md" | "mdx" => "M",
                    "html" | "htm" => "5",
                    "css" | "scss" => "#",
                    "wgsl" | "glsl" => "◇",
                    "cs" => "C#",
                    "zig" => "Zg",
                    _ => "·",
                };
                // Свой язык: значок из описания, иначе встроенный
                if let Some(custom) = langs.icons.get(&ext) {
                    painter.rect_filled(body, 3.0, custom.color.gamma_multiply(0.22));
                    if let Some(tex) = &custom.texture {
                        let icon =
                            egui::Rect::from_center_size(body.center(), egui::vec2(16.0, 16.0));
                        painter.image(
                            tex.id(),
                            icon,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        painter.text(
                            body.center(),
                            egui::Align2::CENTER_CENTER,
                            &custom.badge,
                            FontId::proportional(9.0),
                            custom.color,
                        );
                    }
                } else {
                    painter.rect_filled(body, 3.0, file_color(&f.name).gamma_multiply(0.22));
                    painter.text(
                        body.center(),
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        FontId::proportional(9.0),
                        file_color(&f.name),
                    );
                }
            }
            painter.text(
                egui::pos2(x + 38.0, cy),
                egui::Align2::LEFT_CENTER,
                &f.name,
                egui::FontId::proportional(13.0),
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

fn fuzzy_score(candidate: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let candidate = candidate.to_lowercase();
    let query = query.to_lowercase();
    let mut score = 0;
    let mut position = 0;
    let mut previous = None;
    for ch in query.chars() {
        let relative = candidate[position..].find(ch)?;
        let found = position + relative;
        score += 10;
        if found == 0
            || candidate.as_bytes().get(found.wrapping_sub(1)) == Some(&b'/')
            || candidate.as_bytes().get(found.wrapping_sub(1)) == Some(&b'_')
            || candidate.as_bytes().get(found.wrapping_sub(1)) == Some(&b'-')
        {
            score += 12;
        }
        if previous.is_some_and(|last| found == last + 1) {
            score += 8;
        }
        score -= relative as i32;
        previous = Some(found);
        position = found + ch.len_utf8();
    }
    Some(score - candidate.len() as i32 / 8)
}

struct Project {
    dir: PathBuf,
    name: String,
    files: Vec<FileEntry>,
    tabs: Vec<PathBuf>,
    open: Option<PathBuf>,
    text: String,
    dirty: bool,
    note: Option<String>,
    collapsed: HashSet<PathBuf>,
    selected: PathBuf,
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

#[derive(Clone)]
struct SceneNode {
    kind: String,
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
        if dir.exists() && (!dir.is_dir() || !project_path_is_safe(project_dir, &dir)) {
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
                if (parts.len() != 9 && parts.len() != 10) || parts[0] != "node" {
                    continue;
                }
                let offset = if parts.len() == 10 { 1 } else { 0 };
                let parsed = (
                    parts[2 + offset].parse::<f32>(),
                    parts[3 + offset].parse::<f32>(),
                    parts[4 + offset].parse::<f32>(),
                    parts[5 + offset].parse::<f32>(),
                    parts[6 + offset].parse::<u8>(),
                    parts[7 + offset].parse::<u8>(),
                    parts[8 + offset].parse::<u8>(),
                );
                if let (Ok(x), Ok(y), Ok(width), Ok(height), Ok(r), Ok(g), Ok(b)) = parsed {
                    self.nodes.push(SceneNode {
                        kind: if offset == 1 {
                            parts[1].to_string()
                        } else {
                            "Node2D".to_string()
                        },
                        name: if offset == 1 {
                            parts[2].replace("%7C", "|")
                        } else {
                            parts[1].replace("%7C", "|")
                        },
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
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| format!("scene save error: {e}"))?;
            }
        }
        let mut out = String::new();
        for node in &self.nodes {
            let name = node.name.replace('|', "%7C").replace('\n', " ");
            out.push_str(&format!(
                "node|{}|{}|{:.2}|{:.2}|{:.2}|{:.2}|{}|{}|{}\n",
                node.kind,
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

    fn add_node_kind(&mut self, kind: &str) {
        let n = self.nodes.len();
        // Размер и цвет по виду узла
        let (width, height, color) = scene_ui::node_defaults(kind);
        let base = kind.to_string();
        let count = self.nodes.iter().filter(|node| node.kind == kind).count();
        self.nodes.push(SceneNode {
            kind: base.clone(),
            name: if count == 0 {
                base
            } else {
                format!("{}{}", base, count + 1)
            },
            x: 60.0 + n as f32 * 18.0,
            y: 60.0 + n as f32 * 18.0,
            width,
            height,
            color,
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
    CloseScene,
    SaveScene,
    RunLang(String),
    TrustLang(String),
    CancelTrust,
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
    command_palette: bool,
    command_filter: String,
    project_search: bool,
    project_search_query: String,
    search_current_file: bool,
    scene_editor: SceneEditor,
    window_title: String,
    ctx: egui::Context,
    langs: LangSet,
    trust_prompt: Option<String>,
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
        let mut app = Self {
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
            command_palette: false,
            command_filter: String::new(),
            project_search: false,
            project_search_query: String::new(),
            search_current_file: false,
            scene_editor: SceneEditor::default(),
            window_title: "IC".into(),
            ctx: cc.egui_ctx.clone(),
            langs: LangSet::default(),
            trust_prompt: None,
        };
        app.reload_langs();
        app
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
        if !is_project_dir(&dir) {
            self.start_error = Some(tr(lang, K::ErrNotProject).to_string());
            return;
        }
        let mut files = Vec::new();
        scan(&dir, 0, &mut files);
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());
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
        };
        if dir.join("main.lua").is_file() {
            load_file(&mut proj, dir.join("main.lua"), lang);
        }
        self.project = Some(proj);
        self.scene_editor.open_project(&dir);
        self.scene_editor.open = false;
        self.reload_langs();
        self.start_error = None;
        self.settings.push_recent(&dir);
        self.settings.save();
    }

    fn open_file(&mut self, path: PathBuf) {
        self.save();
        let lang = self.settings.lang;
        if let Some(p) = self.project.as_mut() {
            if !project_path_is_safe(&p.dir, &path) {
                self.log
                    .push("open refused: file is outside the project".into());
                return;
            }
            load_file(p, path, lang);
            if let Some(o) = p.open.clone() {
                p.selected = o;
            }
        }
    }

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
                self.log
                    .push("save refused: file is outside the project".into());
                return;
            }
            match fs::write(path, &p.text) {
                Ok(()) => p.dirty = false,
                Err(e) => self.log.push(format!("save error: {e}")),
            }
        }
    }

    fn save_scene(&mut self, force: bool) {
        let Some(path) = self.scene_editor.path.clone() else {
            return;
        };
        if !force && !self.scene_editor.dirty {
            return;
        }
        let existed = path.is_file();
        match self.scene_editor.save() {
            Ok(()) => {
                self.scene_editor.status = Some(tr(self.settings.lang, K::SceneSaved).to_string());
            }
            Err(e) => self.scene_editor.status = Some(e),
        }
        if !existed {
            if let Some(proj) = self.project.as_mut() {
                proj.files.clear();
                scan(&proj.dir, 0, &mut proj.files);
            }
        }
    }

    fn reload_langs(&mut self) {
        let mut dirs = Vec::new();
        if let Some(d) = ic_build::confine::data_dir() {
            dirs.push(d.join("languages"));
        }
        if let Some(p) = &self.project {
            dirs.push(p.dir.join("languages"));
        }
        let mut list = Vec::new();
        for d in dirs {
            let (found, errors) = ic_build::langs::load_dir(&d);
            self.log.extend(errors);
            list.extend(found);
        }
        let mut icons = HashMap::new();
        for l in &list {
            let texture = l.icon.as_ref().and_then(|p| load_png_texture(&self.ctx, p));
            for ext in &l.extensions {
                icons.insert(
                    ext.clone(),
                    LangIcon {
                        badge: l.badge.clone(),
                        color: Color32::from_rgb(l.color[0], l.color[1], l.color[2]),
                        texture: texture.clone(),
                    },
                );
            }
        }
        self.langs = LangSet { list, icons };
    }

    fn run_lang(&mut self, ctx: &egui::Context, l: ic_build::langs::Language) {
        let Some(proj) = self.project.as_ref() else {
            return;
        };
        let Some(file) = proj.open.clone() else {
            return;
        };
        if !project_path_is_safe(&proj.dir, &file) {
            self.log
                .push("run refused: file is outside the project".into());
            return;
        }
        let out_dir = proj.dir.join("build");
        if let Err(e) = fs::create_dir_all(&out_dir) {
            self.log.push(format!("{}: {e}", out_dir.display()));
            return;
        }
        let mut out = out_dir.join(file.file_stem().unwrap_or_default());
        if cfg!(windows) {
            out.set_extension("exe");
        }
        let compiler = l
            .compiler
            .as_ref()
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (file_s, out_s, dir_s) = (
            file.to_string_lossy().into_owned(),
            out.to_string_lossy().into_owned(),
            proj.dir.to_string_lossy().into_owned(),
        );
        let vars = [
            ("compiler", compiler.as_str()),
            ("file", file_s.as_str()),
            ("out", out_s.as_str()),
            ("dir", dir_s.as_str()),
        ];
        let steps = [
            ic_build::langs::expand(&l.build, &vars),
            ic_build::langs::expand(&l.run, &vars),
        ];
        if steps[0].is_empty() {
            self.log.push(format!("{}: пустая команда build", l.name));
            return;
        }
        let cwd = proj.dir.clone();
        let data = ic_build::confine::data_dir();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            for step in steps {
                if step.is_empty() {
                    continue;
                }
                let _ = tx.send(format!("> {}", step.join(" ")));
                let mut cmd = Command::new(&step[0]);
                cmd.args(&step[1..])
                    .current_dir(&cwd)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                if let Some(d) = &data {
                    ic_build::confine::child_env(&mut cmd, d);
                }
                hide_console(&mut cmd);
                match cmd.spawn() {
                    Err(e) => {
                        let _ = tx.send(format!("error: {e}"));
                        break;
                    }
                    Ok(mut child) => {
                        let pumps = [
                            pump(child.stdout.take(), tx.clone(), ctx.clone()),
                            pump(child.stderr.take(), tx.clone(), ctx.clone()),
                        ];
                        for h in pumps.into_iter().flatten() {
                            let _ = h.join();
                        }
                        match child.wait() {
                            Ok(status) => {
                                let _ = tx.send(format!("[{status}]"));
                                if !status.success() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
            ctx.request_repaint();
        });
    }

    fn project_dir(&self) -> Option<String> {
        self.project
            .as_ref()
            .map(|p| p.dir.to_string_lossy().into_owned())
    }

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
            Action::Save => {
                self.save();
                self.save_scene(false);
            }
            Action::SaveScene => self.save_scene(true),
            Action::CloseScene => {
                self.save_scene(false);
                self.scene_editor.open = false;
            }
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
            Action::OpenFile(p) | Action::SelectTab(p) => {
                self.scene_editor.open = false;
                self.open_file(p);
            }
            Action::CloseTab(p) => self.close_tab(p),
            Action::MovePath(source, destination) => self.move_project_file(source, destination),
            Action::OpenScene => {
                if let Some(dir) = self.project.as_ref().map(|p| p.dir.clone()) {
                    if self.scene_editor.path.is_none() {
                        self.scene_editor.open_project(&dir);
                    }
                    self.scene_editor.open = self.scene_editor.path.is_some();
                }
            }
            Action::RunLang(id) => {
                self.save();
                let found = self.langs.list.iter().find(|l| l.id == id).cloned();
                if let Some(l) = found {
                    let compiler = l
                        .compiler
                        .as_ref()
                        .map(|c| c.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    if self.settings.trusted.contains(&compiler) {
                        self.run_lang(ctx, l);
                    } else {
                        self.trust_prompt = Some(id);
                    }
                }
            }
            Action::TrustLang(id) => {
                self.trust_prompt = None;
                let found = self.langs.list.iter().find(|l| l.id == id).cloned();
                if let Some(l) = found {
                    if let Some(c) = &l.compiler {
                        self.settings.trusted.push(c.to_string_lossy().into_owned());
                        self.settings.save();
                    }
                    self.run_lang(ctx, l);
                }
            }
            Action::CancelTrust => self.trust_prompt = None,
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
                    self.settings_draft = Some(SettingsDraft { name, s: settings });
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
                    self.log
                        .push("Settings save refused: path is outside the project".into());
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
}

impl eframe::App for IcApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(line) = self.rx.try_recv() {
            self.log.push(line);
        }
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            if let Some(proj) = self.project.as_mut() {
                let assets = proj.dir.join("assets");
                if !assets.exists() {
                    let _ = fs::create_dir(&assets);
                }
                if !assets.is_dir() || !project_path_is_safe(&proj.dir, &assets) {
                    self.log
                        .push("Import refused: assets folder is outside the project".into());
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
                                    dropped
                                        .name
                                        .replace('\\', "/")
                                        .rsplit('/')
                                        .next()
                                        .filter(|name| {
                                            !name.is_empty() && *name != "." && *name != ".."
                                        })
                                        .unwrap_or("dropped_asset.bin")
                                        .to_string()
                                }
                            });
                        let destination = unique_path(&assets, &name);
                        if !project_path_is_safe(&proj.dir, &destination) {
                            self.log
                                .push("Import refused: destination is outside the project".into());
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
        let window_title = self
            .project
            .as_ref()
            .map(|project| format!("IC - \"{}\"", project.name))
            .unwrap_or_else(|| "IC".to_string());
        if self.window_title != window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title.clone()));
            self.window_title = window_title;
        }
        let lang = self.settings.lang;
        let dark = self.settings.dark;
        let p = palette(dark);
        let theme_tip = format!(
            "{}: {}",
            tr(lang, K::Theme),
            if self.settings.theme_system {
                "System"
            } else if dark {
                "Dark"
            } else {
                "Light"
            }
        );

        // Кнопка запуска для открытого файла на своём языке
        let run_lang: Option<(String, String)> = self
            .project
            .as_ref()
            .and_then(|proj| proj.open.as_ref())
            .and_then(|file| file.file_name())
            .and_then(|name| {
                self.langs
                    .list
                    .iter()
                    .find(|l| l.matches(&name.to_string_lossy()))
            })
            .filter(|l| l.compiler.is_some() && !l.build.is_empty())
            .map(|l| (l.id.clone(), l.name.clone()));

        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::none()
                    .fill(p.panel)
                    .inner_margin(Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.project.is_some() {
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
                        if let Some((id, name)) = &run_lang {
                            if primary_button(ui, &format!("▶ {name}")).clicked() {
                                action = Some(Action::RunLang(id.clone()));
                            }
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
                        if theme_toggle(
                            ui,
                            &mut self.settings.dark,
                            &mut self.settings.theme_system,
                            &theme_tip,
                        ) {
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
                                        self.settings.lang_explicit = true;
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
        if self.settings.theme_system {
            let system_dark = system_prefers_dark();
            if self.settings.dark != system_dark {
                self.settings.dark = system_dark;
                setup_style(ctx, system_dark);
            }
        }
        let lang = self.settings.lang;
        let dark = self.settings.dark;
        let p = palette(dark);

        if has_project {
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
            if ctx.input_mut(|i| i.consume_key(command_shift, egui::Key::P)) {
                self.command_filter.clear();
                self.command_palette = true;
            }
            if ctx.input_mut(|i| i.consume_key(command, egui::Key::F)) {
                self.project_search_query.clear();
                self.search_current_file = false;
                self.project_search = true;
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
                .default_height(118.0)
                .min_height(48.0)
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
                .width_range(150.0..=480.0)
                .frame(
                    egui::Frame::none()
                        .fill(p.panel)
                        .inner_margin(Margin::symmetric(0.0, 6.0)),
                )
                .show(ctx, |ui| {
                    let Some(proj) = self.project.as_ref() else {
                        return;
                    };
                    // Дерево сцены над файлами, граница двигается мышью
                    if self.scene_editor.open {
                        egui::TopBottomPanel::top("scene_dock")
                            .resizable(true)
                            .default_height(230.0)
                            .height_range(90.0..=700.0)
                            .frame(egui::Frame::none().inner_margin(Margin::symmetric(10.0, 4.0)))
                            .show_inside(ui, |ui| {
                                scene_ui::scene_tree_ui(ui, &mut self.scene_editor, lang);
                            });
                    }
                    let mut base = proj.selected.clone();
                    if !base.is_dir() {
                        base = base
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| proj.dir.clone());
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new(tr(lang, K::Explorer)).small().strong());
                    });
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(6.0);
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
                    ui.add_space(4.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!("▾  📁 {}", proj.name))
                                .strong()
                                .small(),
                        );
                    });
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            explorer_rows(
                                ui,
                                proj,
                                dark,
                                lang,
                                &self.langs,
                                &mut self.dragging_file,
                                &mut action,
                            );
                            if ui.input(|i| i.pointer.any_released()) {
                                self.dragging_file = None;
                            }
                        });
                });
            // Свойства узла справа, как в Godot
            if self.scene_editor.open {
                egui::SidePanel::right("scene_inspector")
                    .resizable(true)
                    .default_width(230.0)
                    .width_range(180.0..=420.0)
                    .frame(
                        egui::Frame::none()
                            .fill(p.panel)
                            .inner_margin(Margin::same(10.0)),
                    )
                    .show(ctx, |ui| {
                        scene_ui::scene_inspector_ui(ui, &mut self.scene_editor, lang);
                    });
            }
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
                    // Переключатель: скрипт или сцена
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(!self.scene_editor.open, t2(lang, "Скрипт", "Script"))
                            .clicked()
                        {
                            action = Some(Action::CloseScene);
                        }
                        if ui
                            .selectable_label(self.scene_editor.open, tr(lang, K::SceneEditor))
                            .clicked()
                        {
                            action = Some(Action::OpenScene);
                        }
                    });
                    if self.scene_editor.open {
                        if scene_ui::scene_toolbar_ui(ui, &mut self.scene_editor, lang) {
                            action = Some(Action::SaveScene);
                        }
                        scene_ui::scene_canvas_ui(ui, &mut self.scene_editor, lang, dark, p.editor);
                        return;
                    }
                    if proj.dirty {
                        ui.label(
                            RichText::new(format!("● {}", tr(lang, K::Unsaved)))
                                .color(Color32::from_rgb(0xE5, 0xA0, 0x3B)),
                        );
                    }
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
            let exists =
                name_ok && is_project_dir(&resolve_dir(&self.location).join(self.name.trim()));
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
                                            .range(320..=7680)
                                            .suffix(" px"),
                                    );
                                    ui.label("×");
                                    ui.add(
                                        egui::DragValue::new(&mut draft.s.height)
                                            .range(240..=4320)
                                            .suffix(" px"),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.label(tr(lang, K::SetTick));
                                ui.add(
                                    egui::DragValue::new(&mut draft.s.tick_rate)
                                        .range(10..=480)
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
        if self.project_search {
            let mut open = true;
            egui::Window::new(if self.search_current_file {
                "Find in file"
            } else {
                "Search in project"
            })
            .open(&mut open)
            .default_width(680.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Find");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.project_search_query)
                            .hint_text("Search across project files…")
                            .desired_width(f32::INFINITY),
                    );
                });
                if let Some(project) = self.project.as_ref() {
                    let query = self.project_search_query.to_lowercase();
                    let mut results: Vec<(PathBuf, usize, String)> = Vec::new();
                    if !query.is_empty() {
                        for entry in project.files.iter().filter(|entry| {
                            !entry.is_dir
                                && (!self.search_current_file
                                    || project.open.as_ref() == Some(&entry.path))
                        }) {
                            if let Ok(contents) = fs::read_to_string(&entry.path) {
                                for (line_index, line) in contents.lines().enumerate() {
                                    if line.to_lowercase().contains(&query) {
                                        results.push((
                                            entry.path.clone(),
                                            line_index + 1,
                                            line.trim().to_string(),
                                        ));
                                        if results.len() >= 500 {
                                            break;
                                        }
                                    }
                                }
                            }
                            if results.len() >= 500 {
                                break;
                            }
                        }
                    }
                    ui.label(format!("{} results", results.len()));
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (path, line, text) in results {
                            let relative =
                                path.strip_prefix(&project.dir).unwrap_or(&path).display();
                            if ui
                                .selectable_label(false, format!("{}:{}  {}", relative, line, text))
                                .clicked()
                            {
                                action = Some(Action::SelectTab(path));
                                self.project_search = false;
                            }
                        }
                    });
                }
            });
            if !open {
                self.project_search = false;
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
                        let filter = self.quick_filter.clone();
                        let mut ranked: Vec<(i32, String, PathBuf)> = proj
                            .files
                            .iter()
                            .filter(|file| !file.is_dir)
                            .filter_map(|file| {
                                let label = file
                                    .path
                                    .strip_prefix(&proj.dir)
                                    .unwrap_or(&file.path)
                                    .display()
                                    .to_string();
                                fuzzy_score(&label, &filter)
                                    .map(|score| (score, label, file.path.clone()))
                            })
                            .collect();
                        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .show(ui, |ui| {
                                for (_, label, path) in ranked.into_iter().take(100) {
                                    if ui
                                        .selectable_label(proj.open.as_ref() == Some(&path), label)
                                        .clicked()
                                    {
                                        action = Some(Action::SelectTab(path));
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

        if self.command_palette {
            let mut open = true;
            egui::Window::new(tr(lang, K::CommandPalette))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(520.0)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 70.0])
                .show(ctx, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.command_filter)
                            .hint_text(tr(lang, K::TypeCommand))
                            .desired_width(f32::INFINITY),
                    );
                    let query = self.command_filter.to_lowercase();
                    let commands = [
                        (tr(lang, K::CommandSave), "save"),
                        (tr(lang, K::CommandRun), "run"),
                        (tr(lang, K::CommandBuild), "build"),
                        (tr(lang, K::CommandQuickOpen), "quick open"),
                        (tr(lang, K::CommandSearch), "search"),
                        (tr(lang, K::CommandScene), "scene"),
                        (tr(lang, K::CommandSettings), "settings"),
                        (tr(lang, K::CommandCloseProject), "close project"),
                    ];
                    for (label, command) in commands {
                        if !query.is_empty()
                            && !label.to_lowercase().contains(&query)
                            && !command.contains(&query)
                        {
                            continue;
                        }
                        if ui.selectable_label(false, label).clicked() {
                            action = Some(match command {
                                "save" => Action::Save,
                                "run" => Action::Run,
                                "build" => Action::Build,
                                "scene" => Action::OpenScene,
                                "settings" => Action::OpenSettings,
                                "close project" => Action::CloseProject,
                                "quick open" => {
                                    self.quick_filter.clear();
                                    self.quick_open = true;
                                    self.command_palette = false;
                                    return;
                                }
                                "search" => {
                                    self.project_search_query.clear();
                                    self.search_current_file = false;
                                    self.project_search = true;
                                    self.command_palette = false;
                                    return;
                                }
                                _ => continue,
                            });
                            self.command_palette = false;
                        }
                    }
                });
            if !open {
                self.command_palette = false;
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
        if let Some(id) = self.trust_prompt.clone() {
            if let Some(l) = self.langs.list.iter().find(|l| l.id == id) {
                let compiler = l
                    .compiler
                    .as_ref()
                    .map(|c| c.display().to_string())
                    .unwrap_or_default();
                let command = l.build.join(" ");
                egui::Window::new(t2(lang, "Запустить компилятор?", "Run the compiler?"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(t2(
                            lang,
                            "IC запустит этот компилятор:",
                            "IC will run this compiler:",
                        ));
                        ui.monospace(&compiler);
                        ui.label(t2(lang, "Команда сборки:", "Build command:"));
                        ui.monospace(&command);
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if primary_button(
                                ui,
                                t2(lang, "Разрешить и запустить", "Allow and run"),
                            )
                            .clicked()
                            {
                                action = Some(Action::TrustLang(id.clone()));
                            }
                            if button(ui, tr(lang, K::Cancel)).clicked() {
                                action = Some(Action::CancelTrust);
                            }
                        });
                    });
            } else {
                self.trust_prompt = None;
            }
        }
        if let Some(a) = action {
            self.handle(ctx, a);
        }
    }
}

pub fn run() -> eframe::Result<()> {
    confine_caches().map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::other(e))))?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([980.0, 560.0])
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

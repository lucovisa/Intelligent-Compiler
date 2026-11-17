use super::*;
use std::cell::RefCell;

// Значки Material Icon Theme (MIT), растр 36×36; таблица соответствий — из того же набора
const PAK: &[u8] = include_bytes!("../assets/icons/icons.pak");
const MAP: &str = include_str!("../assets/icons/icon_map.txt");

pub struct IconSet {
    ctx: egui::Context,
    png: HashMap<&'static str, (usize, usize)>,
    ext: HashMap<&'static str, &'static str>,
    file: HashMap<&'static str, &'static str>,
    dir: HashMap<&'static str, &'static str>,
    light: HashSet<&'static str>,
    cache: RefCell<HashMap<String, Option<egui::TextureHandle>>>,
}

impl IconSet {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut set = Self {
            ctx: ctx.clone(),
            png: HashMap::new(),
            ext: HashMap::new(),
            file: HashMap::new(),
            dir: HashMap::new(),
            light: HashSet::new(),
            cache: RefCell::new(HashMap::new()),
        };
        for line in MAP.lines() {
            let mut p = line.split(' ');
            match (p.next(), p.next(), p.next(), p.next()) {
                (Some("png"), Some(n), Some(o), Some(l)) => {
                    if let (Ok(o), Ok(l)) = (o.parse(), l.parse()) {
                        set.png.insert(n, (o, l));
                    }
                }
                (Some("ext"), Some(k), Some(v), _) => {
                    set.ext.entry(k).or_insert(v);
                }
                (Some("file"), Some(k), Some(v), _) => {
                    set.file.entry(k).or_insert(v);
                }
                (Some("dir"), Some(k), Some(v), _) => {
                    set.dir.entry(k).or_insert(v);
                }
                (Some("light"), Some(n), _, _) => {
                    set.light.insert(n);
                }
                _ => {}
            }
        }
        set
    }

    pub fn has_ext(&self, ext: &str) -> bool {
        self.ext.contains_key(ext)
    }

    fn texture(&self, key: &str) -> Option<egui::TextureHandle> {
        if let Some(t) = self.cache.borrow().get(key) {
            return t.clone();
        }
        let loaded = self.png.get(key).and_then(|(o, l)| {
            let image = decode_png(std::io::Cursor::new(&PAK[*o..*o + *l]))?;
            Some(self.ctx.load_texture(format!("icon-{key}"), image, egui::TextureOptions::LINEAR))
        });
        self.cache.borrow_mut().insert(key.to_string(), loaded.clone());
        loaded
    }

    fn draw(&self, painter: &egui::Painter, rect: egui::Rect, key: &str, dark: bool) {
        let name = if !dark && self.light.contains(key) {
            format!("{key}_light")
        } else {
            key.to_string()
        };
        if let Some(t) = self.texture(&name) {
            painter.image(
                t.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    // Имя файла → значок: сначала точное имя, потом самое длинное расширение
    fn file_key(&self, name: &str) -> &'static str {
        let lower = name.to_ascii_lowercase();
        if let Some(k) = self.file.get(lower.as_str()) {
            return k;
        }
        for (i, c) in lower.char_indices() {
            if c == '.' && i + 1 < lower.len() {
                if let Some(k) = self.ext.get(&lower[i + 1..]) {
                    return k;
                }
            }
        }
        "document"
    }

    pub fn paint_file(&self, painter: &egui::Painter, rect: egui::Rect, name: &str, dark: bool) {
        self.draw(painter, rect, self.file_key(name), dark);
    }

    pub fn paint_folder(&self, painter: &egui::Painter, rect: egui::Rect, name: &str, open: bool, dark: bool) {
        let base = self.dir.get(name.to_ascii_lowercase().as_str()).copied().unwrap_or("folder-base");
        let key = if open { format!("{base}-open") } else { base.to_string() };
        if self.png.contains_key(key.as_str()) {
            self.draw(painter, rect, &key, dark);
        } else {
            self.draw(painter, rect, base, dark);
        }
    }

    pub fn paint_root(&self, painter: &egui::Painter, rect: egui::Rect, open: bool, dark: bool) {
        self.draw(painter, rect, if open { "folder-root-open" } else { "folder-root" }, dark);
    }
}

#[derive(Clone, Copy)]
pub enum Tool {
    NewFile,
    NewFolder,
    NewScene,
}

// Кнопка-значок для шапки проводника
pub fn tool_button(ui: &mut egui::Ui, set: &IconSet, tool: Tool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 24.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let icon = egui::Rect::from_center_size(rect.center() + egui::vec2(-2.0, 0.0), egui::vec2(16.0, 16.0));
    let dark = ui.visuals().dark_mode;
    let painter = ui.painter();
    match tool {
        Tool::NewFile => set.paint_file(painter, icon, "a.txt", dark),
        Tool::NewFolder => set.paint_folder(painter, icon, "", false, dark),
        Tool::NewScene => set.paint_file(painter, icon, "a.ic-scene", dark),
    }
    let plus = rect.right_bottom() + egui::vec2(-5.0, -5.0);
    let color = ui.visuals().text_color();
    painter.line_segment([plus + egui::vec2(-3.0, 0.0), plus + egui::vec2(3.0, 0.0)], Stroke::new(1.5_f32, color));
    painter.line_segment([plus + egui::vec2(0.0, -3.0), plus + egui::vec2(0.0, 3.0)], Stroke::new(1.5_f32, color));
    resp.on_hover_text(tip)
}

// Название языка для строки состояния
pub fn language_name(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "lua" => "Lua",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => "C++",
        "c" | "h" => "C",
        "cs" => "C#",
        "rs" => "Rust",
        "zig" => "Zig",
        "toml" => "TOML",
        "json" => "JSON",
        "md" => "Markdown",
        "wgsl" => "WGSL",
        "glsl" => "GLSL",
        _ => "Text",
    }
}


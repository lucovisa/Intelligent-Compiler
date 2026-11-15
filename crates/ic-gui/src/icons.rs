use super::*;

// Значки из Material Icon Theme (MIT), растр 36×36 для чёткости на любом экране
const ASSETS: &[(&str, &[u8])] = &[
    ("lua", include_bytes!("../assets/icons/lua.png")),
    ("cpp", include_bytes!("../assets/icons/cpp.png")),
    ("c", include_bytes!("../assets/icons/c.png")),
    ("h", include_bytes!("../assets/icons/h.png")),
    ("hpp", include_bytes!("../assets/icons/hpp.png")),
    ("csharp", include_bytes!("../assets/icons/csharp.png")),
    ("rust", include_bytes!("../assets/icons/rust.png")),
    ("zig", include_bytes!("../assets/icons/zig.png")),
    ("json", include_bytes!("../assets/icons/json.png")),
    ("toml", include_bytes!("../assets/icons/toml.png")),
    ("markdown", include_bytes!("../assets/icons/markdown.png")),
    ("image", include_bytes!("../assets/icons/image.png")),
    ("audio", include_bytes!("../assets/icons/audio.png")),
    ("document", include_bytes!("../assets/icons/document.png")),
    ("javascript", include_bytes!("../assets/icons/javascript.png")),
    ("python", include_bytes!("../assets/icons/python.png")),
    ("console", include_bytes!("../assets/icons/console.png")),
    ("settings", include_bytes!("../assets/icons/settings.png")),
    ("shader", include_bytes!("../assets/icons/shader.png")),
    ("yaml", include_bytes!("../assets/icons/yaml.png")),
    ("xml", include_bytes!("../assets/icons/xml.png")),
    ("folder-base", include_bytes!("../assets/icons/folder-base.png")),
    ("folder-open", include_bytes!("../assets/icons/folder-open.png")),
    ("scene", include_bytes!("../assets/icons/scene.png")),
];

#[derive(Default)]
pub struct IconSet {
    tex: HashMap<&'static str, egui::TextureHandle>,
}

impl IconSet {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut tex = HashMap::new();
        for (name, bytes) in ASSETS {
            if let Some(image) = decode_png(std::io::Cursor::new(*bytes)) {
                tex.insert(
                    *name,
                    ctx.load_texture(format!("icon-{name}"), image, egui::TextureOptions::LINEAR),
                );
            }
        }
        Self { tex }
    }

    fn draw(&self, painter: &egui::Painter, rect: egui::Rect, key: &str) {
        if let Some(t) = self.tex.get(key) {
            painter.image(
                t.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    pub fn paint_file(&self, painter: &egui::Painter, rect: egui::Rect, name: &str) {
        self.draw(painter, rect, asset_key(name));
    }

    pub fn paint_folder(&self, painter: &egui::Painter, rect: egui::Rect, open: bool) {
        self.draw(painter, rect, if open { "folder-open" } else { "folder-base" });
    }
}

// Имя файла → значок
fn asset_key(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "lua" => "lua",
        "cpp" | "cc" | "cxx" => "cpp",
        "c" => "c",
        "h" => "h",
        "hpp" | "hh" => "hpp",
        "cs" => "csharp",
        "rs" => "rust",
        "zig" => "zig",
        "json" => "json",
        "toml" => "toml",
        "md" | "mdx" => "markdown",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" => "image",
        "wav" | "ogg" | "mp3" | "flac" => "audio",
        "js" | "mjs" => "javascript",
        "py" => "python",
        "sh" | "bat" | "cmd" | "ps1" => "console",
        "lang" | "theme" => "settings",
        "wgsl" | "glsl" | "vert" | "frag" => "shader",
        "yml" | "yaml" => "yaml",
        "xml" | "html" | "htm" => "xml",
        "ic-scene" => "scene",
        _ => "document",
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
    let painter = ui.painter();
    match tool {
        Tool::NewFile => set.paint_file(painter, icon, "a.txt"),
        Tool::NewFolder => set.paint_folder(painter, icon, false),
        Tool::NewScene => set.paint_file(painter, icon, "a.ic-scene"),
    }
    // Плюс в правом нижнем углу
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

// Есть ли встроенный значок для расширения
pub fn has_builtin_icon(ext: &str) -> bool {
    matches!(
        ext,
        "lua" | "toml" | "json" | "md" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "h" | "c" | "cs"
            | "rs" | "zig" | "py" | "js" | "mjs" | "png" | "jpg" | "jpeg" | "gif" | "webp"
            | "bmp" | "ico" | "wav" | "ogg" | "mp3" | "flac" | "wgsl" | "glsl" | "ic-scene"
    )
}

use super::*;

fn hexagon(painter: &egui::Painter, c: egui::Pos2, r: f32, fill: Color32) {
    // Шестиугольник остриём вверх
    let pts: Vec<egui::Pos2> = (0..6)
        .map(|k| {
            let a = (-90.0_f32 + 60.0 * k as f32).to_radians();
            egui::pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::NONE));
}

fn label(painter: &egui::Painter, c: egui::Pos2, text: &str, size: f32, color: Color32) {
    painter.text(
        c,
        egui::Align2::CENTER_CENTER,
        text,
        FontId::proportional(size),
        color,
    );
}

// Значок файла по расширению, rect ≈ 16×16
pub fn file_icon(painter: &egui::Painter, rect: egui::Rect, name: &str) {
    let s = rect.width();
    let c = rect.center();
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    let white = Color32::WHITE;
    match ext {
        "lua" => {
            // Синий шар с «луной», как логотип Lua
            let blue = Color32::from_rgb(0x4F, 0x86, 0xD9);
            painter.circle_filled(c, s * 0.42, blue);
            painter.circle_stroke(c, s * 0.2, Stroke::new(s * 0.1, white));
            painter.circle_filled(c + egui::vec2(s * 0.3, -s * 0.3), s * 0.15, white);
            painter.circle_stroke(
                c + egui::vec2(s * 0.3, -s * 0.3),
                s * 0.15,
                Stroke::new(s * 0.06, blue),
            );
        }
        "toml" => {
            painter.rect_filled(
                rect.shrink(s * 0.06),
                3.0,
                Color32::from_rgb(0xA5, 0x4B, 0x2A),
            );
            label(painter, c, "T", s * 0.7, white);
        }
        "json" => label(
            painter,
            c,
            "{}",
            s * 0.85,
            Color32::from_rgb(0xE5, 0xC0, 0x3A),
        ),
        "md" | "mdx" => {
            painter.rect_stroke(
                rect.shrink(s * 0.08),
                Rounding::same(3.0),
                Stroke::new(1.3_f32, Color32::from_rgb(0x56, 0xA5, 0xD8)),
            );
            label(
                painter,
                c,
                "M",
                s * 0.62,
                Color32::from_rgb(0x56, 0xA5, 0xD8),
            );
        }
        "cpp" | "cc" | "cxx" => {
            hexagon(painter, c, s * 0.5, Color32::from_rgb(0x2F, 0x76, 0xC8));
            label(painter, c, "C++", s * 0.42, white);
        }
        "hpp" | "h" | "hh" => {
            hexagon(painter, c, s * 0.5, Color32::from_rgb(0x9A, 0x6B, 0xC9));
            label(painter, c, "H", s * 0.55, white);
        }
        "c" => {
            hexagon(painter, c, s * 0.5, Color32::from_rgb(0x4A, 0x86, 0xC0));
            label(painter, c, "C", s * 0.58, white);
        }
        "cs" => {
            hexagon(painter, c, s * 0.5, Color32::from_rgb(0x84, 0x4F, 0xB0));
            label(painter, c, "C#", s * 0.46, white);
        }
        "rs" => {
            painter.circle_filled(c, s * 0.45, Color32::from_rgb(0xE0, 0x84, 0x55));
            label(
                painter,
                c,
                "R",
                s * 0.6,
                Color32::from_rgb(0x3A, 0x1D, 0x10),
            );
        }
        "zig" => label(
            painter,
            c,
            "Z",
            s * 0.95,
            Color32::from_rgb(0xF7, 0xA4, 0x1D),
        ),
        "py" => label(
            painter,
            c,
            "Py",
            s * 0.7,
            Color32::from_rgb(0x56, 0x9C, 0xD6),
        ),
        "js" | "mjs" => {
            painter.rect_filled(
                rect.shrink(s * 0.06),
                2.0,
                Color32::from_rgb(0xE5, 0xC0, 0x3A),
            );
            label(painter, c, "JS", s * 0.5, Color32::BLACK);
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => {
            let green = Color32::from_rgb(0x5C, 0xB8, 0x5C);
            painter.rect_stroke(
                rect.shrink(s * 0.1),
                Rounding::same(2.0),
                Stroke::new(1.3_f32, green),
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(rect.left() + s * 0.18, rect.bottom() - s * 0.2),
                    egui::pos2(rect.left() + s * 0.45, rect.top() + s * 0.45),
                    egui::pos2(rect.left() + s * 0.62, rect.bottom() - s * 0.2),
                ],
                green,
                Stroke::NONE,
            ));
            painter.circle_filled(
                egui::pos2(rect.right() - s * 0.28, rect.top() + s * 0.3),
                s * 0.09,
                green,
            );
        }
        "wav" | "ogg" | "mp3" | "flac" => {
            let purple = Color32::from_rgb(0xB0, 0x7C, 0xD8);
            painter.circle_filled(c + egui::vec2(-s * 0.12, s * 0.22), s * 0.17, purple);
            painter.line_segment(
                [
                    c + egui::vec2(s * 0.03, s * 0.2),
                    c + egui::vec2(s * 0.03, -s * 0.32),
                ],
                Stroke::new(1.5_f32, purple),
            );
            painter.line_segment(
                [
                    c + egui::vec2(s * 0.03, -s * 0.32),
                    c + egui::vec2(s * 0.3, -s * 0.18),
                ],
                Stroke::new(1.5_f32, purple),
            );
        }
        "wgsl" | "glsl" | "vert" | "frag" => {
            let pink = Color32::from_rgb(0xD9, 0x5B, 0xA6);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x, rect.top() + s * 0.06),
                    egui::pos2(rect.right() - s * 0.06, c.y),
                    egui::pos2(c.x, rect.bottom() - s * 0.06),
                    egui::pos2(rect.left() + s * 0.06, c.y),
                ],
                pink,
                Stroke::NONE,
            ));
        }
        "ic-scene" => {
            // Дерево из трёх узлов
            let teal = Color32::from_rgb(0x4F, 0xC1, 0xB0);
            let top = c + egui::vec2(0.0, -s * 0.28);
            let left = c + egui::vec2(-s * 0.26, s * 0.26);
            let right = c + egui::vec2(s * 0.26, s * 0.26);
            painter.line_segment([top, left], Stroke::new(1.3_f32, teal));
            painter.line_segment([top, right], Stroke::new(1.3_f32, teal));
            for p in [top, left, right] {
                painter.circle_filled(p, s * 0.13, teal);
            }
        }
        "lang" => {
            painter.rect_filled(
                rect.shrink(s * 0.06),
                3.0,
                Color32::from_rgb(0x6C, 0x8E, 0xBF),
            );
            label(painter, c, "L", s * 0.62, white);
        }
        _ => {
            // Обычный файл с загнутым уголком
            let gray = Color32::from_rgb(0x9A, 0xA0, 0xA8);
            let r = rect.shrink2(egui::vec2(s * 0.2, s * 0.08));
            let fold = s * 0.26;
            painter.add(egui::Shape::closed_line(
                vec![
                    r.left_top(),
                    egui::pos2(r.right() - fold, r.top()),
                    egui::pos2(r.right(), r.top() + fold),
                    r.right_bottom(),
                    r.left_bottom(),
                ],
                Stroke::new(1.2_f32, gray),
            ));
            painter.line_segment(
                [
                    egui::pos2(r.right() - fold, r.top()),
                    egui::pos2(r.right() - fold, r.top() + fold),
                ],
                Stroke::new(1.0_f32, gray),
            );
            painter.line_segment(
                [
                    egui::pos2(r.right() - fold, r.top() + fold),
                    egui::pos2(r.right(), r.top() + fold),
                ],
                Stroke::new(1.0_f32, gray),
            );
        }
    }
}

// Значок папки
pub fn folder_icon(painter: &egui::Painter, rect: egui::Rect, open: bool) {
    let s = rect.width();
    let back = Color32::from_rgb(0xB8, 0x8F, 0x4B);
    let front = Color32::from_rgb(0xDC, 0xB6, 0x7A);
    let x0 = rect.left() + s * 0.04;
    let x1 = rect.right() - s * 0.04;
    let y0 = rect.top() + s * 0.2;
    let y1 = rect.bottom() - s * 0.16;
    // Ушко папки
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(x0, y0 - s * 0.08),
            egui::pos2(x0 + s * 0.42, y0 + s * 0.1),
        ),
        Rounding::same(1.5),
        back,
    );
    if open {
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)),
            Rounding::same(1.5),
            back,
        );
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(x0 + s * 0.14, y0 + s * 0.2),
                egui::pos2(x1 + s * 0.04, y0 + s * 0.2),
                egui::pos2(x1 - s * 0.1, y1),
                egui::pos2(x0, y1),
            ],
            front,
            Stroke::NONE,
        ));
    } else {
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)),
            Rounding::same(1.5),
            front,
        );
    }
}

#[derive(Clone, Copy)]
pub enum Tool {
    NewFile,
    NewFolder,
    NewScene,
}

// Кнопка-значок для шапки проводника
pub fn tool_button(ui: &mut egui::Ui, tool: Tool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let icon = egui::Rect::from_center_size(
        rect.center() + egui::vec2(-1.0, 0.0),
        egui::vec2(14.0, 14.0),
    );
    let painter = ui.painter();
    match tool {
        Tool::NewFile => file_icon(painter, icon, "a.txt"),
        Tool::NewFolder => folder_icon(painter, icon, false),
        Tool::NewScene => file_icon(painter, icon, "a.ic-scene"),
    }
    // Плюс в правом нижнем углу
    let plus = rect.right_bottom() + egui::vec2(-5.0, -5.0);
    let color = ui.visuals().text_color();
    painter.line_segment(
        [plus + egui::vec2(-3.0, 0.0), plus + egui::vec2(3.0, 0.0)],
        Stroke::new(1.5_f32, color),
    );
    painter.line_segment(
        [plus + egui::vec2(0.0, -3.0), plus + egui::vec2(0.0, 3.0)],
        Stroke::new(1.5_f32, color),
    );
    resp.on_hover_text(tip)
}

// Название языка для строки состояния
pub fn language_name(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
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
        "lua"
            | "toml"
            | "json"
            | "md"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "hh"
            | "h"
            | "c"
            | "cs"
            | "rs"
            | "zig"
            | "py"
            | "js"
            | "mjs"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "ico"
            | "wav"
            | "ogg"
            | "mp3"
            | "flac"
            | "wgsl"
            | "glsl"
            | "ic-scene"
    )
}

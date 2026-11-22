use eframe::egui::{self, Color32, RichText};

// Яркость цвета
fn luma(c: Color32) -> f32 {
    let f = |v: u8| {
        let s = f32::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

// Текст под фоном: белый на тёмном синем, чёрный на светлом
pub fn on(bg: Color32) -> Color32 {
    if luma(bg) > 0.22 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

// Цвет поверх фона с прозрачностью
pub fn over(fg: Color32, alpha: f32, bg: Color32) -> Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) * alpha + f32::from(b) * (1.0 - alpha)).round() as u8;
    Color32::from_rgb(
        mix(fg.r(), bg.r()),
        mix(fg.g(), bg.g()),
        mix(fg.b(), bg.b()),
    )
}

// Выбранная подпись: цвет текста по фону выделения
pub fn sel_label(ui: &mut egui::Ui, selected: bool, text: impl Into<String>) -> egui::Response {
    let text = text.into();
    if selected {
        let color = on(ui.visuals().selection.bg_fill);
        ui.selectable_label(true, RichText::new(text).color(color))
    } else {
        ui.selectable_label(false, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_follows_background() {
        assert_eq!(on(Color32::from_rgb(0x00, 0x78, 0xD4)), Color32::WHITE);
        assert_eq!(on(Color32::from_rgb(0x0E, 0x63, 0x9C)), Color32::WHITE);
        assert_eq!(
            on(over(
                Color32::from_rgb(0x00, 0x78, 0xD4),
                0.5,
                Color32::from_rgb(0xF3, 0xF3, 0xF3)
            )),
            Color32::BLACK
        );
        assert_eq!(on(Color32::WHITE), Color32::BLACK);
    }
}

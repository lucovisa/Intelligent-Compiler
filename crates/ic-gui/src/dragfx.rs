use super::*;

// Анимированный «призрак» при перетаскивании: плавно догоняет курсор, появляется с масштабом и тенью
pub fn ghost(
    ctx: &egui::Context,
    id: &str,
    label: &str,
    icon: Option<egui::TextureId>,
    fill: Color32,
    accent: Color32,
) {
    let Some(pointer) = ctx.input(|i| i.pointer.latest_pos()) else {
        return;
    };
    let key = egui::Id::new(("ghost_pos", id));
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let prev: egui::Pos2 = ctx.data(|d| d.get_temp(key)).unwrap_or(pointer);
    let k = 1.0 - (-22.0 * dt).exp();
    let pos = prev + (pointer - prev) * k;
    ctx.data_mut(|d| d.insert_temp(key, pos));
    let appear = ctx.animate_bool_with_time(egui::Id::new(("ghost_on", id)), true, 0.14);
    // Отставание от курсора даёт лёгкий наклон
    let lag = ((pointer.x - pos.x) * 0.5).clamp(-14.0, 14.0);

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("ic_drag_ghost"),
    ));
    let galley = painter.layout_no_wrap(
        label.to_string(),
        FontId::proportional(13.0),
        Color32::WHITE,
    );
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let size = egui::vec2(galley.size().x + 16.0 + icon_w, 26.0) * (0.8 + 0.2 * appear);
    let rect = egui::Rect::from_min_size(pos + egui::vec2(14.0 + lag, 10.0), size);
    let alpha = |c: Color32, a: f32| {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a) as u8)
    };
    painter.rect_filled(
        rect.translate(egui::vec2(3.0, 5.0)),
        6.0,
        Color32::from_black_alpha((110.0 * appear) as u8),
    );
    painter.rect_filled(rect, 6.0, alpha(fill, 0.96 * appear));
    painter.rect_stroke(
        rect,
        Rounding::same(6.0),
        Stroke::new(1.5_f32, alpha(accent, appear)),
    );
    if let Some(tex) = icon {
        painter.image(
            tex,
            egui::Rect::from_min_size(rect.min + egui::vec2(5.0, 4.0), egui::vec2(18.0, 18.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            alpha(Color32::WHITE, appear),
        );
    }
    painter.galley(
        egui::pos2(
            rect.min.x + 8.0 + icon_w,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        Color32::WHITE,
    );
    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    ctx.request_repaint();
}

// Перетаскивание закончилось: сбрасываем позицию и появление
pub fn end(ctx: &egui::Context, id: &str) {
    ctx.data_mut(|d| d.remove::<egui::Pos2>(egui::Id::new(("ghost_pos", id))));
    ctx.animate_bool_with_time(egui::Id::new(("ghost_on", id)), false, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_renders_and_resets_without_panics() {
        let ctx = egui::Context::default();
        for step in 0..3 {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::PointerMoved(egui::pos2(
                100.0 + step as f32 * 30.0,
                80.0,
            )));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |_| {});
                ghost(
                    ctx,
                    "t",
                    "main.lua",
                    None,
                    Color32::DARK_GRAY,
                    Color32::LIGHT_BLUE,
                );
            });
        }
        end(&ctx, "t");
        let key = egui::Id::new(("ghost_pos", "t"));
        assert!(ctx.data(|d| d.get_temp::<egui::Pos2>(key)).is_none());
    }
}

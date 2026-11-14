use super::*;

// Группы узлов: название, цвет, список
pub const NODE_GROUPS: &[(&str, [u8; 3], &[&str])] = &[
    (
        "2D",
        [72, 132, 196],
        &[
            "Node",
            "Node2D",
            "Marker2D",
            "Sprite2D",
            "AnimatedSprite2D",
            "Camera2D",
            "TileMap",
            "Line2D",
            "Polygon2D",
            "ParallaxBackground",
        ],
    ),
    (
        "Physics",
        [190, 112, 86],
        &[
            "Area2D",
            "StaticBody2D",
            "RigidBody2D",
            "CharacterBody2D",
            "CollisionShape2D",
            "RayCast2D",
        ],
    ),
    (
        "UI",
        [88, 132, 190],
        &[
            "CanvasLayer",
            "Control",
            "Panel",
            "Label",
            "Button",
            "LineEdit",
            "ProgressBar",
            "ColorRect",
            "TextureRect",
        ],
    ),
    (
        "Audio",
        [164, 118, 178],
        &["AudioStreamPlayer", "AudioStreamPlayer2D"],
    ),
    (
        "Effects",
        [190, 170, 82],
        &["CPUParticles2D", "GPUParticles2D", "PointLight2D"],
    ),
    (
        "Other",
        [92, 164, 116],
        &["Timer", "Path2D", "NavigationRegion2D", "AnimationPlayer"],
    ),
];

// Размер и цвет нового узла
pub fn node_defaults(kind: &str) -> (f32, f32, [u8; 3]) {
    let color = node_color(kind);
    let (w, h) = match kind {
        "Label" => (100.0, 38.0),
        "Button" => (110.0, 36.0),
        "LineEdit" => (140.0, 30.0),
        "ProgressBar" => (140.0, 24.0),
        "Panel" => (160.0, 100.0),
        "ColorRect" | "TextureRect" => (120.0, 80.0),
        "Timer" | "Marker2D" | "AnimationPlayer" => (64.0, 40.0),
        "Camera2D" => (72.0, 52.0),
        "Sprite2D" | "AnimatedSprite2D" => (88.0, 88.0),
        "CollisionShape2D" | "RayCast2D" => (76.0, 76.0),
        _ => (112.0, 72.0),
    };
    (w, h, color)
}

pub fn node_color(kind: &str) -> [u8; 3] {
    NODE_GROUPS
        .iter()
        .find(|(_, _, kinds)| kinds.contains(&kind))
        .map(|(_, color, _)| *color)
        .unwrap_or([128, 134, 144])
}

fn add_node_menu(ui: &mut egui::Ui, scene: &mut SceneEditor) {
    for (group, _, kinds) in NODE_GROUPS {
        ui.menu_button(*group, |ui| {
            for kind in *kinds {
                if ui.button(*kind).clicked() {
                    scene.add_node_kind(kind);
                    ui.close_menu();
                }
            }
        });
    }
}

// Дерево сцены (верхняя часть левой колонки, как в Godot)
pub fn scene_tree_ui(ui: &mut egui::Ui, scene: &mut SceneEditor, lang: Lang) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr(lang, K::Hierarchy)).small().strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("＋", |ui| add_node_menu(ui, scene))
                .response
                .on_hover_text(tr(lang, K::AddSceneNode));
        });
    });
    ui.separator();
    let mut remove = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for index in 0..scene.nodes.len() {
                let kind = scene.nodes[index].kind.clone();
                let name = scene.nodes[index].name.clone();
                let color = node_color(&kind);
                let resp = ui
                    .horizontal(|ui| {
                        let (r, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            r,
                            3.0,
                            Color32::from_rgb(color[0], color[1], color[2]),
                        );
                        ui.selectable_label(
                            scene.selected == Some(index),
                            format!("{name}  ·  {kind}"),
                        )
                    })
                    .inner;
                if resp.clicked() {
                    scene.selected = Some(index);
                }
                resp.context_menu(|ui| {
                    if ui.button(tr(lang, K::DeleteSceneNode)).clicked() {
                        remove = Some(index);
                        ui.close_menu();
                    }
                });
            }
            if scene.nodes.is_empty() {
                ui.label(RichText::new(tr(lang, K::SceneEmpty)).small().weak());
            }
        });
    if let Some(index) = remove {
        scene.nodes.remove(index);
        scene.selected = None;
        scene.dirty = true;
    }
}

// Панель сохранения; true — нажали «Сохранить»
pub fn scene_toolbar_ui(ui: &mut egui::Ui, scene: &mut SceneEditor, lang: Lang) -> bool {
    let mut save = false;
    ui.horizontal(|ui| {
        if button(ui, tr(lang, K::Save))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            save = true;
        }
        if scene.dirty {
            ui.label(RichText::new("●").color(Color32::from_rgb(0xE5, 0xA0, 0x3B)));
        }
        if let Some(status) = &scene.status {
            ui.label(RichText::new(status).small().weak());
        }
    });
    ui.add_space(4.0);
    save
}

// Свойства выбранного узла (правая колонка)
pub fn scene_inspector_ui(ui: &mut egui::Ui, scene: &mut SceneEditor, lang: Lang) {
    ui.label(RichText::new(tr(lang, K::Inspector)).small().strong());
    ui.separator();
    let Some(index) = scene.selected.filter(|i| *i < scene.nodes.len()) else {
        ui.label(RichText::new(tr(lang, K::SceneSelectNode)).small().weak());
        return;
    };
    let mut dirty = false;
    let mut delete = false;
    {
        let node = &mut scene.nodes[index];
        let mut kind = node.kind.clone();
        ui.label("Node type");
        egui::ComboBox::from_id_source("scene_node_kind")
            .selected_text(&node.kind)
            .show_ui(ui, |ui| {
                for (group, _, kinds) in NODE_GROUPS {
                    ui.label(RichText::new(*group).small().weak());
                    for option in *kinds {
                        ui.selectable_value(&mut kind, option.to_string(), *option);
                    }
                }
            });
        if kind != node.kind {
            node.kind = kind;
            dirty = true;
        }
        ui.label(tr(lang, K::NodeName));
        dirty |= ui.text_edit_singleline(&mut node.name).changed();
        ui.add_space(8.0);
        ui.label(tr(lang, K::Position));
        ui.horizontal(|ui| {
            ui.label("X");
            dirty |= ui
                .add(egui::DragValue::new(&mut node.x).speed(1.0))
                .changed();
            ui.label("Y");
            dirty |= ui
                .add(egui::DragValue::new(&mut node.y).speed(1.0))
                .changed();
        });
        ui.add_space(8.0);
        ui.label(tr(lang, K::Size));
        ui.horizontal(|ui| {
            ui.label("W");
            dirty |= ui
                .add(
                    egui::DragValue::new(&mut node.width)
                        .speed(1.0)
                        .range(8.0..=2048.0),
                )
                .changed();
            ui.label("H");
            dirty |= ui
                .add(
                    egui::DragValue::new(&mut node.height)
                        .speed(1.0)
                        .range(8.0..=2048.0),
                )
                .changed();
        });
        ui.add_space(8.0);
        ui.label(tr(lang, K::Color));
        dirty |= ui.color_edit_button_srgb(&mut node.color).changed();
        ui.add_space(12.0);
        if ui.button(tr(lang, K::DeleteSceneNode)).clicked() {
            delete = true;
        }
    }
    if delete {
        scene.nodes.remove(index);
        scene.selected = None;
        dirty = true;
    }
    if dirty {
        scene.dirty = true;
    }
}

// Холст сцены
pub fn scene_canvas_ui(
    ui: &mut egui::Ui,
    scene: &mut SceneEditor,
    lang: Lang,
    dark: bool,
    canvas: Color32,
) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
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
            Stroke::new(1.0_f32, grid_color),
        );
        gx += 32.0;
    }
    let mut gy = rect.top() + 24.0;
    while gy < rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), gy), egui::pos2(rect.right(), gy)],
            Stroke::new(1.0_f32, grid_color),
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
        let selected = scene.selected == Some(index);
        painter.rect_filled(node_rect, 3.0, fill.gamma_multiply(0.82));
        painter.rect_stroke(
            node_rect,
            Rounding::same(3.0),
            Stroke::new(
                if selected { 2.0 } else { 1.0_f32 },
                if selected { Color32::WHITE } else { fill },
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
    if ui.input(|i| i.pointer.any_released()) {
        scene.drag_origin = None;
    }
    painter.text(
        rect.left_bottom() + egui::vec2(8.0, -8.0),
        egui::Align2::LEFT_BOTTOM,
        tr(lang, K::CanvasHint),
        FontId::proportional(11.0),
        ui.style().visuals.weak_text_color(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_with_nodes() -> SceneEditor {
        let mut scene = SceneEditor::default();
        scene.add_node_kind("Sprite2D");
        scene.add_node_kind("Timer");
        scene.add_node_kind("Sprite2D");
        scene
    }

    #[test]
    fn kinds_are_unique_and_colored_by_group() {
        let mut seen = std::collections::HashSet::new();
        for (_, color, kinds) in NODE_GROUPS {
            for kind in *kinds {
                assert!(seen.insert(*kind), "{kind} повторяется");
                assert_eq!(node_color(kind), *color);
            }
        }
        assert!(seen.len() >= 30);
        assert_eq!(node_color("Unknown"), [128, 134, 144]);
    }

    #[test]
    fn new_nodes_get_numbered_names() {
        let scene = scene_with_nodes();
        assert_eq!(scene.nodes[0].name, "Sprite2D");
        assert_eq!(scene.nodes[2].name, "Sprite2D2");
        assert!(scene.dirty);
        assert_eq!(scene.selected, Some(2));
    }

    #[test]
    fn scene_panels_render_headless() {
        let ctx = egui::Context::default();
        let mut scene = scene_with_nodes();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::SidePanel::left("t").show(ctx, |ui| {
                    egui::TopBottomPanel::top("dock")
                        .resizable(true)
                        .show_inside(ui, |ui| scene_tree_ui(ui, &mut scene, Lang::Ru));
                });
                egui::SidePanel::right("i").show(ctx, |ui| {
                    scene_inspector_ui(ui, &mut scene, Lang::En);
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = scene_toolbar_ui(ui, &mut scene, Lang::En);
                    scene_canvas_ui(ui, &mut scene, Lang::En, true, Color32::BLACK);
                });
            });
        }
        assert_eq!(scene.nodes.len(), 3);
    }

    #[test]
    fn png_icon_loads_and_bad_file_is_rejected() {
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir().join(format!("ic_icon_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let good = dir.join("ok.png");
        {
            let file = fs::File::create(&good).unwrap();
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 2, 2);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 0])
                .unwrap();
        }
        let tex = load_png_texture(&ctx, &good).expect("png должен загрузиться");
        assert_eq!(tex.size(), [2, 2]);
        let bad = dir.join("bad.png");
        fs::write(&bad, b"not a png").unwrap();
        assert!(load_png_texture(&ctx, &bad).is_none());
        assert!(load_png_texture(&ctx, &dir.join("missing.png")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}

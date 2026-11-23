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
    ("Game", [224, 164, 64], &["FPS", "Body", "Solid"]),
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
        "FPS" => (110.0, 30.0),
        "Body" => (40.0, 40.0),
        "Solid" => (240.0, 24.0),
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

fn add_node_menu(ui: &mut egui::Ui, scene: &mut SceneEditor, parent: Option<usize>) {
    for (group, _, kinds) in NODE_GROUPS {
        ui.menu_button(*group, |ui| {
            for kind in *kinds {
                if ui.button(*kind).clicked() {
                    scene.add_node_under(kind, parent);
                    ui.close_menu();
                }
            }
        });
    }
}

use scene_model::{is_visual, Drag, FRAME_H, FRAME_W};

fn kind_rgb(kind: &str) -> Color32 {
    let c = node_color(kind);
    Color32::from_rgb(c[0], c[1], c[2])
}

// Значок «глаз» (виден/скрыт)
fn eye_toggle(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 14.0), egui::Sense::click());
    let color = ui.visuals().text_color();
    let p = ui.painter();
    let c = rect.center();
    if on {
        p.rect_stroke(
            egui::Rect::from_center_size(c, egui::vec2(12.0, 7.0)),
            Rounding::same(3.5),
            Stroke::new(1.3_f32, color),
        );
        p.circle_filled(c, 2.0, color);
    } else {
        p.line_segment(
            [c - egui::vec2(6.0, 0.0), c + egui::vec2(6.0, 0.0)],
            Stroke::new(1.3_f32, color.gamma_multiply(0.5)),
        );
    }
    resp
}

// Значок «замок»
fn lock_toggle(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 14.0), egui::Sense::click());
    let color = if on {
        ui.visuals().text_color()
    } else {
        ui.visuals().text_color().gamma_multiply(0.35)
    };
    let p = ui.painter();
    let c = rect.center();
    p.rect_filled(
        egui::Rect::from_center_size(c + egui::vec2(0.0, 2.0), egui::vec2(9.0, 6.0)),
        1.5,
        color,
    );
    let top = if on { c.y - 3.0 } else { c.y - 5.0 };
    p.line_segment(
        [egui::pos2(c.x - 2.5, c.y), egui::pos2(c.x - 2.5, top)],
        Stroke::new(1.3_f32, color),
    );
    p.line_segment(
        [egui::pos2(c.x + 2.5, c.y), egui::pos2(c.x + 2.5, top)],
        Stroke::new(1.3_f32, color),
    );
    p.line_segment(
        [egui::pos2(c.x - 2.5, top), egui::pos2(c.x + 2.5, top)],
        Stroke::new(1.3_f32, color),
    );
    resp
}

// Дерево сцены (верхняя часть левой колонки, как в Godot)
pub fn scene_tree_ui(ui: &mut egui::Ui, scene: &mut SceneEditor, lang: Lang) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr(lang, K::Hierarchy)).small().strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("+", |ui| add_node_menu(ui, scene, None))
                .response
                .on_hover_text(tr(lang, K::AddSceneNode));
        });
    });
    ui.separator();
    let mut action: Option<(usize, u8)> = None;
    let mut rows: Vec<(usize, egui::Rect)> = Vec::new();
    let mods = ui.input(|i| i.modifiers);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, depth) in scene.tree_order() {
                let kind = scene.nodes[index].kind.clone();
                let name = scene.nodes[index].name.clone();
                let color = kind_rgb(&kind);
                let row = ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 14.0);
                    let mut visible = scene.nodes[index].visible;
                    if eye_toggle(ui, visible)
                        .on_hover_text(t2(lang, "Показать / скрыть", "Show / hide"))
                        .clicked()
                    {
                        visible = !visible;
                        scene.nodes[index].visible = visible;
                    }
                    let mut locked = scene.nodes[index].locked;
                    if lock_toggle(ui, locked)
                        .on_hover_text(t2(lang, "Заблокировать", "Lock"))
                        .clicked()
                    {
                        locked = !locked;
                        scene.nodes[index].locked = locked;
                    }
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 3.0, color);
                    if scene.renaming == Some(index) {
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut scene.rename_buf).desired_width(130.0),
                        );
                        if !edit.has_focus() && !edit.lost_focus() {
                            edit.request_focus();
                        }
                        if edit.lost_focus() {
                            let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
                            if !cancelled {
                                let buf = scene.rename_buf.clone();
                                scene.rename(index, &buf);
                            }
                            scene.renaming = None;
                        }
                    } else {
                        let resp = ui_color::sel_label(
                            ui,
                            scene.is_selected(index),
                            format!("{name}  ·  {kind}"),
                        );
                        let grab = ui.interact(
                            resp.rect,
                            ui.id().with(("scene_row", index)),
                            egui::Sense::drag(),
                        );
                        if grab.drag_started() {
                            scene.tree_drag = Some(index);
                        }
                        if resp.clicked() {
                            if mods.command || mods.shift {
                                scene.toggle_select(index);
                            } else {
                                scene.set_selection(&[index]);
                            }
                        }
                        if resp.double_clicked() {
                            scene.set_selection(&[index]);
                            action = Some((index, 0));
                        }
                        resp.context_menu(|ui| {
                            if !scene.is_selected(index) {
                                scene.set_selection(&[index]);
                            }
                            ui.menu_button(
                                t2(lang, "Добавить дочерний", "Add child"),
                                |ui| {
                                    add_node_menu(ui, scene, Some(index));
                                },
                            );
                            if scene.nodes[index].parent.is_empty() {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(t2(lang, "Сделать корневым", "Make root")),
                                );
                            } else if ui
                                .button(t2(lang, "Сделать корневым", "Make root"))
                                .clicked()
                            {
                                action = Some((index, 5));
                                ui.close_menu();
                            }
                            ui.separator();
                            for (label, code) in [
                                (t2(lang, "Переименовать (F2)", "Rename (F2)"), 0),
                                (t2(lang, "Дублировать (Ctrl+D)", "Duplicate (Ctrl+D)"), 1),
                                (t2(lang, "Выше", "Move up"), 2),
                                (t2(lang, "Ниже", "Move down"), 3),
                                (tr(lang, K::DeleteSceneNode), 4),
                            ] {
                                if ui.button(label).clicked() {
                                    action = Some((index, code));
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                });
                rows.push((index, row.response.rect));
            }
            if scene.nodes.is_empty() {
                ui.label(RichText::new(tr(lang, K::SceneEmpty)).small());
            }
        });
    // Перетаскивание строки: на середину — стать дочерним, к краю — встать рядом
    if let Some(from) = scene.tree_drag {
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let hit = pointer.and_then(|p| {
            rows.iter()
                .find(|(_, r)| r.y_range().contains(p.y))
                .map(|(i, r)| (*i, *r, p))
        });
        let drop = hit.map(|(t, r, p)| {
            let f = (p.y - r.top()) / r.height().max(1.0);
            (
                t,
                r,
                if f < 0.25 {
                    0u8
                } else if f > 0.75 {
                    2
                } else {
                    1
                },
            )
        });
        if let Some((_, r, zone)) = drop {
            let color = Color32::from_rgb(0x4F, 0x9D, 0xE8);
            match zone {
                1 => {
                    ui.painter()
                        .rect_stroke(r, Rounding::same(3.0), Stroke::new(2.0_f32, color));
                }
                z => {
                    let y = if z == 0 { r.top() } else { r.bottom() };
                    ui.painter().line_segment(
                        [egui::pos2(r.left(), y), egui::pos2(r.right(), y)],
                        Stroke::new(2.0_f32, color),
                    );
                }
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            scene.tree_drag = None;
            if let Some((t, _, zone)) = drop {
                scene.drop_on(from, t, zone);
            }
        }
    }
    if let Some((index, code)) = action {
        if code != 0 || scene.selected.is_none() {
            if !scene.is_selected(index) {
                scene.set_selection(&[index]);
            }
        }
        match code {
            0 => scene.start_rename(index),
            1 => scene.duplicate_selected(),
            2 => scene.move_layer(-1),
            3 => scene.move_layer(1),
            5 => {
                scene.set_parent(index, None);
            }
            _ => scene.delete_selected(),
        }
    }
}

// Панель сцены: сохранить, отмена, масштаб, сетка, модули
pub fn scene_toolbar_ui(ui: &mut egui::Ui, scene: &mut SceneEditor, lang: Lang) -> (bool, bool) {
    let mut save = false;
    let mut run = false;
    ui.horizontal_wrapped(|ui| {
        if button(ui, tr(lang, K::Save))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            save = true;
        }
        if primary_button(ui, tr(lang, K::Run))
            .on_hover_text("F5")
            .clicked()
        {
            run = true;
        }
        for kind in ["FPS", "Body", "Solid"] {
            if button(ui, kind)
                .on_hover_text(t2(lang, "Добавить узел", "Add node"))
                .clicked()
            {
                scene.add_node_kind(kind);
            }
        }
        if ui
            .add_enabled(
                scene.can_undo(),
                egui::Button::new(t2(lang, "Отменить", "Undo")),
            )
            .on_hover_text("Ctrl+Z")
            .clicked()
        {
            scene.undo();
        }
        if ui
            .add_enabled(
                scene.can_redo(),
                egui::Button::new(t2(lang, "Вернуть", "Redo")),
            )
            .on_hover_text("Ctrl+Y")
            .clicked()
        {
            scene.redo();
        }
        if button(ui, t2(lang, "Вписать", "Fit")).clicked() {
            scene.fitted = false;
        }
        ui.label(format!("{:.0}%", scene.zoom * 100.0));
        ui.checkbox(&mut scene.snap, t2(lang, "Сетка", "Snap"));
        ui.add(
            egui::DragValue::new(&mut scene.grid)
                .range(2.0..=256.0)
                .speed(1.0),
        );
        if scene.dirty {
            let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(r.center(), 4.0, Color32::from_rgb(0xE5, 0xA0, 0x3B));
        }
        if let Some(status) = &scene.status {
            ui.label(RichText::new(status).small());
        }
    });
    ui.add_space(4.0);
    (save, run)
}

// Свойства выбранного узла (правая колонка)
pub fn scene_inspector_ui(ui: &mut egui::Ui, scene: &mut SceneEditor, lang: Lang) {
    ui.label(RichText::new(tr(lang, K::Inspector)).small().strong());
    ui.separator();
    let Some(index) = scene.selected.filter(|i| *i < scene.nodes.len()) else {
        ui.label(RichText::new(tr(lang, K::SceneSelectNode)).small());
        return;
    };
    let selected_count = scene.all_selected().len();
    if selected_count > 1 {
        ui.label(
            RichText::new(format!(
                "{} {selected_count}",
                t2(lang, "Выбрано узлов:", "Selected nodes:")
            ))
            .small(),
        );
    }
    let mut rename_to: Option<String> = None;
    let mut new_parent: Option<Option<usize>> = None;
    let names: Vec<String> = scene.nodes.iter().map(|n| n.name.clone()).collect();
    let mut blocked = scene.descendants(index);
    blocked.push(index);
    let mut layer = 0isize;
    let mut delete = false;
    {
        let node = &mut scene.nodes[index];
        let mut kind = node.kind.clone();
        ui.label(t2(lang, "Тип узла", "Node type"));
        egui::ComboBox::from_id_source("scene_node_kind")
            .selected_text(&node.kind)
            .show_ui(ui, |ui| {
                for (group, _, kinds) in NODE_GROUPS {
                    ui.label(RichText::new(*group).small());
                    for option in *kinds {
                        if ui_color::sel_label(ui, kind == **option, *option).clicked() {
                            kind = option.to_string();
                        }
                    }
                }
            });
        node.kind = kind;
        let visual = is_visual(&node.kind);
        ui.label(
            RichText::new(if visual {
                t2(
                    lang,
                    "Рисуется в игре прямоугольником",
                    "Drawn in game as a rectangle",
                )
            } else {
                t2(
                    lang,
                    "В игре не рисуется (служебный узел)",
                    "Not drawn in game (helper node)",
                )
            })
            .small(),
        );
        ui.add_space(6.0);
        ui.label(t2(lang, "Родитель", "Parent"));
        egui::ComboBox::from_id_source("scene_node_parent")
            .selected_text(if node.parent.is_empty() {
                "—"
            } else {
                node.parent.as_str()
            })
            .width(170.0)
            .show_ui(ui, |ui| {
                if ui_color::sel_label(ui, node.parent.is_empty(), "—").clicked() {
                    new_parent = Some(None);
                }
                for (i, other) in names.iter().enumerate() {
                    if i != index && !blocked.contains(&i) {
                        if ui_color::sel_label(ui, node.parent == *other, other.as_str()).clicked()
                        {
                            new_parent = Some(Some(i));
                        }
                    }
                }
            });
        ui.add_space(6.0);
        ui.label(tr(lang, K::NodeName));
        let mut name = node.name.clone();
        let resp = ui.text_edit_singleline(&mut name);
        if resp.lost_focus() && name != node.name {
            rename_to = Some(name);
        }
        ui.add_space(6.0);
        ui.label(tr(lang, K::Position));
        ui.horizontal(|ui| {
            ui.label("X");
            ui.add(egui::DragValue::new(&mut node.x).speed(1.0));
            ui.label("Y");
            ui.add(egui::DragValue::new(&mut node.y).speed(1.0));
        });
        ui.label(tr(lang, K::Size));
        ui.horizontal(|ui| {
            ui.label("W");
            ui.add(
                egui::DragValue::new(&mut node.width)
                    .speed(1.0)
                    .range(1.0..=8192.0),
            );
            ui.label("H");
            ui.add(
                egui::DragValue::new(&mut node.height)
                    .speed(1.0)
                    .range(1.0..=8192.0),
            );
        });
        ui.label(t2(lang, "Поворот (градусы)", "Rotation (degrees)"));
        ui.add(
            egui::DragValue::new(&mut node.rotation)
                .speed(0.5)
                .range(-360.0..=360.0),
        );
        ui.add_space(6.0);
        ui.label(t2(lang, "Картинка", "Texture"));
        egui::ComboBox::from_id_source("scene_node_texture")
            .selected_text(if node.texture.is_empty() {
                "—"
            } else {
                node.texture.as_str()
            })
            .width(170.0)
            .show_ui(ui, |ui| {
                if ui_color::sel_label(ui, node.texture.is_empty(), "—").clicked() {
                    node.texture.clear();
                }
                for png in &scene.pngs {
                    if ui_color::sel_label(ui, &node.texture == png, png).clicked() {
                        node.texture = png.clone();
                    }
                }
            });
        ui.label(t2(lang, "Текст", "Text"));
        ui.text_edit_singleline(&mut node.text);
        ui.horizontal(|ui| {
            ui.label(t2(lang, "Размер шрифта", "Font size"));
            ui.add(
                egui::DragValue::new(&mut node.font_size)
                    .speed(0.5)
                    .range(4.0..=400.0),
            );
        });
        if node.kind == "Body" {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(t2(lang, "Гравитация", "Gravity"));
                ui.add(egui::DragValue::new(&mut node.gravity).speed(5.0));
            });
            ui.horizontal(|ui| {
                ui.label(t2(lang, "Упругость", "Bounce"));
                ui.add(
                    egui::DragValue::new(&mut node.bounce)
                        .speed(0.01)
                        .range(0.0..=1.0),
                );
            });
            ui.label(
                RichText::new(t2(
                    lang,
                    "Тело падает и отскакивает от узлов «Solid». Ставь тело и твёрдое в корень сцены.",
                    "The body falls and bounces off Solid nodes. Keep bodies and solids at the scene root.",
                ))
                .small(),
            );
        }
        ui.add_space(6.0);
        ui.label(tr(lang, K::Color));
        ui.color_edit_button_srgb(&mut node.color);
        ui.label(t2(lang, "Прозрачность", "Opacity"));
        let mut alpha = node.alpha as f32;
        if ui
            .add(egui::Slider::new(&mut alpha, 0.0..=255.0).integer())
            .changed()
        {
            node.alpha = alpha as u8;
        }
        ui.checkbox(
            &mut node.visible,
            t2(lang, "Виден в игре", "Visible in game"),
        );
        ui.checkbox(
            &mut node.locked,
            t2(lang, "Заблокирован в редакторе", "Locked in editor"),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(t2(lang, "Выше", "Up")).clicked() {
                layer = -1;
            }
            if ui.button(t2(lang, "Ниже", "Down")).clicked() {
                layer = 1;
            }
        });
        ui.add_space(6.0);
        if ui.button(tr(lang, K::DeleteSceneNode)).clicked() {
            delete = true;
        }
    }
    if let Some(name) = rename_to {
        scene.rename(index, &name);
    }
    if let Some(parent) = new_parent {
        scene.set_parent(index, parent);
    }
    if layer != 0 {
        scene.move_layer(layer);
    }
    if delete {
        scene.delete_selected();
    }
}

fn w2s(center: egui::Pos2, pan: egui::Vec2, zoom: f32, x: f32, y: f32) -> egui::Pos2 {
    center + pan + egui::vec2(x * zoom, -y * zoom)
}

fn s2w(center: egui::Pos2, pan: egui::Vec2, zoom: f32, p: egui::Pos2) -> (f32, f32) {
    let v = (p - center - pan) / zoom;
    (v.x, -v.y)
}

// Углы узла в мировых координатах (Y вверх, поворот против часовой)
fn corners(n: &SceneNode) -> [(f32, f32); 4] {
    let (s, c) = n.rotation.to_radians().sin_cos();
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(sx, sy)| {
        let (lx, ly) = (sx * n.width / 2.0, sy * n.height / 2.0);
        (n.x + lx * c - ly * s, n.y + lx * s + ly * c)
    })
}

fn to_local(n: &SceneNode, wx: f32, wy: f32) -> (f32, f32) {
    let (s, c) = n.rotation.to_radians().sin_cos();
    let (dx, dy) = (wx - n.x, wy - n.y);
    (dx * c + dy * s, -dx * s + dy * c)
}

fn node_at(scene: &SceneEditor, wx: f32, wy: f32) -> Option<usize> {
    (0..scene.nodes.len()).rev().find(|&i| {
        let n = &scene.nodes[i];
        if n.locked {
            return false;
        }
        let (lx, ly) = to_local(n, wx, wy);
        lx.abs() <= n.width / 2.0 && ly.abs() <= n.height / 2.0
    })
}

const HANDLES: [(f32, f32); 8] = [
    (-1.0, -1.0),
    (0.0, -1.0),
    (1.0, -1.0),
    (1.0, 0.0),
    (1.0, 1.0),
    (0.0, 1.0),
    (-1.0, 1.0),
    (-1.0, 0.0),
];

fn handle_pos(n: &SceneNode, h: usize) -> (f32, f32) {
    let (s, c) = n.rotation.to_radians().sin_cos();
    let (lx, ly) = (HANDLES[h].0 * n.width / 2.0, HANDLES[h].1 * n.height / 2.0);
    (n.x + lx * c - ly * s, n.y + lx * s + ly * c)
}

fn snap_value(v: f32, grid: f32, on: bool) -> f32 {
    if on && grid > 0.0 {
        (v / grid).round() * grid
    } else {
        v
    }
}

// Холст сцены: мир как в игре (центр кадра — начало координат, Y вверх)
pub fn scene_canvas_ui(
    ui: &mut egui::Ui,
    scene: &mut SceneEditor,
    lang: Lang,
    dark: bool,
    canvas: Color32,
) {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    scene.canvas_rect = Some(rect);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, canvas);
    let center = rect.center();
    if !scene.fitted {
        scene.zoom = (rect.width() / FRAME_W).min(rect.height() / FRAME_H) * 0.92;
        scene.pan = egui::Vec2::ZERO;
        scene.fitted = true;
    }
    // F: показать выделенное
    if scene.focus_request {
        scene.focus_request = false;
        if let Some((x0, y0, x1, y1)) = scene.selection_bounds() {
            let (w, h) = ((x1 - x0).max(32.0), (y1 - y0).max(32.0));
            scene.zoom = ((rect.width() / w).min(rect.height() / h) * 0.6).clamp(0.05, 8.0);
            scene.pan = -egui::vec2((x0 + x1) / 2.0 * scene.zoom, -(y0 + y1) / 2.0 * scene.zoom);
        } else {
            scene.fitted = false;
        }
    }

    // Масштаб колесом мыши вокруг курсора
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
                let (wx, wy) = s2w(center, scene.pan, scene.zoom, ptr);
                scene.zoom = (scene.zoom * (1.0 + scroll * 0.0015)).clamp(0.05, 8.0);
                scene.pan = ptr - center - egui::vec2(wx * scene.zoom, -wy * scene.zoom);
            }
        }
    }
    // Правая кнопка без перетаскивания: меню (с перетаскиванием она двигает холст)
    if resp.secondary_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let (wx, wy) = s2w(center, scene.pan, scene.zoom, p);
            match node_at(scene, wx, wy) {
                Some(i) if !scene.is_selected(i) => scene.set_selection(&[i]),
                Some(_) => {}
                None => scene.clear_selection(),
            }
        }
    }
    resp.context_menu(|ui| {
        let sel = scene.selected;
        ui.menu_button(t2(lang, "Добавить узел", "Add node"), |ui| {
            add_node_menu(ui, scene, None);
        });
        if sel.is_some() {
            ui.menu_button(
                t2(lang, "Добавить дочерний", "Add child"),
                |ui| {
                    add_node_menu(ui, scene, sel);
                },
            );
        }
        ui.separator();
        let mut done = true;
        if ui
            .add_enabled(
                sel.is_some(),
                egui::Button::new(t2(lang, "Копировать", "Copy")),
            )
            .clicked()
        {
            scene.copy_selected();
        } else if ui
            .add_enabled(
                !scene.clipboard.is_empty(),
                egui::Button::new(t2(lang, "Вставить", "Paste")),
            )
            .clicked()
        {
            scene.paste();
        } else if ui
            .add_enabled(
                sel.is_some(),
                egui::Button::new(t2(lang, "Дублировать", "Duplicate")),
            )
            .clicked()
        {
            scene.duplicate_selected();
        } else if ui
            .add_enabled(
                sel.is_some(),
                egui::Button::new(tr(lang, K::DeleteSceneNode)),
            )
            .clicked()
        {
            scene.delete_selected();
        } else if ui.button(t2(lang, "Выделить всё", "Select all")).clicked() {
            scene.select_all();
        } else {
            done = false;
        }
        if done {
            ui.close_menu();
        }
    });
    let (pan, zoom) = (scene.pan, scene.zoom);

    // Сетка
    let line = if dark {
        Color32::from_gray(48)
    } else {
        Color32::from_gray(228)
    };
    let strong = if dark {
        Color32::from_gray(70)
    } else {
        Color32::from_gray(205)
    };
    let mut step = scene.grid.max(2.0);
    while step * zoom < 10.0 {
        step *= 2.0;
    }
    let (wl, wt) = s2w(center, pan, zoom, rect.left_top());
    let (wr, wb) = s2w(center, pan, zoom, rect.right_bottom());
    let mut gx = (wl / step).floor() * step;
    while gx <= wr {
        let x = w2s(center, pan, zoom, gx, 0.0).x;
        let major = ((gx / step).round() as i64) % 5 == 0;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(1.0_f32, if major { strong } else { line }),
        );
        gx += step;
    }
    let mut gy = (wb / step).floor() * step;
    while gy <= wt {
        let y = w2s(center, pan, zoom, 0.0, gy).y;
        let major = ((gy / step).round() as i64) % 5 == 0;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0_f32, if major { strong } else { line }),
        );
        gy += step;
    }
    let origin = w2s(center, pan, zoom, 0.0, 0.0);
    painter.line_segment(
        [
            egui::pos2(rect.left(), origin.y),
            egui::pos2(rect.right(), origin.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(0xC0, 0x50, 0x50)),
    );
    painter.line_segment(
        [
            egui::pos2(origin.x, rect.top()),
            egui::pos2(origin.x, rect.bottom()),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(0x50, 0xB0, 0x60)),
    );
    // Кадр камеры игры
    let frame = egui::Rect::from_two_pos(
        w2s(center, pan, zoom, -FRAME_W / 2.0, FRAME_H / 2.0),
        w2s(center, pan, zoom, FRAME_W / 2.0, -FRAME_H / 2.0),
    );
    painter.rect_stroke(
        frame,
        Rounding::ZERO,
        Stroke::new(1.5_f32, Color32::from_rgb(0x4F, 0x9D, 0xE8)),
    );
    painter.text(
        frame.left_top() + egui::vec2(4.0, -4.0),
        egui::Align2::LEFT_BOTTOM,
        "1280×720",
        FontId::proportional(11.0),
        Color32::from_rgb(0x4F, 0x9D, 0xE8),
    );

    // Текстуры узлов: загружаем один раз
    let wanted: Vec<String> = scene
        .nodes
        .iter()
        .filter(|n| !n.texture.is_empty() && !scene.textures.contains_key(&n.texture))
        .map(|n| n.texture.clone())
        .collect();
    for name in wanted {
        let tex = scene
            .project_dir
            .as_ref()
            .and_then(|dir| load_png_texture(ui.ctx(), &dir.join(&name)));
        scene.textures.insert(name, tex);
    }

    // Узлы
    for (i, n) in scene.nodes.iter().enumerate() {
        let pts: Vec<egui::Pos2> = corners(n)
            .iter()
            .map(|(x, y)| w2s(center, pan, zoom, *x, *y))
            .collect();
        let base = Color32::from_rgb(n.color[0], n.color[1], n.color[2]);
        let visual = is_visual(&n.kind) && n.visible;
        let texture = scene.textures.get(&n.texture).and_then(|t| t.as_ref());
        let fill = if !visual {
            base.gamma_multiply(0.15)
        } else if texture.is_some() || n.kind == "Label" || n.kind == "FPS" {
            Color32::TRANSPARENT
        } else {
            Color32::from_rgba_unmultiplied(n.color[0], n.color[1], n.color[2], n.alpha)
        };
        let selected = scene.is_selected(i);
        painter.add(egui::Shape::convex_polygon(
            pts.clone(),
            fill,
            Stroke::new(
                if selected { 2.0 } else { 1.0_f32 },
                if selected { Color32::WHITE } else { base },
            ),
        ));
        if let (Some(tex), true) = (texture, visual) {
            let mut mesh = egui::Mesh::with_texture(tex.id());
            let tint = Color32::from_white_alpha(n.alpha);
            for (p, uv) in pts
                .iter()
                .zip([(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)])
            {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: *p,
                    uv: egui::pos2(uv.0, uv.1),
                    color: tint,
                });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));
        }
        let fps_text = "FPS: 60".to_string();
        let label = if n.kind == "FPS" {
            &fps_text
        } else if n.text.is_empty() {
            &n.name
        } else {
            &n.text
        };
        let (label_size, label_color) = if n.kind == "FPS" {
            (
                (n.font_size * zoom).clamp(6.0, 400.0),
                Color32::from_rgb(n.color[0], n.color[1], n.color[2]),
            )
        } else if n.text.is_empty() {
            (
                13.0,
                if n.locked {
                    Color32::from_gray(190)
                } else {
                    Color32::WHITE
                },
            )
        } else if n.kind == "Label" {
            (
                (n.font_size * zoom).clamp(6.0, 400.0),
                Color32::from_rgb(n.color[0], n.color[1], n.color[2]),
            )
        } else {
            ((n.font_size * zoom).clamp(6.0, 400.0), Color32::WHITE)
        };
        painter.text(
            w2s(center, pan, zoom, n.x, n.y),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(label_size),
            label_color,
        );
    }
    let single = scene.all_selected().len() == 1;
    if let Some(i) = scene
        .selected
        .filter(|i| single && *i < scene.nodes.len() && !scene.nodes[*i].locked)
    {
        for h in 0..8 {
            let (hx, hy) = handle_pos(&scene.nodes[i], h);
            let p = w2s(center, pan, zoom, hx, hy);
            let r = egui::Rect::from_center_size(p, egui::vec2(8.0, 8.0));
            painter.rect_filled(r, 1.0, Color32::WHITE);
            painter.rect_stroke(
                r,
                Rounding::ZERO,
                Stroke::new(1.0_f32, Color32::from_rgb(0x1E, 0x6F, 0xD9)),
            );
        }
    }

    // Мышь
    let ptr = ui.input(|i| i.pointer.hover_pos());
    let space = ui.input(|i| i.key_down(egui::Key::Space));
    let mods = ui.input(|i| i.modifiers);
    let handle_under = |scene: &SceneEditor, p: egui::Pos2| -> Option<usize> {
        if scene.all_selected().len() != 1 {
            return None;
        }
        let i = scene
            .selected
            .filter(|i| *i < scene.nodes.len() && !scene.nodes[*i].locked)?;
        (0..8).find(|&h| {
            let (hx, hy) = handle_pos(&scene.nodes[i], h);
            w2s(center, pan, zoom, hx, hy).distance(p) <= 7.0
        })
    };
    if let Some(p) = ptr.filter(|p| rect.contains(*p)) {
        let (wx, wy) = s2w(center, pan, zoom, p);
        let icon = if handle_under(scene, p).is_some() {
            egui::CursorIcon::ResizeNwSe
        } else if node_at(scene, wx, wy).is_some() {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::Default
        };
        if scene.drag.is_none() {
            ui.ctx().set_cursor_icon(icon);
        }
    }
    if resp.drag_started() {
        let origin_ptr = ui.input(|i| i.pointer.press_origin()).or(ptr);
        if let Some(p) = origin_ptr {
            let button_pan = ui.input(|i| {
                i.pointer.button_down(egui::PointerButton::Middle)
                    || i.pointer.button_down(egui::PointerButton::Secondary)
            });
            let (wx, wy) = s2w(center, pan, zoom, p);
            if button_pan || space {
                scene.drag = Some(Drag::Pan);
            } else if let Some(h) = handle_under(scene, p) {
                let i = scene.selected.unwrap_or(0);
                let n = &scene.nodes[i];
                scene.drag = Some(Drag::Resize {
                    index: i,
                    handle: h as u8,
                    orig: (n.x, n.y, n.width, n.height),
                });
            } else if let Some(i) = node_at(scene, wx, wy) {
                if mods.command || mods.shift {
                    scene.toggle_select(i);
                } else if !scene.is_selected(i) {
                    scene.set_selection(&[i]);
                }
                let origs = scene
                    .all_selected()
                    .into_iter()
                    .filter(|j| !scene.nodes[*j].locked)
                    .map(|j| (j, scene.nodes[j].x, scene.nodes[j].y))
                    .collect();
                scene.drag = Some(Drag::Move {
                    start: (wx, wy),
                    origs,
                });
            } else {
                scene.drag = Some(Drag::Marquee {
                    start: (wx, wy),
                    additive: mods.shift || mods.command,
                });
            }
        }
    }
    let alt = mods.alt;
    let snap = scene.snap && !alt;
    let mut marquee_rect: Option<egui::Rect> = None;
    if resp.dragged() {
        if let (Some(drag), Some(p)) = (scene.drag.clone(), ptr) {
            let (wx, wy) = s2w(center, pan, zoom, p);
            match drag {
                Drag::Pan => scene.pan += resp.drag_delta(),
                Drag::Move { start, origs } => {
                    let primary = scene
                        .selected
                        .and_then(|s| origs.iter().find(|(j, _, _)| *j == s))
                        .or(origs.first())
                        .copied();
                    if let Some((_, ox, oy)) = primary {
                        let dx = snap_value(ox + wx - start.0, scene.grid, snap) - ox;
                        let dy = snap_value(oy + wy - start.1, scene.grid, snap) - oy;
                        for (j, x, y) in origs {
                            if let Some(n) = scene.nodes.get_mut(j) {
                                n.x = x + dx;
                                n.y = y + dy;
                            }
                        }
                    }
                }
                Drag::Marquee { start, .. } => {
                    marquee_rect = Some(egui::Rect::from_two_pos(
                        w2s(center, pan, zoom, start.0, start.1),
                        p,
                    ));
                }
                Drag::Resize {
                    index,
                    handle,
                    orig,
                } => {
                    if let Some(n) = scene.nodes.get(index).cloned() {
                        let base = SceneNode {
                            x: orig.0,
                            y: orig.1,
                            width: orig.2,
                            height: orig.3,
                            ..n
                        };
                        let (lx, ly) = to_local(&base, wx, wy);
                        let (hx, hy) = HANDLES[handle as usize];
                        let min = if snap { scene.grid } else { 4.0 };
                        let mut nw = orig.2;
                        let mut nh = orig.3;
                        let (mut ox, mut oy) = (0.0, 0.0);
                        if hx != 0.0 {
                            nw = snap_value(hx * lx + orig.2 / 2.0, scene.grid, snap).max(min);
                            ox = hx * (nw - orig.2) / 2.0;
                        }
                        if hy != 0.0 {
                            nh = snap_value(hy * ly + orig.3 / 2.0, scene.grid, snap).max(min);
                            oy = hy * (nh - orig.3) / 2.0;
                        }
                        let (s, c) = base.rotation.to_radians().sin_cos();
                        if let Some(m) = scene.nodes.get_mut(index) {
                            m.width = nw;
                            m.height = nh;
                            m.x = orig.0 + ox * c - oy * s;
                            m.y = orig.1 + ox * s + oy * c;
                        }
                    }
                }
            }
        }
    }
    if let Some(r) = marquee_rect {
        painter.rect_filled(
            r,
            0.0,
            Color32::from_rgba_unmultiplied(0x4F, 0x9D, 0xE8, 40),
        );
        painter.rect_stroke(
            r,
            Rounding::ZERO,
            Stroke::new(1.0_f32, Color32::from_rgb(0x4F, 0x9D, 0xE8)),
        );
    }
    if resp.drag_stopped() {
        if let (Some(Drag::Marquee { start, additive }), Some(p)) = (scene.drag.clone(), ptr) {
            let (wx, wy) = s2w(center, pan, zoom, p);
            let (x0, x1) = (start.0.min(wx), start.0.max(wx));
            let (y0, y1) = (start.1.min(wy), start.1.max(wy));
            let mut hit: Vec<usize> = (0..scene.nodes.len())
                .filter(|&i| {
                    let n = &scene.nodes[i];
                    !n.locked
                        && corners(n)
                            .iter()
                            .any(|(x, y)| *x >= x0 && *x <= x1 && *y >= y0 && *y <= y1)
                })
                .collect();
            if additive {
                hit.extend(scene.all_selected());
                hit.sort_unstable();
                hit.dedup();
            }
            scene.set_selection(&hit);
        }
        scene.drag = None;
    }
    if resp.clicked() {
        if let Some(p) = ptr {
            let (wx, wy) = s2w(center, pan, zoom, p);
            match node_at(scene, wx, wy) {
                Some(i) if mods.command || mods.shift => scene.toggle_select(i),
                Some(i) => scene.set_selection(&[i]),
                None if mods.shift || mods.command => {}
                None => scene.clear_selection(),
            }
        }
    }
    if resp.double_clicked() {
        if let Some(i) = scene.selected {
            scene.start_rename(i);
        }
    }
    painter.text(
        rect.left_bottom() + egui::vec2(8.0, -8.0),
        egui::Align2::LEFT_BOTTOM,
        t2(
            lang,
            "Колесо — масштаб, средняя кнопка или Пробел+мышь — перемещение, Shift — несколько узлов, Alt — без привязки",
            "Wheel zoom, middle button or Space+drag to pan, Shift multi-select, Alt disables snapping",
        ),
        FontId::proportional(11.0),
        ui.style().visuals.text_color(),
    );
}

// Горячие клавиши сцены (когда не идёт ввод текста)
pub fn scene_keys(ctx: &egui::Context, scene: &mut SceneEditor) {
    if ctx.wants_keyboard_input() {
        return;
    }
    let none = egui::Modifiers::NONE;
    let ctrl = egui::Modifiers::COMMAND;
    let shift = egui::Modifiers::SHIFT;
    let consume = |m: egui::Modifiers, k: egui::Key| ctx.input_mut(|i| i.consume_key(m, k));
    if consume(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    ) || consume(ctrl, egui::Key::Y)
    {
        scene.redo();
    } else if consume(ctrl, egui::Key::Z) {
        scene.undo();
    }
    if consume(ctrl, egui::Key::D) {
        scene.duplicate_selected();
    }
    if consume(ctrl, egui::Key::A) {
        scene.select_all();
    }
    if consume(ctrl, egui::Key::C) {
        scene.copy_selected();
    }
    if consume(ctrl, egui::Key::V) {
        scene.paste();
    }
    if consume(none, egui::Key::Escape) {
        scene.clear_selection();
    }
    if consume(none, egui::Key::F) {
        scene.focus_request = true;
    }
    if consume(none, egui::Key::Home) {
        scene.fitted = false;
    }
    if consume(none, egui::Key::Delete) || consume(none, egui::Key::Backspace) {
        scene.delete_selected();
    }
    if consume(none, egui::Key::F2) {
        if let Some(i) = scene.selected {
            scene.start_rename(i);
        }
    }
    let targets: Vec<usize> = scene
        .all_selected()
        .into_iter()
        .filter(|i| !scene.nodes[*i].locked)
        .collect();
    if targets.is_empty() {
        return;
    }
    for (key, dx, dy) in [
        (egui::Key::ArrowLeft, -1.0, 0.0),
        (egui::Key::ArrowRight, 1.0, 0.0),
        (egui::Key::ArrowUp, 0.0, 1.0),
        (egui::Key::ArrowDown, 0.0, -1.0),
    ] {
        // Сначала Shift (шаг 10): без него клавиша ушла бы в шаг 1
        for (m, k) in [(shift, 10.0), (none, 1.0)] {
            if consume(m, key) {
                for i in &targets {
                    scene.nodes[*i].x += dx * k;
                    scene.nodes[*i].y += dy * k;
                }
                break;
            }
        }
    }
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

    fn press(
        ctx: &egui::Context,
        scene: &mut SceneEditor,
        key: egui::Key,
        modifiers: egui::Modifiers,
    ) {
        let mut input = egui::RawInput::default();
        input.modifiers = modifiers;
        input.events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        });
        let _ = ctx.run(input, |ctx| scene_keys(ctx, scene));
    }

    #[test]
    fn keyboard_shortcuts_edit_the_scene() {
        let ctx = egui::Context::default();
        let mut scene = SceneEditor::default();
        scene.add_node_kind("ColorRect");
        let (x0, y0) = (scene.nodes[0].x, scene.nodes[0].y);

        press(
            &ctx,
            &mut scene,
            egui::Key::ArrowRight,
            egui::Modifiers::NONE,
        );
        assert_eq!(scene.nodes[0].x, x0 + 1.0);
        press(&ctx, &mut scene, egui::Key::ArrowUp, egui::Modifiers::SHIFT);
        assert_eq!(scene.nodes[0].y, y0 + 10.0);

        press(&ctx, &mut scene, egui::Key::D, egui::Modifiers::COMMAND);
        assert_eq!(scene.nodes.len(), 2);
        assert_eq!(scene.nodes[1].name, "ColorRect2");

        press(&ctx, &mut scene, egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(scene.nodes.len(), 1);
        press(&ctx, &mut scene, egui::Key::Y, egui::Modifiers::COMMAND);
        assert_eq!(scene.nodes.len(), 2);

        scene.selected = Some(1);
        press(&ctx, &mut scene, egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(scene.nodes.len(), 1);

        scene.selected = Some(0);
        press(&ctx, &mut scene, egui::Key::F2, egui::Modifiers::NONE);
        assert_eq!(scene.renaming, Some(0));
    }

    #[test]
    fn resize_handles_and_hit_testing_follow_rotation() {
        let mut n = SceneNode {
            kind: "ColorRect".into(),
            name: "a".into(),
            x: 100.0,
            y: 50.0,
            width: 80.0,
            height: 40.0,
            rotation: 90.0,
            color: [1, 2, 3],
            alpha: 255,
            visible: true,
            locked: false,
            texture: String::new(),
            text: String::new(),
            font_size: 24.0,
            parent: String::new(),
            gravity: 900.0,
            bounce: 0.0,
        };
        let (hx, hy) = handle_pos(&n, 3);
        assert!(
            (hx - 100.0).abs() < 0.01 && (hy - 90.0).abs() < 0.01,
            "{hx} {hy}"
        );
        let mut scene = SceneEditor::default();
        scene.nodes.push(n.clone());
        assert_eq!(node_at(&scene, 100.0, 85.0), Some(0));
        assert_eq!(node_at(&scene, 135.0, 50.0), None);
        n.rotation = 0.0;
        scene.nodes[0] = n;
        assert_eq!(node_at(&scene, 135.0, 50.0), Some(0));
    }
}

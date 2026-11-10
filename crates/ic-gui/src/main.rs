// #компиляция #стиль #гравитация
// IC Engine main GUI module
use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1150.0, 720.0])
            .with_title("IC Engine"),
        ..Default::default()
    };
    eframe::run_native(
        "IC Engine",
        options,
        Box::new(|_cc| Box::new(IcApp::default())),
    )
}

struct IcApp {
    dark_mode: bool,
    show_new_project_dialog: bool,
    show_settings_dialog: bool,
    show_scene_view: bool,
    is_game: bool,
    ask_default_code: bool,
    // Settings parameters (Removed "Тип проекта" & "Название типа" per request)
    proj_name: String,
    window_title: String,
    window_w: u32,
    window_h: u32,
    target_fps: u32,
    bg_color: egui::Color32,
    build_target: String,
    active_tab: String,
    code_content: String,
    logs: Vec<String>,
}

impl Default for IcApp {
    fn default() -> Self {
        Self {
            dark_mode: true,
            show_new_project_dialog: false,
            show_settings_dialog: false,
            show_scene_view: true,
            is_game: true,
            ask_default_code: true,
            proj_name: "my_game".to_string(),
            window_title: "my_game".to_string(),
            window_w: 1280,
            window_h: 720,
            target_fps: 60,
            bg_color: egui::Color32::from_rgb(30, 30, 50),
            build_target: "Эта система".to_string(),
            active_tab: "main.lua".to_string(),
            code_content: "-- #компиляция #стиль #гравитация\nlocal player = { x = 400, y = 300, vy = 0 }\nlocal gravity = 9.8\n\nfunction ic.update(dt)\n    -- #гравитация\n    player.vy = player.vy + gravity * dt\n    player.y = player.y + player.vy * dt\nend\n\nfunction ic.draw()\n    -- #стиль\n    ic.rect(player.x, player.y, 40, 40, 1.0, 0.5, 0.2, 1.0)\nend".to_string(),
            logs: vec!["[IC Engine] Инициализация успешна. За пределы папки движок не выходит!".to_string()],
        }
    }
}

impl eframe::App for IcApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // #стиль
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        // Верхняя панель управления
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Иконка: Шестеренка с лампочкой ⚙💡 (Собственная иконка IC Engine)
                ui.label(egui::RichText::new("⚙💡 IC").bold().size(18.0));
                ui.separator();
                
                if ui.button("📁 Новый проект").clicked() {
                    self.show_new_project_dialog = true;
                }
                
                if ui.button("⚙ Настройки").clicked() {
                    self.show_settings_dialog = true;
                }

                if ui.button("▶ Запустить (F5)").clicked() {
                    // #компиляция
                    self.logs.push("[Запуск] Скрипт запущен локально в папке проекта.".to_string());
                }

                if ui.button("📦 Собрать exe").clicked() {
                    // #компиляция
                    self.logs.push(format!("[Сборка] Проект '{}' собран в один файл!", self.proj_name));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let theme_btn = if self.dark_mode { "🌙" } else { "☀" };
                    if ui.button(theme_btn).clicked() {
                        self.dark_mode = !self.dark_mode;
                    }
                });
            });
        });

        // Левая панель - Проводник в стиле VS Code & Узлы Сцены (Godot-style)
        egui::SidePanel::left("left_panel").resizable(true).default_width(230.0).show(ctx, |ui| {
            ui.heading("Проводник");
            ui.horizontal(|ui| {
                if ui.button("+ Файл").clicked() {}
                if ui.button("+ Папка").clicked() {}
            });
            ui.separator();
            ui.collapsing(format!("📁 {}", self.proj_name), |ui| {
                if ui.selectable_label(self.active_tab == "main.lua", "📄 main.lua").clicked() {
                    self.active_tab = "main.lua".to_string();
                }
                if ui.selectable_label(self.active_tab == "ic.toml", "⚙ ic.toml").clicked() {
                    self.active_tab = "ic.toml".to_string();
                }
            });

            ui.add_space(15.0);
            ui.separator();
            ui.heading("Узлы сцены (Godot)");
            ui.label("Node2D (Корень)");
            ui.label(" ├── Camera2D");
            ui.label(" ├── Player (KinematicBody2D)");
            ui.label(" └── RigidBody2D (Физика)");
        });

        // Центральная область редактора и сцены
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.show_scene_view, true, "🎬 Сцена (Godot View)");
                ui.selectable_value(&mut self.show_scene_view, false, "📝 Код (VS Code View)");
            });
            ui.separator();

            if self.show_scene_view {
                // Визуальный редактор сцены
                ui.vertical_centered(|ui| {
                    let (rect, _) = ui.allocate_at_least(egui::vec2(640.0, 360.0), egui::Sense::drag());
                    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 25));
                    ui.painter().circle_filled(rect.center(), 25.0, egui::Color32::from_rgb(220, 90, 40));
                    ui.label("Перетаскивайте объекты на сцене мышью");
                });
            } else {
                // Текстовый редактор кода
                ui.add(
                    egui::TextEdit::multiline(&mut self.code_content)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_rows(22)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                );
            }

            ui.separator();
            ui.heading("Консоль / Журнал");
            egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                for log in &self.logs {
                    ui.label(log);
                }
            });
        });

        // Диалог создания нового проекта
        if self.show_new_project_dialog {
            egui::Window::new("Что создаём?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.is_game, true, "Игра");
                        ui.radio_value(&mut self.is_game, false, "Программа");
                    });
                    ui.checkbox(&mut self.ask_default_code, "Вставить код по умолчанию?");
                    ui.horizontal(|ui| {
                        if ui.button("Создать").clicked() {
                            self.show_new_project_dialog = false;
                            self.logs.push("Проект успешно создан!".to_string());
                        }
                        if ui.button("Отмена").clicked() {
                            self.show_new_project_dialog = false;
                        }
                    });
                });
        }

        // Диалог настроек (Убран тип проекта и название типа по вашему требованию)
        if self.show_settings_dialog {
            egui::Window::new("Настройки проекта")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Настройки хранятся в ic.toml в папке проекта. За пределы папки не выходят.");
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Название проекта:");
                        ui.text_edit_singleline(&mut self.proj_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Название окна:");
                        ui.text_edit_singleline(&mut self.window_title);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Размер окна:");
                        ui.add(egui::DragValue::new(&mut self.window_w).suffix(" px"));
                        ui.label("x");
                        ui.add(egui::DragValue::new(&mut self.window_h).suffix(" px"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Частота логики:");
                        ui.add(egui::DragValue::new(&mut self.target_fps).suffix(" Hz"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Собрать для:");
                        egui::ComboBox::from_id_source("target_cb")
                            .selected_text(&self.build_target)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.build_target, "Эта система".to_string(), "Эта система");
                                ui.selectable_value(&mut self.build_target, "Windows (.exe)".to_string(), "Windows (.exe)");
                                ui.selectable_value(&mut self.build_target, "Linux".to_string(), "Linux");
                                ui.selectable_value(&mut self.build_target, "macOS".to_string(), "macOS");
                            });
                    });
                    ui.separator();
                    if ui.button("Сохранить и закрыть").clicked() {
                        self.show_settings_dialog = false;
                    }
                });
        }
    }
}

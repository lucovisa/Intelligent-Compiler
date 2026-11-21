use super::*;
use ic_build::downloads;
use std::sync::mpsc::Sender as DlSender;

pub enum DlEvent {
    Update(Result<Option<String>, String>),
    Installed(Result<String, String>),
    Tool(String, Result<PathBuf, String>),
}

#[derive(Default)]
pub struct DownloadState {
    pub busy: bool,
    pub update_msg: String,
    pub update_tag: Option<String>,
    pub tool_msgs: HashMap<String, String>,
    pub busy_tool: Option<String>,
    pub installed: HashSet<String>,
    pub drop_msg: String,
}

// Какие языки уже установлены
pub fn installed_tools() -> HashSet<String> {
    let Some(data) = ic_build::confine::data_dir() else {
        return HashSet::new();
    };
    ic_build::tools::TOOLS
        .iter()
        .filter(|t| ic_build::tools::is_installed(t.id, &data))
        .map(|t| t.id.to_string())
        .collect()
}

impl IcApp {
    // Перетаскивание в главное меню: папка проекта или файлы .lang / .theme
    pub(super) fn handle_menu_drop(&mut self, files: &[egui::DroppedFile]) {
        let lang = self.settings.lang;
        let Some(data) = ic_build::confine::data_dir() else {
            return;
        };
        let mut messages = Vec::new();
        for file in files {
            let Some(path) = file.path.as_ref() else {
                continue;
            };
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if path.is_dir() {
                if is_project_dir(path) {
                    self.open_project(path.clone());
                    return;
                }
                messages.push(format!(
                    "{}: {}",
                    path.display(),
                    t2(
                        lang,
                        "это не проект (нужен main.lua или ic.toml)",
                        "not a project (needs main.lua or ic.toml)"
                    )
                ));
            } else if ext == "lang" || ext == "theme" {
                let folder = data.join(if ext == "lang" { "languages" } else { "themes" });
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let target = folder.join(&name);
                match fs::create_dir_all(&folder).and_then(|()| fs::copy(path, &target)) {
                    Ok(_) => {
                        messages.push(format!("{} {name}", t2(lang, "Установлено:", "Installed:")))
                    }
                    Err(e) => messages.push(format!("{name}: {e}")),
                }
            } else {
                messages.push(format!(
                    "{}: {}",
                    path.display(),
                    t2(
                        lang,
                        "в главное меню можно бросать папку проекта, .lang или .theme",
                        "drop a project folder, .lang or .theme here"
                    )
                ));
            }
        }
        if !messages.is_empty() {
            self.dl.drop_msg = messages.join("\n");
            self.reload_custom();
        }
    }
    // Разбор результатов фоновых загрузок
    pub(super) fn dl_poll(&mut self) {
        let lang = self.settings.lang;
        while let Ok(event) = self.dl_rx.try_recv() {
            self.dl.busy = false;
            match event {
                DlEvent::Update(Ok(Some(tag))) => {
                    if downloads::is_newer(env!("CARGO_PKG_VERSION"), &tag) {
                        self.dl.update_msg =
                            format!("{} {tag}", t2(lang, "Доступна версия", "New version"));
                        self.dl.update_tag = Some(tag);
                    } else {
                        self.dl.update_msg = t2(
                            lang,
                            "Установлена последняя версия",
                            "You have the latest version",
                        )
                        .into();
                        self.dl.update_tag = None;
                    }
                }
                DlEvent::Update(Ok(None)) => {
                    self.dl.update_msg = t2(
                        lang,
                        "Релизов на GitHub пока нет",
                        "No releases on GitHub yet",
                    )
                    .into();
                }
                DlEvent::Update(Err(e)) | DlEvent::Installed(Err(e)) => self.dl.update_msg = e,
                DlEvent::Installed(Ok(tag)) => {
                    self.dl.update_tag = None;
                    self.dl.update_msg = format!(
                        "{tag}: {}",
                        t2(lang, "обновлено, перезапусти IC", "updated, restart IC")
                    );
                }
                DlEvent::Tool(id, Ok(_)) => {
                    self.dl.busy_tool = None;
                    self.dl
                        .tool_msgs
                        .insert(id, t2(lang, "Установлено", "Installed").into());
                    self.reload_langs();
                    self.dl.installed = installed_tools();
                }
                DlEvent::Tool(id, Err(e)) => {
                    self.dl.busy_tool = None;
                    self.dl.tool_msgs.insert(id, e);
                }
            }
        }
    }

    pub(super) fn dl_check_update(&mut self) {
        if self.dl.busy {
            return;
        }
        self.dl.busy = true;
        self.dl.update_msg.clear();
        let (tx, ctx): (DlSender<DlEvent>, _) = (self.dl_tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let _ = tx.send(DlEvent::Update(downloads::latest_tag()));
            ctx.request_repaint();
        });
    }

    pub(super) fn dl_install_update(&mut self) {
        let (Some(tag), Some(data)) = (self.dl.update_tag.clone(), ic_build::confine::data_dir())
        else {
            return;
        };
        if self.dl.busy {
            return;
        }
        self.dl.busy = true;
        let (tx, ctx) = (self.dl_tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let result = (|| {
                let exe = env::current_exe().map_err(|e| e.to_string())?;
                let zip = downloads::download_update(&tag, &data.join("updates"))?;
                downloads::install_update(&zip, &exe)?;
                let _ = fs::remove_file(zip);
                Ok(tag)
            })();
            let _ = tx.send(DlEvent::Installed(result));
            ctx.request_repaint();
        });
    }

    pub(super) fn dl_install_tool(&mut self, id: String) {
        let Some(data) = ic_build::confine::data_dir() else {
            return;
        };
        if self.dl.busy || self.dl.busy_tool.is_some() {
            return;
        }
        self.dl.busy_tool = Some(id.clone());
        self.dl.tool_msgs.remove(&id);
        let (tx, ctx) = (self.dl_tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let result = ic_build::tools::install_and_register(&id, &data);
            let _ = tx.send(DlEvent::Tool(id, result));
            ctx.request_repaint();
        });
    }
}

fn card_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().strong());
    ui.add_space(6.0);
}

// Правая колонка главного меню: обновления и языки
pub fn downloads_ui(
    ui: &mut egui::Ui,
    dl: &DownloadState,
    lang: Lang,
    action: &mut Option<Action>,
) {
    ui.heading(t2(lang, "Загрузки", "Downloads"));
    ui.label(RichText::new(t2(
        lang,
        "Обновления и дополнительные языки",
        "Updates and extra languages",
    )));
    ui.add_space(10.0);
    let style = ui.style().clone();

    egui::Frame::group(&style)
        .rounding(Rounding::same(7.0))
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            card_title(ui, t2(lang, "ОБНОВЛЕНИЕ IC", "IC UPDATE"));
            ui.label(format!(
                "{} v{}",
                t2(lang, "Установлена версия", "Installed version"),
                env!("CARGO_PKG_VERSION")
            ));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if dl.busy {
                    ui.spinner();
                    ui.label(RichText::new(t2(lang, "Подожди…", "Please wait…")));
                } else if let Some(tag) = &dl.update_tag {
                    if primary_button(ui, &format!("{} {tag}", t2(lang, "Установить", "Install")))
                        .clicked()
                    {
                        *action = Some(Action::InstallUpdate);
                    }
                } else if button(ui, t2(lang, "Проверить обновления", "Check for updates"))
                    .clicked()
                {
                    *action = Some(Action::CheckUpdate);
                }
            });
            if !dl.update_msg.is_empty() {
                ui.label(RichText::new(&dl.update_msg).small());
            }
            ui.label(RichText::new(format!("github.com/{}/releases", downloads::REPO)).small());
        });

    ui.add_space(10.0);
    egui::Frame::group(&style)
        .rounding(Rounding::same(7.0))
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            card_title(ui, t2(lang, "ЯЗЫКИ", "LANGUAGES"));
            ui.horizontal(|ui| {
                ui.label("Lua 5.4");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(Color32::from_rgb(0x4E, 0xC9, 0x7B), t2(lang, "Встроен", "Built in"));
                });
            });
            ui.label(
                RichText::new(t2(
                    lang,
                    "Компилятор в байт-код, ничего скачивать не нужно",
                    "Bytecode compiler, nothing to download",
                ))
                .small(),
            );
            ui.separator();
            for tool in ic_build::tools::TOOLS {
                ui.horizontal(|ui| {
                    ui.label(tool.name);
                    ui.label(RichText::new(tool.size).small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if dl.installed.contains(tool.id) {
                            ui.colored_label(
                                Color32::from_rgb(0x4E, 0xC9, 0x7B),
                                t2(lang, "Установлен", "Installed"),
                            );
                        } else if dl.busy_tool.as_deref() == Some(tool.id) {
                            ui.spinner();
                        } else if button_on(
                            ui,
                            !dl.busy && dl.busy_tool.is_none(),
                            t2(lang, "Скачать", "Download"),
                        )
                        .clicked()
                        {
                            *action = Some(Action::InstallTool(tool.id.to_string()));
                        }
                    });
                });
                if let Some(msg) = dl.tool_msgs.get(tool.id) {
                    ui.label(RichText::new(msg).small());
                }
            }
            if !dl.drop_msg.is_empty() {
                ui.label(RichText::new(&dl.drop_msg).small());
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(t2(
                    lang,
                    "Свой язык или тема: файл .lang в IC-data/languages, файл .theme в IC-data/themes (там есть _template.txt)",
                    "Your own language or theme: a .lang file in IC-data/languages, a .theme file in IC-data/themes (see _template.txt)",
                ))
                .small()
                ,
            );
        });
}

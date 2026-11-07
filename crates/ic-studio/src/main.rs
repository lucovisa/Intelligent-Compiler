#![cfg_attr(windows, windows_subsystem = "windows")]
//! IC Studio — интерфейс движка: тёмная/светлая тема, локализация,
//! создание проекта, запуск и сборка. Настройки лежат рядом с программой
//! (ic-settings.txt), в систему ничего не пишется.
mod i18n;

use eframe::egui;
use i18n::{tr, Lang, K};
use std::{
    env, fs,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{channel, Receiver, Sender},
    thread::{self, JoinHandle},
};

fn exe_dir() -> Option<PathBuf> {
    env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

fn ic_path() -> Option<PathBuf> {
    let p = exe_dir()?.join(if cfg!(windows) { "ic.exe" } else { "ic" });
    p.is_file().then_some(p)
}

struct Settings {
    dark: bool,
    lang: Lang,
}

impl Settings {
    fn path() -> PathBuf {
        exe_dir().unwrap_or_default().join("ic-settings.txt")
    }

    fn system_lang() -> Lang {
        let v = env::var("LC_ALL")
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default();
        if v.starts_with("ru") {
            Lang::Ru
        } else {
            Lang::En
        }
    }

    fn load() -> Self {
        let mut s = Settings {
            dark: true,
            lang: Self::system_lang(),
        };
        if let Ok(text) = fs::read_to_string(Self::path()) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match (k.trim(), v.trim()) {
                        ("theme", v) => s.dark = v != "light",
                        ("lang", v) => {
                            if let Some(l) = Lang::from_code(v) {
                                s.lang = l;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        s
    }

    fn save(&self) {
        let theme = if self.dark { "dark" } else { "light" };
        let _ = fs::write(
            Self::path(),
            format!("theme={theme}\nlang={}\n", self.lang.code()),
        );
    }
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
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

struct Studio {
    settings: Settings,
    folder: String,
    log: Vec<String>,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl Studio {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        apply_theme(&cc.egui_ctx, settings.dark);
        let (tx, rx) = channel();
        Self {
            settings,
            folder: "my_game".into(),
            log: Vec::new(),
            tx,
            rx,
        }
    }

    /// Запускает `ic <args>` и стримит вывод в журнал.
    fn run_ic(&mut self, ctx: &egui::Context, args: Vec<String>) {
        let Some(ic) = ic_path() else {
            self.log
                .push(tr(self.settings.lang, K::IcMissing).to_string());
            return;
        };
        self.log.push(format!("> ic {}", args.join(" ")));
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let mut cmd = Command::new(ic);
            cmd.args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // без мигающего консольного окна
            }
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
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(line) = self.rx.try_recv() {
            self.log.push(line);
        }
        if self.log.len() > 1000 {
            let n = self.log.len() - 1000;
            self.log.drain(..n);
        }

        let lang = self.settings.lang;
        let mut changed = false;

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("IC Studio");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_source("lang")
                        .selected_text(lang.label())
                        .show_ui(ui, |ui| {
                            for l in Lang::ALL {
                                if ui
                                    .selectable_value(&mut self.settings.lang, l, l.label())
                                    .clicked()
                                {
                                    changed = true;
                                }
                            }
                        });
                    ui.label(tr(lang, K::Language));
                    ui.add_space(12.0);
                    if ui
                        .selectable_value(&mut self.settings.dark, false, tr(lang, K::Light))
                        .clicked()
                    {
                        changed = true;
                    }
                    if ui
                        .selectable_value(&mut self.settings.dark, true, tr(lang, K::Dark))
                        .clicked()
                    {
                        changed = true;
                    }
                    ui.label(tr(lang, K::Theme));
                });
            });
        });

        if changed {
            apply_theme(ctx, self.settings.dark);
            self.settings.save();
        }

        let lang = self.settings.lang;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(tr(lang, K::ProjectFolder));
            ui.text_edit_singleline(&mut self.folder);
            ui.add_space(6.0);

            let folder = self.folder.trim().to_string();
            ui.horizontal(|ui| {
                if ui.button(tr(lang, K::NewProject)).clicked() {
                    self.run_ic(ctx, vec!["new".into(), folder.clone()]);
                }
                if ui.button(tr(lang, K::Run)).clicked() {
                    self.run_ic(ctx, vec![folder.clone()]);
                }
                if ui.button(tr(lang, K::Build)).clicked() {
                    self.run_ic(ctx, vec!["build".into(), folder.clone()]);
                }
            });
            ui.weak(tr(lang, K::Hint));

            ui.separator();
            ui.horizontal(|ui| {
                ui.strong(tr(lang, K::Log));
                if ui.small_button(tr(lang, K::ClearLog)).clicked() {
                    self.log.clear();
                }
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
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 560.0])
            .with_title("IC Studio"),
        ..Default::default()
    };
    eframe::run_native(
        "IC Studio",
        options,
        Box::new(|cc| Ok(Box::new(Studio::new(cc)))),
    )
}

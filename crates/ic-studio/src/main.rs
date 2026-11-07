#![cfg_attr(windows, windows_subsystem = "windows")]
//! IC Studio — интерфейс движка: тёмная/светлая тема, локализация,
//! создание проекта, запуск и сборка. Настройки лежат рядом с программой
//! (ic-settings.txt), проекты создаются рядом с программой, в систему ничего не пишется.
mod i18n;

use eframe::egui::{self, Color32, Margin, RichText, Rounding, Stroke};
use i18n::{tr, Lang, K};
use std::{
    env, fs,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{channel, Receiver, Sender},
    thread::{self, JoinHandle},
};

const ICON_RGBA: &[u8] = include_bytes!("../assets/icon.rgba"); // 256x256
const LOGO_RGBA: &[u8] = include_bytes!("../assets/logo.rgba"); // 128x128
const ACCENT: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);

fn exe_dir() -> Option<PathBuf> {
    env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

fn ic_path() -> Option<PathBuf> {
    let p = exe_dir()?.join(if cfg!(windows) { "ic.exe" } else { "ic" });
    p.is_file().then_some(p)
}

/// Относительные пути считаются от папки Studio, а не от того места, откуда её запустили.
fn resolve_folder(input: &str) -> PathBuf {
    let p = PathBuf::from(input.trim());
    if p.is_absolute() { p } else { exe_dir().unwrap_or_default().join(p) }
}

/// Кэши шейдеров драйверов (Mesa / NVIDIA) по умолчанию лежат в ~/.cache — переносим рядом.
fn confine_caches() {
    if let Some(dir) = exe_dir() {
        let cache = dir.join("cache");
        let _ = fs::create_dir_all(&cache);
        let set = |k: &str, v: &std::ffi::OsStr| {
            if env::var_os(k).is_none() {
                env::set_var(k, v);
            }
        };
        set("MESA_SHADER_CACHE_DIR", cache.as_os_str());
        set("__GL_SHADER_DISK_CACHE_PATH", cache.as_os_str());
        // потолок размера кэша: драйвер сам вытесняет старое
        set("MESA_SHADER_CACHE_MAX_SIZE", std::ffi::OsStr::new("256M"));
        set("__GL_SHADER_DISK_CACHE_SIZE", std::ffi::OsStr::new("268435456"));
    }
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
        let v = env::var("LC_ALL").or_else(|_| env::var("LANG")).unwrap_or_default();
        if v.starts_with("ru") { Lang::Ru } else { Lang::En }
    }

    fn load() -> Self {
        let mut s = Settings { dark: true, lang: Self::system_lang() };
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
        let _ = fs::write(Self::path(), format!("theme={theme}\nlang={}\n", self.lang.code()));
    }
}

fn setup_style(ctx: &egui::Context, dark: bool) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (egui::TextStyle::Heading, egui::FontId::new(24.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Body, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Button, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Small, egui::FontId::new(13.0, egui::FontFamily::Proportional)),
        (egui::TextStyle::Monospace, egui::FontId::new(14.0, egui::FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 8.0);
    style.spacing.interact_size.y = 36.0;

    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let (panel, idle, hover, line, extreme) = if dark {
        (
            Color32::from_rgb(0x12, 0x13, 0x18),
            Color32::from_rgb(0x25, 0x27, 0x31),
            Color32::from_rgb(0x30, 0x33, 0x41),
            Color32::from_rgb(0x2A, 0x2C, 0x38),
            Color32::from_rgb(0x0B, 0x0C, 0x10),
        )
    } else {
        (
            Color32::from_rgb(0xF3, 0xF4, 0xF9),
            Color32::from_rgb(0xFF, 0xFF, 0xFF),
            Color32::from_rgb(0xE9, 0xEA, 0xF5),
            Color32::from_rgb(0xDD, 0xDF, 0xEA),
            Color32::from_rgb(0xFF, 0xFF, 0xFF),
        )
    };
    v.panel_fill = panel;
    v.window_fill = panel;
    v.extreme_bg_color = extreme;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, line);
    v.widgets.inactive.bg_fill = idle;
    v.widgets.inactive.weak_bg_fill = idle;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, line);
    v.widgets.hovered.bg_fill = hover;
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(10.0);
    }
    style.visuals = v;
    ctx.set_style(style);
}

fn card_fill(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(0x1A, 0x1C, 0x24) } else { Color32::WHITE }
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let fill = card_fill(ui);
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    egui::Frame::none().fill(fill).stroke(stroke).rounding(14.0).inner_margin(Margin::same(18.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
            .fill(ACCENT)
            .min_size(egui::vec2(140.0, 40.0)),
    )
}

fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(text).min_size(egui::vec2(140.0, 40.0)))
}

fn pump<R: Read + Send + 'static>(r: Option<R>, tx: Sender<String>, ctx: egui::Context) -> Option<JoinHandle<()>> {
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
    logo: egui::TextureHandle,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl Studio {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        setup_style(&cc.egui_ctx, settings.dark);
        let logo = cc.egui_ctx.load_texture(
            "ic-logo",
            egui::ColorImage::from_rgba_unmultiplied([128, 128], LOGO_RGBA),
            egui::TextureOptions::LINEAR,
        );
        let (tx, rx) = channel();
        Self { settings, folder: "games/my_game".into(), log: Vec::new(), logo, tx, rx }
    }

    /// Запускает `ic <args>` и стримит вывод в журнал.
    fn run_ic(&mut self, ctx: &egui::Context, args: Vec<String>) {
        let Some(ic) = ic_path() else {
            self.log.push(tr(self.settings.lang, K::IcMissing).to_string());
            return;
        };
        self.log.push(format!("> ic {}", args.join(" ")));
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let cwd = exe_dir().unwrap_or_default();
        thread::spawn(move || {
            let mut cmd = Command::new(ic);
            cmd.args(&args)
                .current_dir(cwd)
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
        let panel = ctx.style().visuals.panel_fill;
        let mut changed = false;

        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::none().fill(panel).inner_margin(Margin::symmetric(22.0, 14.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(self.logo.id(), egui::vec2(38.0, 38.0))));
                    ui.label(RichText::new("IC Studio").size(22.0).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // переключатель темы: солнце / луна
                        let pill_fill = card_fill(ui);
                        let pill = egui::Frame::none()
                            .fill(pill_fill)
                            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                            .rounding(22.0)
                            .inner_margin(Margin::symmetric(5.0, 3.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 3.0;
                                    let sun = ui
                                        .selectable_value(&mut self.settings.dark, false, RichText::new("☀").size(20.0))
                                        .on_hover_text(tr(lang, K::Light));
                                    let moon = ui
                                        .selectable_value(&mut self.settings.dark, true, RichText::new("🌙").size(20.0))
                                        .on_hover_text(tr(lang, K::Dark));
                                    if sun.clicked() || moon.clicked() {
                                        changed = true;
                                    }
                                });
                            });
                        pill.response.on_hover_text(tr(lang, K::Theme));

                        ui.add_space(8.0);
                        egui::ComboBox::from_id_source("lang").selected_text(lang.label()).show_ui(ui, |ui| {
                            for l in Lang::ALL {
                                if ui.selectable_value(&mut self.settings.lang, l, l.label()).clicked() {
                                    changed = true;
                                }
                            }
                        });
                        ui.label(tr(lang, K::Language));
                    });
                });
            });

        if changed {
            setup_style(ctx, self.settings.dark);
            self.settings.save();
        }

        let lang = self.settings.lang;
        let panel = ctx.style().visuals.panel_fill;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(panel).inner_margin(Margin::symmetric(22.0, 6.0)))
            .show(ctx, |ui| {
                card(ui, |ui| {
                    ui.label(RichText::new(tr(lang, K::ProjectFolder)).strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut self.folder)
                            .desired_width(f32::INFINITY)
                            .hint_text("games/my_game"),
                    );
                    let full = resolve_folder(&self.folder);
                    ui.weak(format!("→ {}", full.display()));

                    let path = full.to_string_lossy().into_owned();
                    ui.horizontal(|ui| {
                        if secondary_button(ui, tr(lang, K::NewProject)).clicked() {
                            self.run_ic(ctx, vec!["new".into(), path.clone()]);
                        }
                        if primary_button(ui, tr(lang, K::Run)).clicked() {
                            self.run_ic(ctx, vec![path.clone()]);
                        }
                        if secondary_button(ui, tr(lang, K::Build)).clicked() {
                            self.run_ic(ctx, vec!["build".into(), path.clone()]);
                        }
                    });
                    ui.weak(tr(lang, K::Hint));
                });

                ui.add_space(4.0);
                let avail = ui.available_height();
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr(lang, K::Log)).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(tr(lang, K::ClearLog)).clicked() {
                                self.log.clear();
                            }
                        });
                    });
                    let console_fill = ui.visuals().extreme_bg_color;
                    egui::Frame::none().fill(console_fill).rounding(10.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .max_height((avail - 120.0).max(80.0))
                            .show(ui, |ui| {
                                for line in &self.log {
                                    ui.monospace(line);
                                }
                            });
                    });
                });
            });
    }
}

fn main() -> eframe::Result<()> {
    confine_caches();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 640.0])
            .with_min_inner_size([680.0, 460.0])
            .with_title("IC Studio")
            .with_icon(egui::IconData { rgba: ICON_RGBA.to_vec(), width: 256, height: 256 }),
        ..Default::default()
    };
    eframe::run_native("IC Studio", options, Box::new(|cc| Ok(Box::new(Studio::new(cc)))))
}

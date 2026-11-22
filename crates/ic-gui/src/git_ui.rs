use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Entry {
    pub x: char,
    pub y: char,
    pub path: String,
}

impl Entry {
    fn staged(&self) -> bool {
        self.x != ' ' && self.x != '?'
    }
}

pub struct GitState {
    pub open: bool,
    pub name: String,
    pub email: String,
    pub identity_dirty: bool,
    available: bool,
    is_repo: bool,
    has_identity: bool,
    branch: String,
    upstream: String,
    ahead: u32,
    behind: u32,
    entries: Vec<Entry>,
    history: Vec<String>,
    message: String,
    remote_url: String,
    token: String,
    selected: Option<String>,
    diff: String,
    note: String,
    dir: Option<PathBuf>,
    last: Option<Instant>,
    busy: Arc<AtomicBool>,
    stale: Arc<AtomicBool>,
}

impl GitState {
    pub fn new(name: String, email: String) -> Self {
        GitState {
            open: false,
            name,
            email,
            identity_dirty: false,
            available: true,
            is_repo: false,
            has_identity: false,
            branch: String::new(),
            upstream: String::new(),
            ahead: 0,
            behind: 0,
            entries: Vec::new(),
            history: Vec::new(),
            message: String::new(),
            remote_url: String::new(),
            token: String::new(),
            selected: None,
            diff: String::new(),
            note: String::new(),
            dir: None,
            last: None,
            busy: Arc::new(AtomicBool::new(false)),
            stale: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn base_command(dir: &Path, token: &str) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .arg("-c")
        .arg("core.quotepath=off")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C");
    if let Some(data) = ic_build::confine::data_dir() {
        ic_build::confine::child_env(&mut cmd, &data);
    }
    if !token.is_empty() {
        let header = format!(
            "AUTHORIZATION: basic {}",
            base64(&format!("x-access-token:{token}"))
        );
        cmd.env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.extraheader")
            .env("GIT_CONFIG_VALUE_0", header);
    }
    hide_console(&mut cmd);
    cmd
}

fn base64(s: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in s.as_bytes().chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

// Запуск git: Ok(stdout) или Err(текст ошибки)
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = base_command(dir, "")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Err(if err.is_empty() { text } else { err })
    }
}

// Разбор `git status --porcelain=v1 -b -z`
fn parse_status(raw: &str, st: &mut GitState) {
    st.entries.clear();
    st.branch.clear();
    st.upstream.clear();
    st.ahead = 0;
    st.behind = 0;
    let mut parts = raw.split('\0').filter(|p| !p.is_empty());
    while let Some(part) = parts.next() {
        if let Some(head) = part.strip_prefix("## ") {
            let head = head.strip_prefix("No commits yet on ").unwrap_or(head);
            let (names, counts) = match head.split_once(" [") {
                Some((n, c)) => (n, c.trim_end_matches(']')),
                None => (head, ""),
            };
            match names.split_once("...") {
                Some((b, u)) => {
                    st.branch = b.to_string();
                    st.upstream = u.to_string();
                }
                None => st.branch = names.to_string(),
            }
            for c in counts.split(", ") {
                if let Some(n) = c.strip_prefix("ahead ") {
                    st.ahead = n.parse().unwrap_or(0);
                }
                if let Some(n) = c.strip_prefix("behind ") {
                    st.behind = n.parse().unwrap_or(0);
                }
            }
            continue;
        }
        let mut chars = part.chars();
        let (Some(x), Some(y)) = (chars.next(), chars.next()) else {
            continue;
        };
        let path: String = part.chars().skip(3).collect();
        if x == 'R' || x == 'C' {
            parts.next();
        }
        st.entries.push(Entry { x, y, path });
    }
}

fn refresh(st: &mut GitState, dir: &Path) {
    st.last = Some(Instant::now());
    st.available = Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !st.available {
        return;
    }
    match git(dir, &["status", "--porcelain=v1", "-b", "-z", "-uall"]) {
        Ok(raw) => {
            st.is_repo = true;
            parse_status(&raw, st);
        }
        Err(_) => {
            st.is_repo = false;
            st.entries.clear();
            st.history.clear();
            return;
        }
    }
    st.has_identity = git(dir, &["config", "user.name"]).is_ok_and(|v| !v.trim().is_empty())
        && git(dir, &["config", "user.email"]).is_ok_and(|v| !v.trim().is_empty());
    st.remote_url = git(dir, &["remote", "get-url", "origin"])
        .map(|v| v.trim().to_string())
        .unwrap_or_default();
    st.history = git(dir, &["log", "--oneline", "-n", "10"])
        .map(|v| v.lines().map(str::to_string).collect())
        .unwrap_or_default();
    if let Some(sel) = st.selected.clone() {
        st.diff = diff_of(dir, &sel, &st.entries);
    }
}

fn diff_of(dir: &Path, path: &str, entries: &[Entry]) -> String {
    let untracked = entries.iter().any(|e| e.path == path && e.x == '?');
    let text = if untracked {
        fs::read_to_string(dir.join(path)).map(|t| {
            t.lines()
                .map(|l| format!("+{l}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
    } else {
        let staged_only = entries
            .iter()
            .any(|e| e.path == path && e.staged() && e.y == ' ');
        let mut args = vec!["diff"];
        if staged_only {
            args.push("--cached");
        }
        args.push("--");
        args.push(path);
        git(dir, &args).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    };
    let text = text.unwrap_or_else(|e| e.to_string());
    let mut lines: Vec<&str> = text.lines().take(300).collect();
    if text.lines().count() > 300 {
        lines.push("…");
    }
    lines.join("\n")
}

// Сетевая операция в фоне: вывод идёт в журнал
fn spawn_net(st: &GitState, dir: &Path, args: Vec<String>, tx: Sender<String>, ctx: egui::Context) {
    st.busy.store(true, Ordering::SeqCst);
    let busy = st.busy.clone();
    let stale = st.stale.clone();
    let dir = dir.to_path_buf();
    let token = st.token.clone();
    thread::spawn(move || {
        let _ = tx.send(format!("> git {}", args.join(" ")));
        let mut cmd = base_command(&dir, &token);
        cmd.args(&args);
        match cmd.output() {
            Ok(out) => {
                for line in String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .chain(String::from_utf8_lossy(&out.stderr).lines())
                {
                    let _ = tx.send(line.to_string());
                }
                let _ = tx.send(format!("exit status: {}", out.status.code().unwrap_or(-1)));
            }
            Err(e) => {
                let _ = tx.send(format!("error: {e}"));
            }
        }
        busy.store(false, Ordering::SeqCst);
        stale.store(true, Ordering::SeqCst);
        ctx.request_repaint();
    });
}

fn run(st: &mut GitState, dir: &Path, args: &[&str]) {
    st.note = match git(dir, args) {
        Ok(_) => String::new(),
        Err(e) => e,
    };
    refresh(st, dir);
}

pub fn show(
    ctx: &egui::Context,
    st: &mut GitState,
    project: &Path,
    lang: Lang,
    tx: &Sender<String>,
    dark: bool,
) {
    if st.dir.as_deref() != Some(project) {
        st.dir = Some(project.to_path_buf());
        st.selected = None;
        st.diff.clear();
        st.note.clear();
        st.message.clear();
        refresh(st, project);
    }
    if st.stale.swap(false, Ordering::SeqCst) {
        refresh(st, project);
    }
    if st.last.is_none_or(|t| t.elapsed() > Duration::from_secs(3))
        && !st.busy.load(Ordering::SeqCst)
    {
        refresh(st, project);
    }
    ctx.request_repaint_after(Duration::from_secs(3));

    let mut open = st.open;
    let busy = st.busy.load(Ordering::SeqCst);
    egui::Window::new("Git")
        .open(&mut open)
        .default_width(520.0)
        .default_height(560.0)
        .show(ctx, |ui| {
            if !st.available {
                ui.label(t2(
                    lang,
                    "Git не найден. Установите его и перезапустите программу.",
                    "Git was not found. Install it and restart the program.",
                ));
                ui.hyperlink("https://git-scm.com/downloads");
                return;
            }
            if !st.is_repo {
                ui.label(t2(
                    lang,
                    "В этой папке нет репозитория Git.",
                    "This folder is not a Git repository.",
                ));
                if primary_button(ui, t2(lang, "Создать репозиторий", "Initialize repository"))
                    .clicked()
                {
                    if git(project, &["init", "-b", "main"]).is_err() {
                        run(st, project, &["init"]);
                    } else {
                        refresh(st, project);
                    }
                }
                if !st.note.is_empty() {
                    ui.label(&st.note);
                }
                return;
            }

            ui.horizontal(|ui| {
                ui.label(RichText::new(&st.branch).strong());
                if !st.upstream.is_empty() {
                    ui.label(format!("/ {}", st.upstream));
                }
                if st.ahead > 0 {
                    ui.label(format!("{} {}", t2(lang, "впереди", "ahead"), st.ahead));
                }
                if st.behind > 0 {
                    ui.label(format!("{} {}", t2(lang, "позади", "behind"), st.behind));
                }
            });
            ui.horizontal(|ui| {
                if button(ui, t2(lang, "Обновить", "Refresh")).clicked() {
                    refresh(st, project);
                }
                if button_on(ui, !busy, t2(lang, "Получить (pull)", "Pull")).clicked() {
                    spawn_net(
                        st,
                        project,
                        vec!["pull".into(), "--ff-only".into()],
                        tx.clone(),
                        ctx.clone(),
                    );
                }
                if button_on(ui, !busy, t2(lang, "Отправить (push)", "Push")).clicked() {
                    let args = if st.upstream.is_empty() {
                        vec![
                            "push".to_string(),
                            "-u".to_string(),
                            "origin".to_string(),
                            st.branch.clone(),
                        ]
                    } else {
                        vec!["push".to_string()]
                    };
                    spawn_net(st, project, args, tx.clone(), ctx.clone());
                }
                if busy {
                    ui.spinner();
                }
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new(t2(lang, "Изменения", "Changes")).strong());
                ui.label(format!("({})", st.entries.len()));
                if button(ui, t2(lang, "Добавить всё", "Stage all")).clicked() {
                    run(st, project, &["add", "-A"]);
                }
                if button(ui, t2(lang, "Убрать всё", "Unstage all")).clicked() {
                    run(st, project, &["reset", "-q"]);
                }
            });
            let mut toggle: Option<(String, bool)> = None;
            let mut pick: Option<String> = None;
            egui::ScrollArea::vertical()
                .id_salt("git_changes")
                .max_height(170.0)
                .show(ui, |ui| {
                    for e in &st.entries {
                        ui.horizontal(|ui| {
                            let mut staged = e.staged();
                            if ui.checkbox(&mut staged, "").changed() {
                                toggle = Some((e.path.clone(), staged));
                            }
                            let mark = if e.x == '?' { 'U' } else if e.staged() { e.x } else { e.y };
                            ui.label(RichText::new(mark.to_string()).monospace());
                            let on = st.selected.as_deref() == Some(e.path.as_str());
                            if ui_color::sel_label(ui, on, e.path.as_str()).clicked() {
                                pick = Some(e.path.clone());
                            }
                        });
                    }
                    if st.entries.is_empty() {
                        ui.label(t2(lang, "Изменений нет", "No changes"));
                    }
                });
            if let Some((path, stage)) = toggle {
                if stage {
                    run(st, project, &["add", "--", &path]);
                } else {
                    run(st, project, &["reset", "-q", "--", &path]);
                }
            }
            if let Some(path) = pick {
                st.diff = diff_of(project, &path, &st.entries);
                st.selected = Some(path);
            }
            if st.selected.is_some() {
                egui::ScrollArea::both()
                    .id_salt("git_diff")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for line in st.diff.lines() {
                            let color = if line.starts_with('+') {
                                if dark {
                                    Color32::from_rgb(0x6A, 0xD0, 0x6A)
                                } else {
                                    Color32::from_rgb(0x0B, 0x7A, 0x0B)
                                }
                            } else if line.starts_with('-') {
                                if dark {
                                    Color32::from_rgb(0xF0, 0x7A, 0x7A)
                                } else {
                                    Color32::from_rgb(0xB0, 0x1E, 0x1E)
                                }
                            } else if dark {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            };
                            ui.label(RichText::new(line).monospace().color(color));
                        }
                    });
            }
            ui.separator();

            ui.label(RichText::new(t2(lang, "Сообщение коммита", "Commit message")).strong());
            ui.add(
                egui::TextEdit::multiline(&mut st.message)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            if !st.has_identity {
                ui.label(t2(
                    lang,
                    "Имя и почта для коммитов (в git они не заданы):",
                    "Name and email for commits (not set in git):",
                ));
                ui.horizontal(|ui| {
                    let a = ui.add(
                        egui::TextEdit::singleline(&mut st.name)
                            .hint_text(t2(lang, "Имя", "Name"))
                            .desired_width(150.0),
                    );
                    let b = ui.add(
                        egui::TextEdit::singleline(&mut st.email)
                            .hint_text("email")
                            .desired_width(200.0),
                    );
                    if a.changed() || b.changed() {
                        st.identity_dirty = true;
                    }
                });
            }
            let staged = st.entries.iter().any(Entry::staged);
            let identity_ok = st.has_identity || (!st.name.trim().is_empty() && !st.email.trim().is_empty());
            let can_commit = staged && !st.message.trim().is_empty() && identity_ok;
            if primary_on(ui, can_commit, t2(lang, "Закоммитить", "Commit")).clicked() {
                let msg = st.message.trim().to_string();
                let mut args: Vec<String> = Vec::new();
                if !st.has_identity {
                    args.push("-c".into());
                    args.push(format!("user.name={}", st.name.trim()));
                    args.push("-c".into());
                    args.push(format!("user.email={}", st.email.trim()));
                }
                args.push("commit".into());
                args.push("-m".into());
                args.push(msg);
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                let ok = git(project, &refs);
                st.note = ok.clone().err().unwrap_or_default();
                if ok.is_ok() {
                    st.message.clear();
                    st.selected = None;
                    st.diff.clear();
                }
                refresh(st, project);
            }
            if !st.note.is_empty() {
                ui.label(RichText::new(&st.note).color(Color32::from_rgb(0xE5, 0x5B, 0x5B)));
            }
            ui.separator();

            egui::CollapsingHeader::new(t2(lang, "GitHub и доступ", "GitHub and access"))
                .default_open(st.remote_url.is_empty())
                .show(ui, |ui| {
                    ui.label(t2(lang, "Адрес репозитория (origin):", "Repository URL (origin):"));
                    ui.add(
                        egui::TextEdit::singleline(&mut st.remote_url)
                            .hint_text("https://github.com/user/repo.git")
                            .desired_width(f32::INFINITY),
                    );
                    if button(ui, t2(lang, "Сохранить адрес", "Save URL")).clicked() {
                        let url = st.remote_url.trim().to_string();
                        let has = git(project, &["remote", "get-url", "origin"]).is_ok();
                        let sub = if has { "set-url" } else { "add" };
                        run(st, project, &["remote", sub, "origin", &url]);
                    }
                    ui.label(t2(lang, "Токен GitHub (не сохраняется на диск):", "GitHub token (not saved to disk):"));
                    ui.add(
                        egui::TextEdit::singleline(&mut st.token)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    ui.label(
                        RichText::new(t2(
                            lang,
                            "Нужен для push и pull, если вход через git не настроен. GitHub: Settings > Developer settings > Personal access tokens, право Contents: Read and write.",
                            "Needed for push and pull if git sign-in is not set up. GitHub: Settings > Developer settings > Personal access tokens, Contents: Read and write.",
                        ))
                        .small(),
                    );
                });

            if !st.history.is_empty() {
                egui::CollapsingHeader::new(t2(lang, "История", "History")).show(ui, |ui| {
                    for line in &st.history {
                        ui.label(RichText::new(line).monospace());
                    }
                });
            }
        });
    st.open = open;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ic_git_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64("a"), "YQ==");
        assert_eq!(base64("ab"), "YWI=");
        assert_eq!(base64("abc"), "YWJj");
        assert_eq!(base64("x-access-token:t"), "eC1hY2Nlc3MtdG9rZW46dA==");
    }

    #[test]
    fn status_is_parsed() {
        let mut st = GitState::new(String::new(), String::new());
        let raw = "## main...origin/main [ahead 2, behind 1]\0 M a.lua\0A  b.lua\0?? c.lua\0R  new.lua\0old.lua\0";
        parse_status(raw, &mut st);
        assert_eq!(st.branch, "main");
        assert_eq!(st.upstream, "origin/main");
        assert_eq!((st.ahead, st.behind), (2, 1));
        assert_eq!(st.entries.len(), 4);
        assert!(!st.entries[0].staged());
        assert!(st.entries[1].staged());
        assert!(!st.entries[2].staged());
        assert_eq!(st.entries[3].path, "new.lua");
    }

    #[test]
    fn commit_flow_in_real_repository() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = scratch("flow");
        git(&dir, &["init"]).unwrap();
        fs::write(dir.join("main.lua"), "print(1)\n").unwrap();
        let mut st = GitState::new("Tester".into(), "t@example.com".into());
        refresh(&mut st, &dir);
        assert!(st.is_repo);
        assert_eq!(st.entries.len(), 1);
        assert_eq!(st.entries[0].x, '?');
        git(&dir, &["add", "--", "main.lua"]).unwrap();
        refresh(&mut st, &dir);
        assert!(st.entries[0].staged());
        git(
            &dir,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-m",
                "first",
            ],
        )
        .unwrap();
        refresh(&mut st, &dir);
        assert!(st.entries.is_empty());
        assert_eq!(st.history.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}

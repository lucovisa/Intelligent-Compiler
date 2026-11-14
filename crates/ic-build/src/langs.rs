use std::{
    fs,
    path::{Path, PathBuf},
};

// Описание языка: файл `*.lang` в IC-data/languages или в папке проекта
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Language {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub icon: Option<PathBuf>,
    pub badge: String,
    pub color: [u8; 3],
    pub compiler: Option<PathBuf>,
    pub build: Vec<String>,
    pub run: Vec<String>,
    pub source: PathBuf,
}

// Строка в кавычках: "текст"
fn unquote(v: &str) -> Option<String> {
    let v = v.trim();
    let inner = v.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.to_string())
}

// Список строк: ["a", "b c"]
fn parse_list(v: &str) -> Option<Vec<String>> {
    let inner = v.trim().strip_prefix('[')?.strip_suffix(']')?;
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in inner.chars() {
        match c {
            '"' => {
                if quoted {
                    out.push(std::mem::take(&mut cur));
                }
                quoted = !quoted;
            }
            _ if quoted => cur.push(c),
            _ => {}
        }
    }
    if quoted {
        None
    } else {
        Some(out)
    }
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

// Относительный путь считается от папки файла описания
fn resolve_path(base: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

pub fn parse(text: &str, source: &Path) -> Result<Language, String> {
    let base = source.parent().unwrap_or(Path::new("."));
    let mut lang = Language {
        color: [0x8A, 0x8F, 0x98],
        source: source.to_path_buf(),
        id: source
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
        ..Default::default()
    };
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("{}: строка без «=»: {line}", source.display()));
        };
        let (key, value) = (key.trim(), value.trim());
        let bad = || format!("{}: неверное значение «{key}»", source.display());
        match key {
            "name" => lang.name = unquote(value).ok_or_else(bad)?,
            "badge" => lang.badge = unquote(value).ok_or_else(bad)?,
            "color" => {
                lang.color = parse_hex(&unquote(value).ok_or_else(bad)?).ok_or_else(bad)?;
            }
            "icon" => lang.icon = Some(resolve_path(base, &unquote(value).ok_or_else(bad)?)),
            "compiler" => {
                lang.compiler = Some(resolve_path(base, &unquote(value).ok_or_else(bad)?));
            }
            "extensions" => {
                lang.extensions = parse_list(value)
                    .ok_or_else(bad)?
                    .into_iter()
                    .map(|e| e.trim_start_matches('.').to_lowercase())
                    .filter(|e| !e.is_empty())
                    .collect();
            }
            "build" => lang.build = parse_list(value).ok_or_else(bad)?,
            "run" => lang.run = parse_list(value).ok_or_else(bad)?,
            _ => {}
        }
    }
    if lang.name.is_empty() {
        return Err(format!("{}: нет поля name", source.display()));
    }
    if lang.extensions.is_empty() {
        return Err(format!("{}: нет поля extensions", source.display()));
    }
    if lang.badge.is_empty() {
        lang.badge = lang.name.chars().take(2).collect();
    }
    Ok(lang)
}

// Все `*.lang` из папки; ошибки возвращаются списком, остальные языки грузятся
pub fn load_dir(dir: &Path) -> (Vec<Language>, Vec<String>) {
    let mut langs = Vec::new();
    let mut errors = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return (langs, errors);
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "lang"))
        .collect();
    files.sort();
    for path in files {
        match fs::read_to_string(&path) {
            Ok(text) => match parse(&text, &path) {
                Ok(l) => langs.push(l),
                Err(e) => errors.push(e),
            },
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    (langs, errors)
}

// Подстановка {compiler} {file} {out} {dir} в шаблон команды
pub fn expand(template: &[String], vars: &[(&str, &str)]) -> Vec<String> {
    template
        .iter()
        .map(|arg| {
            let mut s = arg.clone();
            for (k, v) in vars {
                s = s.replace(&format!("{{{k}}}"), v);
            }
            s
        })
        .collect()
}

impl Language {
    pub fn matches(&self, file_name: &str) -> bool {
        let ext = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
        file_name.contains('.') && self.extensions.iter().any(|e| *e == ext)
    }

    // Что IC покажет пользователю перед запуском чужого компилятора
    pub fn describe_build(&self, vars: &[(&str, &str)]) -> String {
        expand(&self.build, vars).join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "name = \"Бум Пим\"\nextensions = [\".BP\", \"bpim\"]\nicon = \"bp.png\"\n\
        color = \"#ff8800\"\ncompiler = \"/opt/bp/bpc\"\nbuild = [\"{compiler}\", \"{file}\", \"-o\", \"{out}\"]\nrun = [\"{out}\"]\n";

    #[test]
    fn parses_language() {
        let l = parse(SAMPLE, Path::new("/data/languages/boompim.lang")).unwrap();
        assert_eq!(l.id, "boompim");
        assert_eq!(l.name, "Бум Пим");
        assert_eq!(l.extensions, vec!["bp", "bpim"]);
        assert_eq!(l.icon, Some(PathBuf::from("/data/languages/bp.png")));
        assert_eq!(l.color, [0xff, 0x88, 0x00]);
        assert_eq!(l.compiler, Some(PathBuf::from("/opt/bp/bpc")));
        assert_eq!(l.badge, "Бу");
        assert!(l.matches("hello.BP"));
        assert!(!l.matches("hello.lua"));
        assert!(!l.matches("bp"));
    }

    #[test]
    fn rejects_missing_fields() {
        assert!(parse("extensions = [\"x\"]", Path::new("a.lang")).is_err());
        assert!(parse("name = \"X\"", Path::new("a.lang")).is_err());
        assert!(parse("name = X\nextensions = [\"x\"]", Path::new("a.lang")).is_err());
    }

    #[test]
    fn expands_templates() {
        let l = parse(SAMPLE, Path::new("/d/b.lang")).unwrap();
        let args = expand(
            &l.build,
            &[("compiler", "/opt/bp/bpc"), ("file", "a.bp"), ("out", "a")],
        );
        assert_eq!(args, vec!["/opt/bp/bpc", "a.bp", "-o", "a"]);
    }
}

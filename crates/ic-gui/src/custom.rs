use super::*;
use std::sync::RwLock;

// Тема оформления из файла `*.theme` в IC-data/themes
#[derive(Clone, Debug)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub panel: [u8; 3],
    pub editor: [u8; 3],
    pub idle: [u8; 3],
    pub hover: [u8; 3],
    pub border: [u8; 3],
    pub accent: [u8; 3],
    pub text: Option<[u8; 3]>,
}

static ACTIVE: RwLock<Option<Theme>> = RwLock::new(None);

pub fn set_active(theme: Option<Theme>) {
    *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = theme;
}

pub fn active() -> Option<Theme> {
    ACTIVE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

fn color(v: &str) -> Option<[u8; 3]> {
    let h = v.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

pub fn parse_theme(text: &str, id: &str) -> Result<Theme, String> {
    let dark_defaults = palette_values(true);
    let mut theme = Theme {
        id: id.to_string(),
        name: String::new(),
        dark: true,
        panel: dark_defaults[0],
        editor: dark_defaults[1],
        idle: dark_defaults[2],
        hover: dark_defaults[3],
        border: dark_defaults[4],
        accent: dark_defaults[5],
        text: None,
    };
    let mut base_set = false;
    let mut entries = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("{id}: строка {}: нет «=»", n + 1))?;
        let v = v.trim().trim_matches('"');
        entries.push((k.trim().to_string(), v.to_string(), n + 1));
    }
    // Сначала основа, чтобы остальные цвета её переопределяли
    for (k, v, _) in &entries {
        match k.as_str() {
            "name" => theme.name = v.clone(),
            "base" => {
                theme.dark = v != "light";
                let d = palette_values(theme.dark);
                theme.panel = d[0];
                theme.editor = d[1];
                theme.idle = d[2];
                theme.hover = d[3];
                theme.border = d[4];
                theme.accent = d[5];
                base_set = true;
            }
            _ => {}
        }
    }
    let _ = base_set;
    for (k, v, line) in &entries {
        let slot = match k.as_str() {
            "panel" => &mut theme.panel,
            "editor" => &mut theme.editor,
            "idle" => &mut theme.idle,
            "hover" => &mut theme.hover,
            "border" => &mut theme.border,
            "accent" => &mut theme.accent,
            "text" => {
                theme.text = Some(color(v).ok_or_else(|| format!("{id}: строка {line}: цвет должен быть вида #RRGGBB"))?);
                continue;
            }
            _ => continue,
        };
        *slot = color(v).ok_or_else(|| format!("{id}: строка {line}: цвет должен быть вида #RRGGBB"))?;
    }
    if theme.name.is_empty() {
        return Err(format!("{id}: нет поля name"));
    }
    Ok(theme)
}

pub fn load_themes(dir: &Path) -> (Vec<Theme>, Vec<String>) {
    let mut themes = Vec::new();
    let mut errors = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return (themes, errors);
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "theme"))
        .collect();
    files.sort();
    for path in files {
        let id = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        match fs::read_to_string(&path) {
            Ok(text) => match parse_theme(&text, &id) {
                Ok(t) => themes.push(t),
                Err(e) => errors.push(e),
            },
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    (themes, errors)
}

pub fn theme_template() -> String {
    let d = palette_values(true);
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    format!(
        "name = \"My theme\"\nbase = \"dark\"\npanel = \"{}\"\neditor = \"{}\"\nidle = \"{}\"\nhover = \"{}\"\nborder = \"{}\"\naccent = \"{}\"\ntext = \"#ffffff\"\n",
        hex(d[0]), hex(d[1]), hex(d[2]), hex(d[3]), hex(d[4]), hex(d[5])
    )
}

// Файлы-шаблоны в папках languages и themes (не загружаются, т.к. расширение .txt)
pub fn write_templates(data: &Path) {
    let langs = data.join("languages");
    let themes = data.join("themes");
    let _ = fs::create_dir_all(&langs);
    let _ = fs::create_dir_all(&themes);
    let lang_tpl = langs.join("_template.txt");
    if !lang_tpl.exists() {
        let _ = fs::write(lang_tpl, i18n::template_text());
    }
    let theme_tpl = themes.join("_template.txt");
    if !theme_tpl.exists() {
        let _ = fs::write(theme_tpl, theme_template());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_theme_with_overrides() {
        let t = parse_theme("name = \"Sol\"\nbase = \"light\"\naccent = \"#268bd2\"\ntext = \"#111111\"\n", "sol").unwrap();
        assert!(!t.dark);
        assert_eq!(t.accent, [0x26, 0x8b, 0xd2]);
        assert_eq!(t.text, Some([0x11, 0x11, 0x11]));
        assert_eq!(t.panel, palette_values(false)[0]);
    }

    #[test]
    fn rejects_bad_theme() {
        assert!(parse_theme("accent = \"#12\"\nname = \"x\"", "x").is_err());
        assert!(parse_theme("accent = \"#112233\"", "x").is_err());
    }

    #[test]
    fn template_is_valid_theme() {
        let t = parse_theme(&theme_template(), "tpl").unwrap();
        assert_eq!(t.name, "My theme");
        assert!(t.dark);
    }
}

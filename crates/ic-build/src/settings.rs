use std::{fs, path::Path};

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSettings {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub tick_rate: u32,
    pub background: [u8; 3],
    pub target: String,
    pub kind: String,
    pub dimension: String,
    pub label: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            title: "IC Game".into(),
            width: 1280,
            height: 720,
            tick_rate: 60,
            background: [0x26, 0x26, 0x3F],
            target: "this".into(),
            kind: "game".into(),
            dimension: "2d".into(),
            label: String::new(),
        }
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

fn parse_range(v: &str, lo: u32, hi: u32) -> Option<u32> {
    v.parse::<u32>().ok().filter(|x| (lo..=hi).contains(x))
}

impl ProjectSettings {
    pub const FILE: &'static str = "ic.toml";

    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut s = Self::default();
        let mut warn = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let n = n + 1;
            let Some((k, v)) = line.split_once('=') else {
                warn.push(format!("ic.toml:{n}: ожидалось «ключ = значение»"));
                continue;
            };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            match k {
                "title" => s.title = v.to_string(),
                "width" => match parse_range(v, 320, 7680) {
                    Some(x) => s.width = x,
                    None => warn.push(format!("ic.toml:{n}: width — число от 320 до 7680")),
                },
                "height" => match parse_range(v, 240, 4320) {
                    Some(x) => s.height = x,
                    None => warn.push(format!("ic.toml:{n}: height — число от 240 до 4320")),
                },
                "tick_rate" => match parse_range(v, 10, 480) {
                    Some(x) => s.tick_rate = x,
                    None => warn.push(format!("ic.toml:{n}: tick_rate — число от 10 до 480")),
                },
                "background" => match parse_hex(v) {
                    Some(c) => s.background = c,
                    None => warn.push(format!("ic.toml:{n}: background — цвет вида #rrggbb")),
                },
                "target" => {
                    if ["this", "windows", "linux", "macos"].contains(&v) {
                        s.target = v.to_string();
                    } else {
                        warn.push(format!("ic.toml:{n}: target — this, windows, linux или macos"));
                    }
                }
                "kind" => {
                    if ["app", "game"].contains(&v) {
                        s.kind = v.to_string();
                    } else {
                        warn.push(format!("ic.toml:{n}: kind — app или game"));
                    }
                }
                "dimension" => {
                    if ["2d", "3d", "custom"].contains(&v) {
                        s.dimension = v.to_string();
                    } else {
                        warn.push(format!("ic.toml:{n}: dimension — 2d, 3d или custom"));
                    }
                }
                "label" => s.label = v.to_string(),
                _ => warn.push(format!("ic.toml:{n}: неизвестный ключ «{k}»")),
            }
        }
        (s, warn)
    }

    pub fn to_toml(&self) -> String {
        let [r, g, b] = self.background;
        format!(
            "# Настройки проекта IC. Меняй здесь или в окне IC → «Настройки».\n\
             title = \"{}\"\nwidth = {}\nheight = {}\ntick_rate = {}\nbackground = \"#{r:02x}{g:02x}{b:02x}\"\n\
             target = \"{}\"\nkind = \"{}\"\ndimension = \"{}\"\nlabel = \"{}\"\n",
            self.title.replace('"', "'"),
            self.width,
            self.height,
            self.tick_rate,
            self.target,
            self.kind,
            self.dimension,
            self.label.replace('"', "'")
        )
    }

    pub fn load(dir: &Path) -> (Self, Vec<String>) {
        match fs::read_to_string(dir.join(Self::FILE)) {
            Ok(t) => Self::parse(&t),
            Err(_) => (Self::default(), Vec::new()),
        }
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::write(dir.join(Self::FILE), self.to_toml()).map_err(|e| format!("{}: {e}", Self::FILE))
    }

    pub fn clear_color(&self) -> [f64; 4] {
        let lin = |c: u8| {
            let c = c as f64 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        [lin(self.background[0]), lin(self.background[1]), lin(self.background[2]), 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = ProjectSettings {
            title: "Моя игра".into(),
            width: 1920,
            height: 1080,
            tick_rate: 120,
            background: [1, 2, 255],
            target: "windows".into(),
            kind: "app".into(),
            dimension: "custom".into(),
            label: "Новелла".into(),
        };
        let (back, warn) = ProjectSettings::parse(&s.to_toml());
        assert_eq!(back, s);
        assert!(warn.is_empty());
    }

    #[test]
    fn warns_but_keeps_defaults() {
        let (s, warn) = ProjectSettings::parse("widht = 5\nwidth = 10\nbackground = red\n# коммент\n");
        assert_eq!(s, ProjectSettings::default());
        assert_eq!(warn.len(), 3);
    }

    #[test]
    fn rejects_bad_kind() {
        let (s, warn) = ProjectSettings::parse("kind = tool\ndimension = 4d\n");
        assert_eq!(s.kind, "game");
        assert_eq!(s.dimension, "2d");
        assert_eq!(warn.len(), 2);
    }
}

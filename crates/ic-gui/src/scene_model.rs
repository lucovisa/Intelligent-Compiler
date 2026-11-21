use super::*;

pub const SCENE_FILE: &str = "main.ic-scene";
const MAX_UNDO: usize = 200;
// Кадр камеры по умолчанию: 1280×720 с началом координат в центре, ось Y вверх
pub const FRAME_W: f32 = 1280.0;
pub const FRAME_H: f32 = 720.0;

#[derive(Clone, PartialEq, Debug)]
pub struct SceneNode {
    pub kind: String,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub color: [u8; 3],
    pub alpha: u8,
    pub visible: bool,
    pub locked: bool,
    // Картинка из проекта (путь вида assets/hero.png), текст и размер шрифта
    pub texture: String,
    pub text: String,
    pub font_size: f32,
}

// Рисуются ли узлы этого вида в игре (движок умеет только цветные прямоугольники)
pub fn is_visual(kind: &str) -> bool {
    !matches!(
        kind,
        "Node"
            | "Node2D"
            | "Marker2D"
            | "Camera2D"
            | "Timer"
            | "AudioStreamPlayer"
            | "AudioStreamPlayer2D"
            | "Path2D"
            | "AnimationPlayer"
            | "NavigationRegion2D"
            | "CollisionShape2D"
            | "RayCast2D"
            | "CanvasLayer"
            | "Control"
            | "ParallaxBackground"
            | "TileMap"
    )
}

#[derive(Clone, PartialEq)]
pub enum Drag {
    Move {
        start: (f32, f32),
        origs: Vec<(usize, f32, f32)>,
    },
    Resize {
        index: usize,
        handle: u8,
        orig: (f32, f32, f32, f32),
    },
    Marquee {
        start: (f32, f32),
        additive: bool,
    },
    Pan,
}

pub struct SceneEditor {
    pub open: bool,
    pub path: Option<PathBuf>,
    pub nodes: Vec<SceneNode>,
    pub selected: Option<usize>,
    pub extra: Vec<usize>,
    pub clipboard: Vec<SceneNode>,
    pub focus_request: bool,
    pub tree_drag: Option<usize>,
    pub module_drag: Option<Generator>,
    pub canvas_rect: Option<egui::Rect>,
    pub project_dir: Option<PathBuf>,
    pub pngs: Vec<String>,
    pub textures: HashMap<String, Option<egui::TextureHandle>>,
    pub dirty: bool,
    pub status: Option<String>,
    pub drag: Option<Drag>,
    pub pan: egui::Vec2,
    pub zoom: f32,
    pub fitted: bool,
    pub snap: bool,
    pub grid: f32,
    pub renaming: Option<usize>,
    pub rename_buf: String,
    undo: Vec<Vec<SceneNode>>,
    redo: Vec<Vec<SceneNode>>,
    frame_start: Vec<SceneNode>,
    gesture_base: Option<Vec<SceneNode>>,
}

impl Default for SceneEditor {
    fn default() -> Self {
        Self {
            open: false,
            path: None,
            nodes: Vec::new(),
            selected: None,
            extra: Vec::new(),
            clipboard: Vec::new(),
            focus_request: false,
            tree_drag: None,
            module_drag: None,
            canvas_rect: None,
            project_dir: None,
            pngs: Vec::new(),
            textures: HashMap::new(),
            dirty: false,
            status: None,
            drag: None,
            pan: egui::Vec2::ZERO,
            zoom: 0.6,
            fitted: false,
            snap: true,
            grid: 16.0,
            renaming: None,
            rename_buf: String::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            frame_start: Vec::new(),
            gesture_base: None,
        }
    }
}

fn fmt(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.to_string()
    }
}

// Движок рисует цвета как линейные, редактор показывает sRGB
fn srgb_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn escape(s: &str) -> String {
    s.replace('%', "%25")
        .replace('|', "%7C")
        .replace('\n', "%0A")
}

fn unescape(s: &str) -> String {
    s.replace("%0A", "\n")
        .replace("%7C", "|")
        .replace("%25", "%")
}

fn lua_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// Файл сцены → узлы; старый формат (угол холста, Y вниз) переводится в мировые координаты
pub fn parse_scene(text: &str) -> Vec<SceneNode> {
    let mut nodes = Vec::new();
    let v2 = text.lines().any(|l| l.trim().starts_with("ic-scene|"));
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.first() != Some(&"node") {
            continue;
        }
        if v2 && (parts.len() == 12 || parts.len() == 14 || parts.len() == 17) {
            let f = |i: usize| parts[i].parse::<f32>().ok();
            let u = |i: usize| parts[i].parse::<u8>().ok();
            if let (
                Some(x),
                Some(y),
                Some(w),
                Some(h),
                Some(r),
                Some(g),
                Some(b),
                Some(rot),
                Some(a),
            ) = (f(3), f(4), f(5), f(6), u(7), u(8), u(9), f(10), u(11))
            {
                nodes.push(SceneNode {
                    kind: parts[1].to_string(),
                    name: parts[2].replace("%7C", "|"),
                    x,
                    y,
                    width: w,
                    height: h,
                    rotation: rot,
                    color: [r, g, b],
                    alpha: a,
                    visible: parts.get(12).map_or(true, |v| *v != "0"),
                    locked: parts.get(13).is_some_and(|v| *v == "1"),
                    texture: parts.get(14).map(|v| unescape(v)).unwrap_or_default(),
                    text: parts.get(15).map(|v| unescape(v)).unwrap_or_default(),
                    font_size: parts.get(16).and_then(|v| v.parse().ok()).unwrap_or(24.0),
                });
            }
        } else if !v2 && (parts.len() == 9 || parts.len() == 10) {
            let offset = usize::from(parts.len() == 10);
            let f = |i: usize| parts[i + offset].parse::<f32>().ok();
            let u = |i: usize| parts[i + offset].parse::<u8>().ok();
            if let (Some(x), Some(y), Some(w), Some(h), Some(r), Some(g), Some(b)) =
                (f(2), f(3), f(4), f(5), u(6), u(7), u(8))
            {
                let kind = if offset == 1 && parts[1] != "Node2D" {
                    parts[1].to_string()
                } else {
                    "ColorRect".to_string()
                };
                nodes.push(SceneNode {
                    kind,
                    name: parts[1 + offset].replace("%7C", "|"),
                    x: x + w / 2.0 - FRAME_W / 2.0,
                    y: FRAME_H / 2.0 - (y + h / 2.0),
                    width: w,
                    height: h,
                    rotation: 0.0,
                    color: [r, g, b],
                    alpha: 255,
                    visible: true,
                    locked: false,
                    texture: String::new(),
                    text: String::new(),
                    font_size: 24.0,
                });
            }
        }
    }
    nodes
}

pub fn scene_text(nodes: &[SceneNode]) -> String {
    let mut out = String::from("ic-scene|4\n");
    for n in nodes {
        out.push_str(&format!(
            "node|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
            n.kind,
            n.name.replace('|', "%7C").replace('\n', " "),
            fmt(n.x),
            fmt(n.y),
            fmt(n.width),
            fmt(n.height),
            n.color[0],
            n.color[1],
            n.color[2],
            fmt(n.rotation),
            n.alpha,
            u8::from(n.visible),
            u8::from(n.locked),
            escape(&n.texture),
            escape(&n.text),
            fmt(n.font_size)
        ));
    }
    out
}

// Lua-модуль со сценой: его читает scene.load в игре
pub fn scene_lua(nodes: &[SceneNode]) -> String {
    let mut out = String::from("return {\n");
    for n in nodes {
        out.push_str(&format!(
            "    {{ kind = {}, name = {}, x = {}, y = {}, w = {}, h = {}, rot = {}, r = {}, g = {}, b = {}, a = {}, visual = {}, tex = {}, text = {}, size = {} }},\n",
            lua_string(&n.kind),
            lua_string(&n.name),
            fmt(n.x),
            fmt(n.y),
            fmt(n.width),
            fmt(n.height),
            fmt(n.rotation.to_radians()),
            fmt(srgb_to_linear(n.color[0])),
            fmt(srgb_to_linear(n.color[1])),
            fmt(srgb_to_linear(n.color[2])),
            fmt(n.alpha as f32 / 255.0),
            is_visual(&n.kind) && n.visible,
            lua_string(&n.texture),
            lua_string(&n.text),
            fmt(n.font_size)
        ));
    }
    out.push_str("}\n");
    out
}

const LOADER: &str = include_str!("../../ic-build/templates/scene.lua");

// Все PNG проекта (до глубины 4), пути с «/»
pub fn list_pngs(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, depth: u32, out: &mut Vec<String>) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if depth < 4 && !matches!(name.as_str(), "build" | "target" | ".ic" | ".git") {
                    walk(root, &p, depth + 1, out);
                }
            } else if name.to_ascii_lowercase().ends_with(".png") {
                if let Ok(rel) = p.strip_prefix(root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, 0, &mut out);
    out.sort();
    out
}

// Загрузчик первой версии (без картинок и текста): его заменяем на новый целиком
const OLD_LOADER: &str = include_str!("old_scene_loader.lua");

// В scene.lua должен быть scene.load; у старых проектов дописываем или обновляем
fn ensure_loader(project: &Path) -> Result<(), String> {
    let path = project.join("scene.lua");
    if !project_path_is_safe(project, &path) && path.exists() {
        return Err("scene.lua вне проекта".into());
    }
    let start = LOADER
        .find("function scene.load")
        .ok_or("шаблон scene.lua повреждён")?;
    let end = LOADER
        .rfind("return scene")
        .ok_or("шаблон scene.lua повреждён")?;
    let snippet = &LOADER[start..end];
    match fs::read_to_string(&path) {
        Ok(text) if text.contains("function level.clicked") => Ok(()),
        Ok(text) if text.contains(OLD_LOADER.trim_end()) => fs::write(
            &path,
            text.replace(OLD_LOADER.trim_end(), snippet.trim_end()),
        )
        .map_err(|e| e.to_string()),
        Ok(text) if text.contains("function scene.load") => {
            Err("scene.lua: свой scene.load не обновлён — удали его и сохрани сцену ещё раз".into())
        }
        Ok(text) => {
            let patched = match text.rfind("return scene") {
                Some(i) => format!("{}{}{}", &text[..i], snippet, &text[i..]),
                None => format!("{text}\n{snippet}"),
            };
            fs::write(&path, patched).map_err(|e| e.to_string())
        }
        Err(_) => fs::write(&path, ic_build::scaffold::SCENE).map_err(|e| e.to_string()),
    }
}

impl SceneEditor {
    pub fn open_project(&mut self, project_dir: &Path) {
        let dir = project_dir.join("scenes");
        if dir.exists() && (!dir.is_dir() || !project_path_is_safe(project_dir, &dir)) {
            self.open = false;
            self.path = None;
            self.status = Some("Scene folder is outside the project".into());
            return;
        }
        let path = dir.join(SCENE_FILE);
        self.nodes = fs::read_to_string(&path)
            .map(|t| parse_scene(&t))
            .unwrap_or_default();
        self.clear_selection();
        self.clipboard.clear();
        self.dirty = false;
        self.status = None;
        self.drag = None;
        self.renaming = None;
        self.undo.clear();
        self.redo.clear();
        self.gesture_base = None;
        self.fitted = false;
        self.path = Some(path);
        self.project_dir = Some(project_dir.to_path_buf());
        self.refresh_pngs();
    }

    // PNG проекта для выбора текстуры
    pub fn refresh_pngs(&mut self) {
        self.textures.clear();
        self.pngs = self
            .project_dir
            .as_deref()
            .map(list_pngs)
            .unwrap_or_default();
    }

    // Добавляет спрайт с текстурой (перетаскивание PNG на сцену)
    pub fn add_sprite_with_texture(&mut self, texture: &str, size: (f32, f32)) {
        self.add_node_kind("Sprite2D");
        if let Some(n) = self.nodes.last_mut() {
            n.texture = texture.to_string();
            n.width = size.0;
            n.height = size.1;
        }
        self.finish_change();
    }

    pub fn save(&mut self) -> Result<(), String> {
        let Some(path) = self.path.clone() else {
            return Err("Scene path is not set".into());
        };
        let project = path.parent().and_then(Path::parent).map(Path::to_path_buf);
        // Сначала папка сцен, потом сам файл: путь должен оставаться внутри проекта
        if let (Some(project), Some(parent)) = (&project, path.parent()) {
            if !project_path_is_safe(project, parent) {
                return Err("Scene path is outside the project".into());
            }
            fs::create_dir_all(parent).map_err(|e| format!("scene save error: {e}"))?;
            if !project_path_is_safe(project, &path) {
                return Err("Scene path is outside the project".into());
            }
        }
        fs::write(&path, scene_text(&self.nodes)).map_err(|e| format!("scene save error: {e}"))?;
        let lua = path.with_extension("lua");
        fs::write(&lua, scene_lua(&self.nodes)).map_err(|e| format!("scene save error: {e}"))?;
        if let Some(project) = &project {
            ensure_loader(project)?;
        }
        self.dirty = false;
        Ok(())
    }

    pub fn unique_name(&self, base: &str, skip: Option<usize>) -> String {
        let taken = |name: &str| {
            self.nodes
                .iter()
                .enumerate()
                .any(|(i, n)| Some(i) != skip && n.name == name)
        };
        if !taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|i| format!("{base}{i}"))
            .find(|n| !taken(n))
            .unwrap_or_default()
    }

    pub fn add_node_kind(&mut self, kind: &str) {
        self.checkpoint();
        let (width, height, color) = scene_ui::node_defaults(kind);
        let n = self.nodes.len() as f32;
        let name = self.unique_name(kind, None);
        self.nodes.push(SceneNode {
            kind: kind.to_string(),
            name,
            x: (n * 112.0).rem_euclid(560.0) - 280.0,
            y: 120.0 - (n * 80.0).rem_euclid(240.0),
            width,
            height,
            rotation: 0.0,
            color,
            alpha: 255,
            visible: true,
            locked: false,
            texture: String::new(),
            text: match kind {
                "Label" => "Label".to_string(),
                "Button" => "Button".to_string(),
                _ => String::new(),
            },
            font_size: 24.0,
        });
        self.set_selection(&[self.nodes.len() - 1]);
        self.finish_change();
    }

    pub fn rename(&mut self, index: usize, new: &str) {
        let clean: String = new
            .chars()
            .filter(|c| !c.is_control() && *c != '|')
            .collect();
        let clean = clean.trim();
        if clean.is_empty() || index >= self.nodes.len() || self.nodes[index].name == clean {
            return;
        }
        self.checkpoint();
        let name = self.unique_name(clean, Some(index));
        self.nodes[index].name = name;
        self.finish_change();
    }

    pub fn start_rename(&mut self, index: usize) {
        if let Some(n) = self.nodes.get(index) {
            self.rename_buf = n.name.clone();
            self.renaming = Some(index);
        }
    }

    // Все выделенные узлы по возрастанию индекса
    pub fn all_selected(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .selected
            .into_iter()
            .chain(self.extra.iter().copied())
            .filter(|i| *i < self.nodes.len())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    pub fn is_selected(&self, i: usize) -> bool {
        self.selected == Some(i) || self.extra.contains(&i)
    }

    pub fn set_selection(&mut self, items: &[usize]) {
        self.selected = items.last().copied();
        self.extra = items
            .iter()
            .copied()
            .take(items.len().saturating_sub(1))
            .collect();
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.extra.clear();
    }

    pub fn toggle_select(&mut self, i: usize) {
        let mut all = self.all_selected();
        if let Some(pos) = all.iter().position(|x| *x == i) {
            all.remove(pos);
        } else {
            all.push(i);
        }
        self.set_selection(&all);
    }

    pub fn select_all(&mut self) {
        let all: Vec<usize> = (0..self.nodes.len()).collect();
        self.set_selection(&all);
    }

    pub fn copy_selected(&mut self) {
        self.clipboard = self
            .all_selected()
            .iter()
            .map(|i| self.nodes[*i].clone())
            .collect();
    }

    // Вставка копий со смещением и новыми именами
    pub fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        self.checkpoint();
        let mut added = Vec::new();
        for mut node in self.clipboard.clone() {
            node.name = self.unique_name(&node.name, None);
            node.x += 16.0;
            node.y -= 16.0;
            self.nodes.push(node);
            added.push(self.nodes.len() - 1);
        }
        self.clipboard = added.iter().map(|i| self.nodes[*i].clone()).collect();
        self.set_selection(&added);
        self.finish_change();
    }

    pub fn duplicate_selected(&mut self) {
        let all = self.all_selected();
        if all.is_empty() {
            return;
        }
        self.checkpoint();
        let mut added = Vec::new();
        for i in all {
            let mut copy = self.nodes[i].clone();
            copy.name = self.unique_name(&copy.name, None);
            copy.x += 16.0;
            copy.y -= 16.0;
            self.nodes.push(copy);
            added.push(self.nodes.len() - 1);
        }
        self.set_selection(&added);
        self.finish_change();
    }

    pub fn delete_selected(&mut self) {
        let all = self.all_selected();
        if all.is_empty() {
            return;
        }
        self.checkpoint();
        for i in all.into_iter().rev() {
            self.nodes.remove(i);
        }
        self.clear_selection();
        self.finish_change();
    }

    // Перетаскивание строки дерева: узел переезжает на новое место списка (слой)
    pub fn move_node(&mut self, from: usize, to: usize) {
        if from >= self.nodes.len() || to >= self.nodes.len() || from == to {
            return;
        }
        self.checkpoint();
        let node = self.nodes.remove(from);
        self.nodes.insert(to, node);
        self.set_selection(&[to]);
        self.finish_change();
    }

    // Границы выделенных узлов в мире: (мин x, мин y, макс x, макс y)
    pub fn selection_bounds(&self) -> Option<(f32, f32, f32, f32)> {
        let mut b: Option<(f32, f32, f32, f32)> = None;
        for i in self.all_selected() {
            let n = &self.nodes[i];
            let (s, c) = n.rotation.to_radians().sin_cos();
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let (lx, ly) = (sx * n.width / 2.0, sy * n.height / 2.0);
                let (x, y) = (n.x + lx * c - ly * s, n.y + lx * s + ly * c);
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((a0, a1, a2, a3)) => (a0.min(x), a1.min(y), a2.max(x), a3.max(y)),
                });
            }
        }
        b
    }

    pub fn move_layer(&mut self, delta: isize) {
        let Some(i) = self.selected.filter(|i| *i < self.nodes.len()) else {
            return;
        };
        let j = i as isize + delta;
        if j < 0 || j as usize >= self.nodes.len() {
            return;
        }
        self.checkpoint();
        self.nodes.swap(i, j as usize);
        self.set_selection(&[j as usize]);
        self.finish_change();
    }

    // Явная команда (кнопка, меню): отдельный шаг отмены
    fn checkpoint(&mut self) {
        self.flush_gesture();
        self.undo.push(self.nodes.clone());
        self.undo.truncate_front(MAX_UNDO);
        self.redo.clear();
    }

    fn finish_change(&mut self) {
        self.frame_start = self.nodes.clone();
        self.dirty = true;
    }

    fn flush_gesture(&mut self) {
        if let Some(base) = self.gesture_base.take() {
            if base != self.nodes {
                self.undo.push(base);
                self.undo.truncate_front(MAX_UNDO);
                self.redo.clear();
            }
        }
    }

    // Начало кадра: запоминаем состояние, чтобы заметить правки из интерфейса
    pub fn begin_frame(&mut self) {
        self.frame_start = self.nodes.clone();
    }

    // Конец кадра: жест (перетаскивание, набор текста) = один шаг отмены
    pub fn end_frame(&mut self, busy: bool) {
        if self.nodes != self.frame_start {
            if self.gesture_base.is_none() {
                self.gesture_base = Some(self.frame_start.clone());
            }
            self.dirty = true;
        }
        if !busy {
            self.flush_gesture();
        }
    }

    pub fn undo(&mut self) {
        self.flush_gesture();
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.nodes, prev));
            self.after_history();
        }
    }

    pub fn redo(&mut self) {
        self.flush_gesture();
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.nodes, next));
            self.after_history();
        }
    }

    fn after_history(&mut self) {
        if self.all_selected().len() != usize::from(self.selected.is_some()) + self.extra.len()
            || self.selected.is_some_and(|i| i >= self.nodes.len())
        {
            self.clear_selection();
        }
        self.finish_change();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.gesture_base.is_some()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

trait TruncateFront {
    fn truncate_front(&mut self, max: usize);
}

impl<T> TruncateFront for Vec<T> {
    fn truncate_front(&mut self, max: usize) {
        if self.len() > max {
            self.drain(..self.len() - max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> SceneNode {
        SceneNode {
            kind: "Sprite2D".into(),
            name: name.into(),
            x: 10.5,
            y: -20.0,
            width: 64.0,
            height: 32.0,
            rotation: 90.0,
            color: [255, 128, 0],
            alpha: 200,
            visible: true,
            locked: false,
            texture: "assets/a|b.png".into(),
            text: "Привет|мир\nстрока".into(),
            font_size: 18.0,
        }
    }

    #[test]
    fn text_roundtrip_keeps_nodes() {
        let nodes = vec![node("A|B"), node("C")];
        assert_eq!(parse_scene(&scene_text(&nodes)), nodes);
    }

    #[test]
    fn legacy_scene_is_converted_to_world_coordinates() {
        let nodes = parse_scene("node|Sprite|60.00|60.00|88.00|88.00|10|20|30\n");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kind, "ColorRect");
        assert_eq!(nodes[0].x, 60.0 + 44.0 - 640.0);
        assert_eq!(nodes[0].y, 360.0 - 104.0);
    }

    #[test]
    fn lua_module_has_world_data() {
        let lua = scene_lua(&[node("Hero")]);
        assert!(lua.starts_with("return {"));
        assert!(lua.contains("name = \"Hero\""));
        assert!(lua.contains("rot = 1.571"));
        assert!(lua.contains("r = 1,"));
        assert!(lua.contains("g = 0.216"));
        assert!(lua.contains("visual = true"));
        assert!(lua.contains("tex = \"assets/a|b.png\""));
        assert!(lua.contains("size = 18"));
        assert!(!lua.contains("--"));
    }

    #[test]
    fn names_stay_unique_and_clean() {
        let mut s = SceneEditor::default();
        s.add_node_kind("Label");
        s.add_node_kind("Label");
        assert_eq!(s.nodes[1].name, "Label2");
        s.rename(1, "Label");
        assert_eq!(s.nodes[1].name, "Label2");
        s.rename(1, "Hero|x");
        assert_eq!(s.nodes[1].name, "Herox");
    }

    #[test]
    fn undo_redo_follow_commands() {
        let mut s = SceneEditor::default();
        s.add_node_kind("Label");
        s.add_node_kind("Button");
        assert_eq!(s.nodes.len(), 2);
        s.undo();
        assert_eq!(s.nodes.len(), 1);
        s.redo();
        assert_eq!(s.nodes.len(), 2);
        s.selected = Some(1);
        s.delete_selected();
        assert_eq!(s.nodes.len(), 1);
        s.undo();
        assert_eq!(s.nodes.len(), 2);
    }

    #[test]
    fn gesture_edits_become_one_undo_step() {
        let mut s = SceneEditor::default();
        s.add_node_kind("Label");
        for step in 1..=5 {
            s.begin_frame();
            s.nodes[0].x = step as f32 * 10.0;
            s.end_frame(true);
        }
        s.begin_frame();
        s.end_frame(false);
        assert_eq!(s.nodes[0].x, 50.0);
        s.undo();
        assert_ne!(s.nodes[0].x, 50.0);
        s.redo();
        assert_eq!(s.nodes[0].x, 50.0);
    }

    #[test]
    fn duplicate_and_layers() {
        let mut s = SceneEditor::default();
        s.add_node_kind("Label");
        s.add_node_kind("Button");
        s.set_selection(&[0]);
        s.duplicate_selected();
        assert_eq!(s.nodes[2].name, "Label2");
        assert_eq!(s.selected, Some(2));
        s.move_layer(-1);
        assert_eq!(s.selected, Some(1));
        assert_eq!(s.nodes[1].name, "Label2");
    }

    #[test]
    fn multi_select_copy_paste_delete() {
        let mut s = SceneEditor::default();
        for k in ["Label", "Button", "Timer"] {
            s.add_node_kind(k);
        }
        s.select_all();
        assert_eq!(s.all_selected(), vec![0, 1, 2]);
        s.toggle_select(1);
        assert_eq!(s.all_selected(), vec![0, 2]);
        s.copy_selected();
        s.paste();
        assert_eq!(s.nodes.len(), 5);
        assert_eq!(s.nodes[3].name, "Label2");
        assert_eq!(s.nodes[4].name, "Timer2");
        assert_eq!(s.all_selected(), vec![3, 4]);
        s.delete_selected();
        assert_eq!(s.nodes.len(), 3);
        assert!(s.all_selected().is_empty());
        s.undo();
        assert_eq!(s.nodes.len(), 5);
    }

    #[test]
    fn reorder_layers_by_drag() {
        let mut s = SceneEditor::default();
        for k in ["Label", "Button", "Timer"] {
            s.add_node_kind(k);
        }
        s.move_node(0, 2);
        let names: Vec<&str> = s.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["Button", "Timer", "Label"]);
        assert_eq!(s.selected, Some(2));
    }

    #[test]
    fn hidden_and_locked_survive_roundtrip_and_hide_in_game() {
        let mut hidden = node("H");
        hidden.visible = false;
        hidden.locked = true;
        let nodes = vec![hidden];
        let back = parse_scene(&scene_text(&nodes));
        assert_eq!(back, nodes);
        assert!(scene_lua(&nodes).contains("visual = false"));
    }

    #[test]
    fn selection_bounds_cover_rotated_nodes() {
        let mut s = SceneEditor::default();
        s.add_node_kind("ColorRect");
        s.nodes[0].x = 0.0;
        s.nodes[0].y = 0.0;
        s.nodes[0].width = 100.0;
        s.nodes[0].height = 20.0;
        s.nodes[0].rotation = 90.0;
        let (x0, y0, x1, y1) = s.selection_bounds().unwrap();
        assert!((x0 + 10.0).abs() < 0.01 && (x1 - 10.0).abs() < 0.01);
        assert!((y0 + 50.0).abs() < 0.01 && (y1 - 50.0).abs() < 0.01);
    }

    #[test]
    fn save_writes_scene_lua_module_and_loader() {
        let dir = std::env::temp_dir().join(format!("ic_scene_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("scene.lua"), "local scene = {}\nreturn scene\n").unwrap();
        let mut s = SceneEditor::default();
        s.open_project(&dir);
        s.add_node_kind("ColorRect");
        s.save().unwrap();
        assert!(dir.join("scenes/main.ic-scene").is_file());
        assert!(fs::read_to_string(dir.join("scenes/main.lua"))
            .unwrap()
            .contains("ColorRect"));
        let loader = fs::read_to_string(dir.join("scene.lua")).unwrap();
        assert!(loader.contains("function scene.load"));
        assert!(loader.trim_end().ends_with("return scene"));
        let _ = fs::remove_dir_all(&dir);
    }
}

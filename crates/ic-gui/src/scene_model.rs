use super::*;

pub const MAIN_SCENE: &str = "main.ic";
const LEGACY_SCENE: &str = "scenes/main.ic-scene";
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
    // Родитель по имени (пусто — корень), гравитация и упругость для физики
    pub parent: String,
    pub gravity: f32,
    pub bounce: f32,
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
        if v2 && matches!(parts.len(), 12 | 14 | 17 | 20) {
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
                    parent: parts.get(17).map(|v| unescape(v)).unwrap_or_default(),
                    gravity: parts.get(18).and_then(|v| v.parse().ok()).unwrap_or(900.0),
                    bounce: parts.get(19).and_then(|v| v.parse().ok()).unwrap_or(0.0),
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
                    parent: String::new(),
                    gravity: 900.0,
                    bounce: 0.0,
                });
            }
        }
    }
    to_world(&mut nodes);
    nodes
}

// Позиция узла в мире по цепочке родителей (в файле позиции дочерних — относительные)
fn parent_of(nodes: &[SceneNode], i: usize) -> Option<usize> {
    let name = nodes[i].parent.as_str();
    if name.is_empty() {
        return None;
    }
    nodes
        .iter()
        .position(|n| n.name == name)
        .filter(|p| *p != i)
}

fn chain_offset(nodes: &[SceneNode], i: usize, local: &[(f32, f32)]) -> (f32, f32) {
    let (mut x, mut y) = (0.0, 0.0);
    let mut cur = parent_of(nodes, i);
    let mut guard = 0;
    while let Some(p) = cur {
        x += local[p].0;
        y += local[p].1;
        guard += 1;
        if guard > nodes.len() {
            break;
        }
        cur = parent_of(nodes, p);
    }
    (x, y)
}

// Файл хранит относительные позиции, редактор работает с мировыми
fn to_world(nodes: &mut [SceneNode]) {
    let local: Vec<(f32, f32)> = nodes.iter().map(|n| (n.x, n.y)).collect();
    for i in 0..nodes.len() {
        let (ox, oy) = chain_offset(nodes, i, &local);
        nodes[i].x = local[i].0 + ox;
        nodes[i].y = local[i].1 + oy;
    }
}

pub fn scene_text(nodes: &[SceneNode]) -> String {
    let world: Vec<(f32, f32)> = nodes.iter().map(|n| (n.x, n.y)).collect();
    let mut out = String::from("ic-scene|5\n");
    for (i, n) in nodes.iter().enumerate() {
        let (ox, oy) = parent_of(nodes, i).map_or((0.0, 0.0), |p| world[p]);
        out.push_str(&format!(
            "node|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
            n.kind,
            n.name.replace('|', "%7C").replace('\n', " "),
            fmt(n.x - ox),
            fmt(n.y - oy),
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
            fmt(n.font_size),
            escape(&n.parent),
            fmt(n.gravity),
            fmt(n.bounce)
        ));
    }
    out
}

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

impl SceneEditor {
    pub fn open_project(&mut self, project_dir: &Path) {
        self.open_scene(project_dir, &project_dir.join(MAIN_SCENE));
    }

    // Открывает .ic из проекта; старый scenes/main.ic-scene читается как main.ic
    pub fn open_scene(&mut self, project_dir: &Path, path: &Path) {
        if !project_path_is_safe(project_dir, path) {
            self.open = false;
            self.path = None;
            self.status = Some("Scene file is outside the project".into());
            return;
        }
        let is_main = path == project_dir.join(MAIN_SCENE);
        let text = fs::read_to_string(path).ok().or_else(|| {
            let legacy = project_dir.join(LEGACY_SCENE);
            if is_main && project_path_is_safe(project_dir, &legacy) {
                fs::read_to_string(legacy).ok()
            } else {
                None
            }
        });
        self.nodes = text.map(|t| parse_scene(&t)).unwrap_or_default();
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
        self.path = Some(path.to_path_buf());
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
        if let Some(project) = &self.project_dir {
            if !project_path_is_safe(project, &path) {
                return Err("Scene path is outside the project".into());
            }
        }
        fs::write(&path, scene_text(&self.nodes)).map_err(|e| format!("scene save error: {e}"))?;
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
        self.add_node_under(kind, None);
    }

    // Новый узел; с родителем он появляется рядом с ним и становится дочерним
    pub fn add_node_under(&mut self, kind: &str, parent: Option<usize>) {
        self.checkpoint();
        let (width, height, color) = scene_ui::node_defaults(kind);
        let n = self.nodes.len() as f32;
        let name = self.unique_name(kind, None);
        let (x, y, parent_name) = match parent.and_then(|p| self.nodes.get(p)) {
            Some(p) => (p.x + 24.0, p.y - 24.0, p.name.clone()),
            None => (
                (n * 112.0).rem_euclid(560.0) - 280.0,
                120.0 - (n * 80.0).rem_euclid(240.0),
                String::new(),
            ),
        };
        self.nodes.push(SceneNode {
            kind: kind.to_string(),
            name,
            x,
            y,
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
            parent: parent_name,
            gravity: 900.0,
            bounce: 0.0,
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
        let old_name = std::mem::replace(&mut self.nodes[index].name, name.clone());
        for n in &mut self.nodes {
            if n.parent == old_name {
                n.parent = name.clone();
            }
        }
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
        let mut renamed: HashMap<String, String> = HashMap::new();
        for mut node in self.clipboard.clone() {
            let old_name = node.name.clone();
            node.name = self.unique_name(&node.name, None);
            renamed.insert(old_name, node.name.clone());
            node.x += 16.0;
            node.y -= 16.0;
            if let Some(new_parent) = renamed.get(&node.parent) {
                node.parent = new_parent.clone();
            }
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
        let mut renamed: HashMap<String, String> = HashMap::new();
        for i in all {
            let mut copy = self.nodes[i].clone();
            let old_name = copy.name.clone();
            copy.name = self.unique_name(&copy.name, None);
            renamed.insert(old_name, copy.name.clone());
            copy.x += 16.0;
            copy.y -= 16.0;
            if let Some(new_parent) = renamed.get(&copy.parent) {
                copy.parent = new_parent.clone();
            }
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
        let mut doomed: Vec<usize> = all.clone();
        for i in all {
            doomed.extend(self.descendants(i));
        }
        doomed.sort_unstable();
        doomed.dedup();
        for i in doomed.into_iter().rev() {
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

    // Индекс родителя узла
    pub fn parent_index(&self, i: usize) -> Option<usize> {
        if i >= self.nodes.len() {
            return None;
        }
        parent_of(&self.nodes, i)
    }

    // Прямые дочерние узлы
    pub fn children_of(&self, i: usize) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|c| self.parent_index(*c) == Some(i))
            .collect()
    }

    // Все потомки узла любой глубины
    pub fn descendants(&self, i: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = self.children_of(i);
        while let Some(c) = stack.pop() {
            if c != i && !out.contains(&c) {
                out.push(c);
                stack.extend(self.children_of(c));
            }
        }
        out
    }

    // Глубина узла в дереве (корень — 0)
    pub fn depth(&self, i: usize) -> usize {
        let mut d = 0;
        let mut cur = self.parent_index(i);
        while let Some(p) = cur {
            d += 1;
            if d > self.nodes.len() {
                break;
            }
            cur = self.parent_index(p);
        }
        d
    }

    // Порядок строк дерева: родитель, затем его дети (индекс, глубина)
    pub fn tree_order(&self) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = Vec::new();
        let mut seen = vec![false; self.nodes.len()];
        fn visit(
            ed: &SceneEditor,
            i: usize,
            depth: usize,
            seen: &mut [bool],
            out: &mut Vec<(usize, usize)>,
        ) {
            if seen[i] {
                return;
            }
            seen[i] = true;
            out.push((i, depth));
            for c in ed.children_of(i) {
                visit(ed, c, depth + 1, seen, out);
            }
        }
        for i in 0..self.nodes.len() {
            if self.parent_index(i).is_none() {
                visit(self, i, 0, &mut seen, &mut out);
            }
        }
        for i in 0..self.nodes.len() {
            visit(self, i, 0, &mut seen, &mut out);
        }
        out
    }

    // Новый родитель; позиция в мире не меняется. false — нельзя (цикл)
    pub fn set_parent(&mut self, i: usize, parent: Option<usize>) -> bool {
        if i >= self.nodes.len() {
            return false;
        }
        let name = match parent {
            Some(p) => {
                if p >= self.nodes.len() || p == i || self.descendants(i).contains(&p) {
                    return false;
                }
                self.nodes[p].name.clone()
            }
            None => String::new(),
        };
        if self.nodes[i].parent == name {
            return true;
        }
        self.checkpoint();
        self.nodes[i].parent = name;
        self.finish_change();
        true
    }

    // Перенос строки дерева: zone 0 — перед целью, 1 — внутрь (дочерним), 2 — после цели
    pub fn drop_on(&mut self, from: usize, target: usize, zone: u8) {
        if from >= self.nodes.len() || target >= self.nodes.len() || from == target {
            return;
        }
        if self.descendants(from).contains(&target) {
            return;
        }
        if zone == 1 {
            self.set_parent(from, Some(target));
            return;
        }
        self.checkpoint();
        let parent = self.nodes[target].parent.clone();
        let target_name = self.nodes[target].name.clone();
        let mut node = self.nodes.remove(from);
        node.parent = parent;
        let at = self
            .nodes
            .iter()
            .position(|n| n.name == target_name)
            .unwrap_or(0);
        let at = if zone == 0 { at } else { at + 1 };
        self.nodes.insert(at, node);
        self.set_selection(&[at]);
        self.finish_change();
    }

    // Сдвиг узла за кадр переносит и его потомков, которые сами не двигались
    fn carry_children(&mut self) {
        if self.frame_start.len() != self.nodes.len() {
            return;
        }
        let deltas: Vec<(f32, f32)> = self
            .nodes
            .iter()
            .zip(&self.frame_start)
            .map(|(a, b)| (a.x - b.x, a.y - b.y))
            .collect();
        // Сверху вниз: сам двигался — свой сдвиг, иначе сдвиг ближайшего родителя
        let mut order: Vec<usize> = (0..self.nodes.len()).collect();
        order.sort_by_key(|i| self.depth(*i));
        let mut eff = vec![(0.0_f32, 0.0_f32); self.nodes.len()];
        for i in order {
            if deltas[i] != (0.0, 0.0) {
                eff[i] = deltas[i];
            } else if let Some(p) = self.parent_index(i) {
                eff[i] = eff[p];
                self.nodes[i].x += eff[i].0;
                self.nodes[i].y += eff[i].1;
            }
        }
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
        self.carry_children();
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
            parent: String::new(),
            gravity: 900.0,
            bounce: 0.0,
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
    fn hidden_and_locked_survive_roundtrip() {
        let mut hidden = node("H");
        hidden.visible = false;
        hidden.locked = true;
        let nodes = vec![hidden];
        let back = parse_scene(&scene_text(&nodes));
        assert_eq!(back, nodes);
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

    fn tmp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ic_scene_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_writes_only_main_ic() {
        let dir = tmp_project("save");
        let mut s = SceneEditor::default();
        s.open_project(&dir);
        s.add_node_kind("ColorRect");
        s.save().unwrap();
        assert!(fs::read_to_string(dir.join("main.ic"))
            .unwrap()
            .contains("ColorRect"));
        assert!(!dir.join("scenes").exists());
        assert!(!dir.join("scene.lua").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_scene_file_is_still_read() {
        let dir = tmp_project("legacy");
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::write(dir.join("scenes/main.ic-scene"), scene_text(&[node("Old")])).unwrap();
        let mut s = SceneEditor::default();
        s.open_project(&dir);
        assert_eq!(s.nodes.len(), 1);
        assert_eq!(s.path, Some(dir.join("main.ic")));
        let _ = fs::remove_dir_all(&dir);
    }

    fn family() -> SceneEditor {
        let mut s = SceneEditor::default();
        let mut parent = node("Parent");
        parent.x = 100.0;
        parent.y = 50.0;
        let mut kid = node("Kid");
        kid.x = 110.0;
        kid.y = 40.0;
        kid.parent = "Parent".into();
        let mut grand = node("Grand");
        grand.x = 120.0;
        grand.y = 30.0;
        grand.parent = "Kid".into();
        s.nodes = vec![parent, kid, grand, node("Other")];
        s
    }

    #[test]
    fn file_keeps_relative_positions_and_editor_world_ones() {
        let s = family();
        let text = scene_text(&s.nodes);
        let kid_line = text
            .lines()
            .find(|l| l.starts_with("node|Sprite2D|Kid|"))
            .unwrap();
        assert!(kid_line.starts_with("node|Sprite2D|Kid|10|-10|"));
        let back = parse_scene(&text);
        assert_eq!(back, s.nodes);
    }

    #[test]
    fn tree_order_and_depth_follow_parents() {
        let s = family();
        assert_eq!(s.tree_order(), vec![(0, 0), (1, 1), (2, 2), (3, 0)]);
        assert_eq!(s.depth(2), 2);
        assert_eq!(s.descendants(0).len(), 2);
    }

    #[test]
    fn moving_a_parent_carries_children_once() {
        let mut s = family();
        s.begin_frame();
        s.nodes[0].x += 30.0;
        s.end_frame(false);
        assert_eq!((s.nodes[1].x, s.nodes[2].x), (140.0, 150.0));
        s.begin_frame();
        s.nodes[0].x += 5.0;
        s.nodes[1].x += 5.0;
        s.end_frame(false);
        assert_eq!((s.nodes[1].x, s.nodes[2].x), (145.0, 155.0));
    }

    #[test]
    fn reparenting_rejects_cycles_and_keeps_world_position() {
        let mut s = family();
        assert!(!s.set_parent(0, Some(2)));
        assert!(!s.set_parent(1, Some(1)));
        let before = (s.nodes[3].x, s.nodes[3].y);
        assert!(s.set_parent(3, Some(0)));
        assert_eq!(s.nodes[3].parent, "Parent");
        assert_eq!((s.nodes[3].x, s.nodes[3].y), before);
        assert!(s.set_parent(3, None));
        assert!(s.nodes[3].parent.is_empty());
    }

    #[test]
    fn delete_removes_subtree_and_rename_updates_children() {
        let mut s = family();
        s.rename(0, "Boss");
        assert_eq!(s.nodes[1].parent, "Boss");
        s.set_selection(&[0]);
        s.delete_selected();
        assert_eq!(s.nodes.len(), 1);
        assert_eq!(s.nodes[0].name, "Other");
    }

    #[test]
    fn dropping_on_a_row_makes_child_or_sibling() {
        let mut s = family();
        s.drop_on(3, 1, 1);
        assert_eq!(s.nodes[3].parent, "Kid");
        s.drop_on(3, 0, 0);
        assert!(
            s.nodes[s.nodes.iter().position(|n| n.name == "Other").unwrap()]
                .parent
                .is_empty()
        );
        assert_eq!(s.nodes[0].name, "Other");
    }

    #[test]
    fn duplicate_family_keeps_inner_links() {
        let mut s = family();
        s.set_selection(&[0, 1]);
        s.duplicate_selected();
        assert_eq!(s.nodes[5].parent, s.nodes[4].name);
    }
}

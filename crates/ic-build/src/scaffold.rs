use crate::ProjectSettings;
use std::{fs, path::Path};

pub const DEMO: &str = include_str!("../templates/demo.lua");
const SCENE: &str = include_str!("../templates/scene.lua");
const FPS: &str = include_str!("../templates/fps.lua");
const PHYSICS: &str = include_str!("../templates/physics.lua");
const CAMERA: &str = include_str!("../templates/camera.lua");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    App,
    Game,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::App => "app",
            Kind::Game => "game",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dimension {
    D2,
    D3,
    Custom,
}

impl Dimension {
    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::D2 => "2d",
            Dimension::D3 => "3d",
            Dimension::Custom => "custom",
        }
    }

    pub fn parse(s: &str) -> Option<Dimension> {
        match s {
            "2d" => Some(Dimension::D2),
            "3d" => Some(Dimension::D3),
            "custom" => Some(Dimension::Custom),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Generator {
    Scene,
    Fps,
    Physics,
    Camera,
}

impl Generator {
    pub const ALL: [Generator; 4] = [
        Generator::Scene,
        Generator::Fps,
        Generator::Physics,
        Generator::Camera,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Generator::Scene => "scene",
            Generator::Fps => "fps",
            Generator::Physics => "physics",
            Generator::Camera => "camera",
        }
    }

    pub fn file(self) -> &'static str {
        match self {
            Generator::Scene => "scene.lua",
            Generator::Fps => "fps.lua",
            Generator::Physics => "physics.lua",
            Generator::Camera => "camera.lua",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Generator::Scene => SCENE,
            Generator::Fps => FPS,
            Generator::Physics => PHYSICS,
            Generator::Camera => CAMERA,
        }
    }

    pub fn usage(self) -> &'static str {
        match self {
            Generator::Scene => "local scene = require(\"scene\")",
            Generator::Fps => "local fps = require(\"fps\")",
            Generator::Physics => "local physics = require(\"physics\")",
            Generator::Camera => "local camera = require(\"camera\")",
        }
    }

    pub fn parse(s: &str) -> Option<Generator> {
        Self::ALL.into_iter().find(|g| g.key() == s)
    }
}

#[derive(Clone, Debug)]
pub struct NewProject {
    pub kind: Kind,
    pub dimension: Dimension,
    pub label: String,
    pub generators: Vec<Generator>,
    pub default_code: bool,
}

impl Default for NewProject {
    fn default() -> Self {
        Self {
            kind: Kind::Game,
            dimension: Dimension::D2,
            label: String::new(),
            generators: Vec::new(),
            default_code: true,
        }
    }
}

pub fn create(dir: &Path, opts: &NewProject) -> Result<Vec<String>, String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut made = Vec::new();

    let main = dir.join("main.lua");
    if !main.exists() {
        let code = if opts.default_code { DEMO } else { "" };
        fs::write(&main, code).map_err(|e| format!("main.lua: {e}"))?;
        made.push("main.lua".to_string());
    }

    let toml = dir.join(ProjectSettings::FILE);
    if !toml.exists() {
        let title = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "IC Game".to_string());
        let settings = ProjectSettings {
            title,
            kind: opts.kind.as_str().into(),
            dimension: opts.dimension.as_str().into(),
            label: opts.label.clone(),
            ..Default::default()
        };
        settings.save(dir)?;
        made.push(ProjectSettings::FILE.to_string());
    }

    for g in &opts.generators {
        if let Ok(f) = add_generator(dir, *g) {
            made.push(f);
        }
    }
    Ok(made)
}

pub fn add_generator(dir: &Path, g: Generator) -> Result<String, String> {
    let path = dir.join(g.file());
    if path.exists() {
        return Err(format!("{} уже есть", g.file()));
    }
    fs::write(&path, g.code()).map_err(|e| format!("{}: {e}", g.file()))?;
    Ok(g.file().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("ic_scaffold_{name}_{}", std::process::id()))
    }

    #[test]
    fn game_with_default_code_and_generators() {
        let dir = tmp("game");
        let opts = NewProject {
            generators: vec![Generator::Fps, Generator::Physics],
            ..Default::default()
        };
        let made = create(&dir, &opts).unwrap();
        assert!(made.contains(&"main.lua".to_string()) && made.contains(&"ic.toml".to_string()));
        assert!(dir.join("fps.lua").is_file() && dir.join("physics.lua").is_file());
        assert!(!fs::read_to_string(dir.join("main.lua")).unwrap().is_empty());
        let (s, warn) = ProjectSettings::load(&dir);
        assert!(warn.is_empty());
        assert_eq!((s.kind.as_str(), s.dimension.as_str()), ("game", "2d"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn app_without_default_code_is_empty() {
        let dir = tmp("app");
        let opts = NewProject {
            kind: Kind::App,
            default_code: false,
            ..Default::default()
        };
        create(&dir, &opts).unwrap();
        assert!(fs::read_to_string(dir.join("main.lua")).unwrap().is_empty());
        assert!(!dir.join("fps.lua").exists());
        assert_eq!(ProjectSettings::load(&dir).0.kind, "app");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn generator_is_added_once() {
        let dir = tmp("gen");
        create(&dir, &NewProject::default()).unwrap();
        assert!(add_generator(&dir, Generator::Scene).is_ok());
        assert!(add_generator(&dir, Generator::Scene).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_files_are_not_overwritten() {
        let dir = tmp("keep");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.lua"), "-- mine").unwrap();
        create(&dir, &NewProject::default()).unwrap();
        assert_eq!(fs::read_to_string(dir.join("main.lua")).unwrap(), "-- mine");
        let _ = fs::remove_dir_all(&dir);
    }
}

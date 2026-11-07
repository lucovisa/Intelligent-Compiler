//! IC (Intelligent Compiler) — ядро движка. Без GPU/окна: легко тестируется в CI.
pub mod math;
pub use math::{Vec2, Vec3};

use std::time::{Duration, Instant};

pub struct Config {
    pub title: &'static str,
    /// Частота fixed_update (физика/логика), Гц.
    pub tick_rate: u32,
    /// Для тестов: остановиться после N кадров (headless).
    pub max_frames: Option<u64>,
    pub clear_color: [f64; 4],
}

impl Default for Config {
    fn default() -> Self {
        Self { title: "IC Game", tick_rate: 60, max_frames: None, clear_color: [0.02, 0.02, 0.05, 1.0] }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Time {
    pub delta: f32,
    pub fixed_delta: f32,
    pub elapsed: f32,
    pub frame: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key { Left, Right, Up, Down, W, A, S, D, Space, Escape, Enter }
pub const KEY_COUNT: usize = 11;

#[derive(Default)]
pub struct Input {
    down: [bool; KEY_COUNT],
    pressed: [bool; KEY_COUNT],
}

impl Input {
    pub fn is_down(&self, k: Key) -> bool { self.down[k as usize] }
    pub fn just_pressed(&self, k: Key) -> bool { self.pressed[k as usize] }
    pub fn set(&mut self, k: Key, is_down: bool) {
        let i = k as usize;
        if is_down && !self.down[i] { self.pressed[i] = true; }
        self.down[i] = is_down;
    }
    pub fn end_frame(&mut self) { self.pressed = [false; KEY_COUNT]; }
}

/// Цветной (пока без текстур) спрайт. Координаты — мировые, Y вверх.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
    pub color: [f32; 4],
}

#[derive(Default)]
pub struct DrawList {
    pub sprites: Vec<Sprite>,
}

impl DrawList {
    pub fn clear(&mut self) { self.sprites.clear(); }
    pub fn sprite(&mut self, s: Sprite) { self.sprites.push(s); }
    pub fn rect(&mut self, pos: Vec2, size: Vec2, color: [f32; 4]) {
        self.sprites.push(Sprite { pos, size, rotation: 0.0, color });
    }
}

/// `height` — сколько мировых единиц видно по вертикали.
pub struct Camera2D {
    pub center: Vec2,
    pub height: f32,
}

impl Default for Camera2D {
    fn default() -> Self { Self { center: Vec2::ZERO, height: 720.0 } }
}

#[derive(Default)]
pub struct Context {
    pub time: Time,
    pub input: Input,
    pub draw: DrawList,
    pub camera: Camera2D,
    pub screen_size: Vec2,
    quit: bool,
}

impl Context {
    pub fn quit(&mut self) { self.quit = true; }
}

/// Всё, что реализует пользователь движка.
pub trait Game {
    fn init(&mut self, _ctx: &mut Context) {}
    /// Постоянный шаг `time.fixed_delta`.
    fn fixed_update(&mut self, _ctx: &mut Context) {}
    /// Каждый кадр. Здесь же добавляй спрайты в `ctx.draw`.
    fn update(&mut self, ctx: &mut Context);
    fn shutdown(&mut self, _ctx: &mut Context) {}
}

/// Шаговая машина движка. Её используют и headless `run`, и оконный рендер.
pub struct Engine<G: Game> {
    pub game: G,
    pub ctx: Context,
    fixed: f32,
    acc: f32,
    start: Instant,
    last: Instant,
}

impl<G: Game> Engine<G> {
    pub fn new(game: G, tick_rate: u32) -> Self {
        let fixed = 1.0 / tick_rate.max(1) as f32;
        let mut ctx = Context::default();
        ctx.time.fixed_delta = fixed;
        ctx.screen_size = Vec2::new(1280.0, 720.0);
        let now = Instant::now();
        Self { game, ctx, fixed, acc: 0.0, start: now, last: now }
    }

    pub fn start(&mut self) {
        self.game.init(&mut self.ctx);
        let now = Instant::now();
        self.start = now;
        self.last = now;
    }

    pub fn frame(&mut self) {
        let now = Instant::now();
        // clamp защищает от «спирали смерти» после лагов
        let dt = (now - self.last).as_secs_f32().min(0.25);
        self.last = now;
        self.ctx.time.delta = dt;
        self.ctx.time.elapsed = (now - self.start).as_secs_f32();
        self.acc += dt;

        while self.acc >= self.fixed {
            self.game.fixed_update(&mut self.ctx);
            self.acc -= self.fixed;
        }

        self.ctx.draw.clear();
        self.game.update(&mut self.ctx);
        self.ctx.time.frame += 1;
        self.ctx.input.end_frame();
    }

    pub fn should_quit(&self) -> bool { self.ctx.quit }

    pub fn shutdown(&mut self) { self.game.shutdown(&mut self.ctx); }
}

/// Headless-цикл (без окна) — для тестов и серверов.
pub fn run<G: Game>(game: G, cfg: Config) {
    let mut e = Engine::new(game, cfg.tick_rate);
    e.start();
    while !e.should_quit() {
        e.frame();
        match cfg.max_frames {
            Some(max) if e.ctx.time.frame >= max => e.ctx.quit(),
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    }
    e.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter { frames: u32 }
    impl Game for Counter {
        fn update(&mut self, ctx: &mut Context) {
            self.frames += 1;
            ctx.draw.rect(Vec2::ZERO, Vec2::new(1.0, 1.0), [1.0; 4]);
        }
    }

    #[test]
    fn engine_counts_frames() {
        let mut e = Engine::new(Counter { frames: 0 }, 60);
        e.start();
        for _ in 0..10 { e.frame(); }
        assert_eq!(e.game.frames, 10);
        assert_eq!(e.ctx.draw.sprites.len(), 1); // draw очищается каждый кадр
    }

    #[test]
    fn input_just_pressed_lasts_one_frame() {
        let mut i = Input::default();
        i.set(Key::Space, true);
        assert!(i.just_pressed(Key::Space) && i.is_down(Key::Space));
        i.end_frame();
        assert!(!i.just_pressed(Key::Space) && i.is_down(Key::Space));
    }
}

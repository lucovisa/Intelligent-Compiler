//! Демо: 20 000 летающих квадратов + управляемый WASD игрок. Esc — выход.
use ic_render::{run, Config, Context, Game, Key, Sprite, Vec2};

const COUNT: usize = 20_000;

struct Rng(u64);
impl Rng {
    fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x >> 40) as f32) / 16_777_216.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    color: [f32; 4],
}

struct Demo {
    particles: Vec<Particle>,
    player: Vec2,
    fps_timer: f32,
    fps_frames: u32,
}

impl Game for Demo {
    fn init(&mut self, _ctx: &mut Context) {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..COUNT {
            self.particles.push(Particle {
                pos: Vec2::new(rng.range(-600.0, 600.0), rng.range(-340.0, 340.0)),
                vel: Vec2::new(rng.range(-120.0, 120.0), rng.range(-120.0, 120.0)),
                color: [rng.next_f32(), rng.next_f32(), rng.next_f32(), 0.85],
            });
        }
        println!("IC demo: {COUNT} спрайтов. WASD — двигать, Esc — выход.");
    }

    fn fixed_update(&mut self, ctx: &mut Context) {
        let dt = ctx.time.fixed_delta;
        let half_h = ctx.camera.height * 0.5;
        let half_w = half_h * (ctx.screen_size.x / ctx.screen_size.y.max(1.0));
        for p in &mut self.particles {
            p.pos += p.vel * dt;
            if p.pos.x.abs() > half_w {
                p.vel.x = -p.vel.x;
                p.pos.x = p.pos.x.clamp(-half_w, half_w);
            }
            if p.pos.y.abs() > half_h {
                p.vel.y = -p.vel.y;
                p.pos.y = p.pos.y.clamp(-half_h, half_h);
            }
        }
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.just_pressed(Key::Escape) {
            ctx.quit();
        }

        let mut dir = Vec2::ZERO;
        if ctx.input.is_down(Key::W) || ctx.input.is_down(Key::Up) {
            dir.y += 1.0;
        }
        if ctx.input.is_down(Key::S) || ctx.input.is_down(Key::Down) {
            dir.y -= 1.0;
        }
        if ctx.input.is_down(Key::D) || ctx.input.is_down(Key::Right) {
            dir.x += 1.0;
        }
        if ctx.input.is_down(Key::A) || ctx.input.is_down(Key::Left) {
            dir.x -= 1.0;
        }
        self.player += dir * (400.0 * ctx.time.delta);

        ctx.draw.sprites.reserve(COUNT + 1);
        for p in &self.particles {
            ctx.draw.sprite(Sprite {
                pos: p.pos,
                size: Vec2::new(6.0, 6.0),
                rotation: ctx.time.elapsed,
                color: p.color,
            });
        }
        ctx.draw.sprite(Sprite {
            pos: self.player,
            size: Vec2::new(40.0, 40.0),
            rotation: ctx.time.elapsed * 2.0,
            color: [1.0, 1.0, 1.0, 1.0],
        });

        self.fps_timer += ctx.time.delta;
        self.fps_frames += 1;
        if self.fps_timer >= 1.0 {
            println!("FPS: {}", self.fps_frames);
            self.fps_timer = 0.0;
            self.fps_frames = 0;
        }
    }
}

fn main() {
    run(
        Demo {
            particles: Vec::with_capacity(COUNT),
            player: Vec2::ZERO,
            fps_timer: 0.0,
            fps_frames: 0,
        },
        Config {
            title: "IC Demo",
            ..Default::default()
        },
    );
}

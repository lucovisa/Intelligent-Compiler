//! Окно + GPU (winit + wgpu) поверх ic-core.
//! Пользователю достаточно одной зависимости: `ic-render` реэкспортирует всё ядро.
pub use ic_core::*;

mod renderer;

use renderer::Renderer;
use std::sync::Arc;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

fn map_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyD => Key::D,
        KeyCode::Space => Key::Space,
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter => Key::Enter,
        _ => return None,
    })
}

/// Запуск игры в окне.
pub fn run<G: Game + 'static>(game: G, cfg: Config) {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = Arc::new(
        WindowBuilder::new()
            .with_title(cfg.title)
            .with_inner_size(LogicalSize::new(cfg.width as f64, cfg.height as f64))
            .build(&event_loop)
            .expect("window"),
    );

    let mut renderer = Renderer::new(window.clone());
    let mut engine = Engine::new(game, cfg.tick_rate);
    let size = window.inner_size();
    engine.ctx.screen_size = Vec2::new(size.width as f32, size.height as f32);
    engine.start();

    let clear = cfg.clear_color;

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(s) => {
                    renderer.resize(s.width, s.height);
                    engine.ctx.screen_size = Vec2::new(s.width as f32, s.height as f32);
                }
                WindowEvent::KeyboardInput {
                    event: KeyEvent { physical_key: PhysicalKey::Code(code), state, .. },
                    ..
                } => {
                    if let Some(k) = map_key(code) {
                        engine.ctx.input.set(k, state == ElementState::Pressed);
                    }
                }
                WindowEvent::RedrawRequested => {
                    engine.frame();
                    if engine.should_quit() {
                        engine.shutdown();
                        elwt.exit();
                    } else {
                        renderer.render(&engine.ctx.draw.sprites, &engine.ctx.camera, clear);
                    }
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop run");
}

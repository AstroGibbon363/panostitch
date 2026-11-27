//! Stability: the window, with photos loaded in every tab, gets thousands of
//! random clicks, drags, wheel turns and keys (as a careless user, or a cat on
//! the keyboard) at changing window sizes. No panic in any thread, and no frame
//! may take long (the work runs in the background). Headless; no file dialog
//! or other program is started (PANOSTITCH_NO_EXTERNAL). Longer runs:
//! MONKEY_SEED=7 MONKEY_STEPS=20000 cargo test --release --test monkey -- --nocapture

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use panostitch::gui::App;
use panostitch::image::{blur, Img};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static PANICS: AtomicUsize = AtomicUsize::new(0);

fn scene(h: usize, w: usize, seed: u64) -> Img {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    blur(&Img::from_vec(h, w, (0..h * w).map(|_| [r.gen(), r.gen(), r.gen()]).collect()), 2.0)
}

const KEYS: [Key; 16] = [Key::Escape, Key::Enter, Key::F, Key::P, Key::B, Key::Z, Key::Y, Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown, Key::Plus, Key::Minus, Key::Num0, Key::Delete, Key::Space];

#[test]
fn random_clicks_and_keys_do_not_break_the_window() {
    let cfg = tempfile::tempdir().unwrap();
    std::env::set_var("PANOSTITCH_CONFIG", cfg.path());
    std::env::set_var("PANOSTITCH_LANG", "en");
    std::env::set_var("PANOSTITCH_NO_EXTERNAL", "1");
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        PANICS.fetch_add(1, Ordering::SeqCst);
        prev(info);
    }));
    // Three overlapping photos, used by every tab.
    let d = tempfile::tempdir().unwrap();
    let sc = scene(500, 1300, 5);
    let paths: Vec<String> = (0..4)
        .map(|k| {
            let p = d.path().join(format!("frame{k}.png")).to_string_lossy().to_string();
            panostitch::io::save_panorama(&p, &sc.crop(0, 500, k.min(2) * 400, k.min(2) * 400 + 500)).unwrap();
            p
        })
        .collect();
    let ctx = egui::Context::default();
    let mut app = App::with_context(&ctx, paths[..3].to_vec());
    let t0 = Instant::now();
    let mut size = egui::vec2(1366.0, 768.0);
    let mut slowest = (0.0f64, String::new());
    let mut frame = |app: &mut App, events: Vec<Event>, size: egui::Vec2, what: &str| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)), time: Some(t0.elapsed().as_secs_f64()), events, ..Default::default() };
        let t = Instant::now();
        let mut out = ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        let dt = t.elapsed().as_secs_f64();
        if dt > slowest.0 {
            slowest = (dt, what.to_string());
        }
    };
    let idle = |app: &mut App, frame: &mut dyn FnMut(&mut App, Vec<Event>, egui::Vec2, &str), size: egui::Vec2| {
        let t = Instant::now();
        for _ in 0..3 {
            frame(app, vec![], size, "idle");
        }
        while app.busy() && t.elapsed() < Duration::from_secs(60) {
            frame(app, vec![], size, "idle");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    idle(&mut app, &mut frame, size);
    app.stitch.t_run_all();
    idle(&mut app, &mut frame, size);
    app.show_stack(true);
    app.stack.t_add(0, paths.clone());
    idle(&mut app, &mut frame, size);
    app.show_finish();
    app.finish.open(&paths[0]);
    idle(&mut app, &mut frame, size);
    app.show_timelapse();
    app.timelapse.t_add(paths.clone());
    idle(&mut app, &mut frame, size);

    let seed: u64 = std::env::var("MONKEY_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(42);
    let steps: usize = std::env::var("MONKEY_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut seen: std::collections::BTreeSet<String> = Default::default();
    let mut pos = Pos2::new(600.0, 400.0);
    let mut held: Option<PointerButton> = None;
    let mods = |rng: &mut ChaCha8Rng| match rng.gen_range(0..10) {
        0 => Modifiers::CTRL,
        1 => Modifiers::SHIFT,
        2 => Modifiers { ctrl: true, command: true, ..Default::default() },
        _ => Modifiers::NONE,
    };
    for step in 0..steps {
        if step % 1500 == 0 {
            match (step / 1500) % 4 {
                0 => app.show_stack(false),
                1 => app.show_stack(true),
                2 => app.show_finish(),
                _ => app.show_timelapse(),
            }
        }
        if step % 700 == 0 {
            size = [egui::vec2(1366.0, 768.0), egui::vec2(1280.0, 720.0), egui::vec2(1920.0, 1080.0), egui::vec2(900.0, 600.0)][rng.gen_range(0..4)];
        }
        let mut ev = Vec::new();
        let m = mods(&mut rng);
        match rng.gen_range(0..100) {
            // Move (a drag, while a button is held).
            0..=29 => {
                pos = if rng.gen_bool(0.3) { Pos2::new(rng.gen_range(0.0..size.x), rng.gen_range(0.0..size.y)) } else { pos + egui::vec2(rng.gen_range(-40.0..40.0), rng.gen_range(-40.0..40.0)) };
                pos = Pos2::new(pos.x.clamp(0.0, size.x - 1.0), pos.y.clamp(0.0, size.y - 1.0));
                ev.push(Event::PointerMoved(pos));
            }
            // Press or release.
            30..=59 => {
                let b = held.unwrap_or(if rng.gen_bool(0.8) { PointerButton::Primary } else { PointerButton::Secondary });
                let pressed = held.is_none();
                ev.push(Event::PointerMoved(pos));
                ev.push(Event::PointerButton { pos, button: b, pressed, modifiers: m });
                held = if pressed { Some(b) } else { None };
            }
            // A click somewhere new.
            60..=79 => {
                pos = Pos2::new(rng.gen_range(0.0..size.x), rng.gen_range(0.0..size.y));
                ev.push(Event::PointerMoved(pos));
                ev.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: m });
                ev.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: m });
                held = None;
            }
            // Wheel.
            80..=89 => ev.push(Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, rng.gen_range(-3.0..3.0)), phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown, modifiers: m }),
            // A key.
            _ => {
                let key = KEYS[rng.gen_range(0..KEYS.len())];
                ev.push(Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: m });
                ev.push(Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: m });
            }
        }
        frame(&mut app, ev, size, &format!("step {step}"));
        // What happened (the status lines), to see the clicks reach the controls.
        for st in [app.stitch.t_status(), app.stack.t_status(), app.finish.t_status(), app.timelapse.t_status()] {
            let key: String = st.chars().take(40).collect();
            seen.insert(key);
        }
        // Now and then let the started work run.
        if step % 50 == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    // Everything started ends (a tab takes in its finished work while it is shown).
    // Only in a long run: the work the clicks started (videos, saves) takes a while.
    let long = std::env::var("MONKEY_STEPS").is_ok();
    for tab in (0..4).filter(|_| long) {
        match tab {
            0 => app.show_stack(false),
            1 => app.show_stack(true),
            2 => app.show_finish(),
            _ => app.show_timelapse(),
        }
        idle(&mut app, &mut frame, size);
    }
    let busy = app.busy();
    if busy {
        println!("still busy: stitch {} stack {} finish {} timelapse {}; statuses: {:?} / {:?} / {:?} / {:?}", app.stitch.busy(), app.stack.busy(), app.finish.busy(), app.timelapse.busy(), app.stitch.t_status(), app.stack.t_status(), app.finish.t_status(), app.timelapse.t_status());
    }
    println!("{steps} random steps (seed {seed}); {} different status lines; slowest frame {:.2} s at {}", seen.len(), slowest.0, slowest.1);
    if std::env::var("MONKEY_SHOW").is_ok() {
        for s in &seen {
            println!("  {s}");
        }
    }
    assert_eq!(PANICS.load(Ordering::SeqCst), 0, "a thread panicked (see above)");
    assert!(!long || !busy, "work still running a minute after the last step");
    assert!(slowest.0 < 2.0, "a frame took {:.2} s ({})", slowest.0, slowest.1);
}

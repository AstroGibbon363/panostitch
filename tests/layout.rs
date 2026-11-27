//! The window's layout on small screens, in every language: no text cut off at
//! the side of its panel, and no panel drawn over another (a line too long to
//! wrap widens a side panel until the preview covers it). Headless, like
//! tests/gui.rs; its own test binary because it switches the language.

use panostitch::gui::i18n::{self, Lang};
use panostitch::gui::App;
use panostitch::image::{blur, Img};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::time::{Duration, Instant};

struct Win {
    ctx: egui::Context,
    app: App,
    t0: Instant,
    size: egui::Vec2,
}

impl Win {
    fn frame(&mut self) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.t0.elapsed().as_secs_f64()),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        out
    }
    fn idle(&mut self) {
        for _ in 0..3 {
            self.frame();
        }
        let t = Instant::now();
        while self.app.busy() {
            assert!(t.elapsed() < Duration::from_secs(120), "still busy: {}", self.app.stitch.t_status());
            self.frame();
            std::thread::sleep(Duration::from_millis(20));
        }
        for _ in 0..3 {
            self.frame();
        }
    }
    /// What is wrong with the layout of the current frame (after it settled).
    fn faults(&mut self) -> Vec<String> {
        for _ in 0..4 {
            self.frame();
        }
        let out = self.frame();
        let mut bad = Vec::new();
        for cs in &out.shapes {
            if let egui::Shape::Text(t) = &cs.shape {
                let r = t.galley.rect.translate(t.pos.to_vec2());
                let c = cs.clip_rect;
                let txt = t.galley.text();
                // Only the panels' own texts (the image views clip what they show on
                // purpose); file paths in text fields scroll; rows scrolled out of view are fine.
                let panel_text = c.min.y < 80.0 && c.max.y > self.size.y - 10.0;
                if !panel_text || txt.trim().is_empty() || txt.contains('/') || txt.contains('\\') || r.max.y < c.min.y || r.min.y > c.max.y {
                    continue;
                }
                if r.max.x > c.max.x + 2.0 || r.min.x < c.min.x - 2.0 {
                    bad.push(format!("text cut at the side: {:?}", txt.chars().take(60).collect::<String>()));
                }
            }
        }
        // The panels: clip rectangles from under the tab bar to the bottom.
        let mut panels: Vec<egui::Rect> = Vec::new();
        for cs in &out.shapes {
            let c = cs.clip_rect;
            if c.min.y > 20.0 && c.min.y < 40.0 && c.max.y >= self.size.y - 1.0 && c.width() < self.size.x - 1.0 && !panels.contains(&c) {
                panels.push(c);
            }
        }
        for i in 0..panels.len() {
            for j in i + 1..panels.len() {
                let (p, q) = (panels[i], panels[j]);
                let ov = p.max.x.min(q.max.x) - p.min.x.max(q.min.x);
                if ov > 2.0 && !(p.contains_rect(q) || q.contains_rect(p)) {
                    bad.push(format!("panels overlap by {ov:.0} px ({:.0}..{:.0} and {:.0}..{:.0})", p.min.x, p.max.x, q.min.x, q.max.x));
                }
            }
        }
        bad.sort();
        bad.dedup();
        bad
    }
}

fn scene(h: usize, w: usize, seed: u64) -> Img {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    blur(&Img::from_vec(h, w, (0..h * w).map(|_| [r.gen(), r.gen(), r.gen()]).collect()), 2.0)
}

#[test]
fn small_windows_in_every_language() {
    let cfg = tempfile::tempdir().unwrap();
    std::env::set_var("PANOSTITCH_CONFIG", cfg.path());
    std::env::set_var("PANOSTITCH_LANG", "en");
    // Three overlapping crops of one scene, stitched, so the panels show their full content.
    let d = tempfile::tempdir().unwrap();
    let sc = scene(500, 1300, 5);
    let paths: Vec<String> = (0..3)
        .map(|k| {
            let p = d.path().join(format!("frame{k}.png")).to_string_lossy().to_string();
            panostitch::io::save_panorama(&p, &sc.crop(0, 500, k * 400, k * 400 + 500)).unwrap();
            p
        })
        .collect();
    let ctx = egui::Context::default();
    let app = App::with_context(&ctx, paths.clone());
    let mut w = Win { ctx, app, t0: Instant::now(), size: egui::vec2(1280.0, 720.0) };
    w.idle();
    w.app.stitch.t_run_all();
    w.idle();
    assert!(w.app.stitch.t_pano_size().is_some(), "{}", w.app.stitch.t_status());
    w.app.show_finish();
    w.app.finish.open(&paths[0]);
    w.idle();
    let mut all = Vec::new();
    for range in [false, true] {
    if range {
        // A display mode with its Low / High sliders (and the reset and "view only" marks).
        let v = panostitch::finish::ViewStretch { mode: panostitch::finish::Display::Log, low: 0.001, high: 0.5 };
        w.app.stitch.t_set_display(v);
        w.app.stack.t_set_display(v);
        w.app.finish.t_set_display(v);
    }
    for lang in [Lang::En, Lang::Cs, Lang::De, Lang::Fr, Lang::Es, Lang::Pl] {
        i18n::set_lang(lang);
        for size in [egui::vec2(1280.0, 720.0), egui::vec2(1366.0, 768.0)] {
            w.size = size;
            for tab in 0..4 {
                match tab {
                    0 => w.app.show_stack(false),
                    1 => w.app.show_stack(true),
                    2 => w.app.show_finish(),
                    _ => w.app.show_timelapse(),
                }
                for f in w.faults() {
                    all.push(format!("{lang:?} {}×{} tab {tab}{}: {f}", size.x, size.y, if range { " (Low / High)" } else { "" }));
                }
            }
        }
    }
    }
    i18n::set_lang(Lang::En);
    assert!(all.is_empty(), "{} layout faults:\n{}", all.len(), all.join("\n"));
}

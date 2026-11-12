//! Headless tests of the window: the real tab code runs frame by frame on an
//! egui context without a screen. Images are written to a temporary folder.

use panostitch::gui::App;
use panostitch::image::{blur, Img};
use panostitch::stitcher::Projection;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::time::{Duration, Instant};

struct Win {
    ctx: egui::Context,
    app: App,
    t0: Instant,
}

impl Win {
    fn new(paths: Vec<String>) -> Self {
        let ctx = egui::Context::default();
        let app = App::with_context(&ctx, paths);
        Self { ctx, app, t0: Instant::now() }
    }
    fn frame(&mut self) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1700.0, 1000.0))),
            time: Some(self.t0.elapsed().as_secs_f64()),
            ..Default::default()
        };
        let app = &mut self.app;
        let _ = self.ctx.run_ui(input, |ui| app.frame(ui));
    }
    /// Run frames until `done` (or fail after `secs`).
    fn until(&mut self, what: &str, secs: u64, done: impl Fn(&App) -> bool) {
        let t = Instant::now();
        loop {
            self.frame();
            if done(&self.app) {
                return;
            }
            assert!(t.elapsed() < Duration::from_secs(secs), "timed out waiting for {what}; status: {} / {}", self.app.stitch.t_status(), self.app.stack.t_status());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn idle(&mut self, what: &str) {
        // A few frames so background work can start, then wait for it to finish.
        for _ in 0..3 {
            self.frame();
        }
        self.until(what, 120, |a| !a.busy());
        for _ in 0..3 {
            self.frame();
        }
    }
    fn settle(&mut self) {
        // Changes are recorded for undo once they have stayed the same for 0.4 s.
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(600) {
            self.frame();
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

/// `dir/name` with the system's path separator.
fn pj(dir: &str, name: &str) -> String {
    std::path::Path::new(dir).join(name).to_string_lossy().to_string()
}

fn scene(h: usize, w: usize, seed: u64) -> Img {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    blur(&Img::from_vec(h, w, (0..h * w).map(|_| [r.gen(), r.gen(), r.gen()]).collect()), 2.0)
}

#[test]
fn stitcher_tab_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy().to_string();
    let sc = scene(500, 1300, 5);
    let paths: Vec<String> = (0..3)
        .map(|k| {
            let p = pj(&d, &format!("frame{k}.png"));
            panostitch::io::save_panorama(&p, &sc.crop(0, 500, k * 400, k * 400 + 500)).unwrap();
            p
        })
        .collect();
    let mut w = Win::new(paths);
    w.idle("loading");
    assert_eq!(w.app.stitch.t_items(), 3, "{}", w.app.stitch.t_status());

    // Detect features only: keypoints on every image, nothing matched yet.
    w.app.stitch.t_detect();
    w.idle("detect");
    assert!(w.app.stitch.t_keypoints().iter().all(|&k| k > 100), "{:?}", w.app.stitch.t_keypoints());
    assert_eq!(w.app.stitch.t_pairs_ok(), 0);

    // Run all steps: detect, match, place, preview.
    w.app.stitch.t_run_all();
    w.idle("run all");
    assert_eq!(w.app.stitch.t_pairs_ok(), 2, "{}", w.app.stitch.t_status());
    assert!(w.app.stitch.t_status().starts_with("Done. 2 of 3 pairs accepted"), "{}", w.app.stitch.t_status());
    let (pw, ph) = w.app.stitch.t_pano_size().expect("no preview");
    assert!(pw > ph && pw > 600, "preview {pw}×{ph}");
    w.settle();

    // Every projection previews.
    for p in Projection::ALL {
        w.app.stitch.t_set_projection(p);
        w.idle("projection");
        assert!(w.app.stitch.t_pano_size().is_some(), "{p:?}: {}", w.app.stitch.t_status());
    }
    w.app.stitch.t_set_projection(Projection::Rectilinear);
    w.idle("rectilinear");
    w.settle();

    // Auto crop, then undo and redo it.
    w.app.stitch.t_crop();
    w.idle("crop");
    assert!(w.app.stitch.t_cropped(), "{}", w.app.stitch.t_status());
    w.settle();
    let (nu, _) = w.app.stitch.t_undo_len();
    assert!(nu >= 2, "undo steps: {nu}");
    w.app.stitch.t_undo(false);
    w.idle("undo");
    assert!(!w.app.stitch.t_cropped(), "undo did not remove the crop: {}", w.app.stitch.t_status());
    assert!(w.app.stitch.t_status().contains("Undone"));
    w.app.stitch.t_undo(true);
    w.idle("redo");
    assert!(w.app.stitch.t_cropped());

    // Nudge an image that is not the reference.
    let r = w.app.stitch.t_ref().unwrap();
    let other = if r == 0 { 1 } else { 0 };
    w.app.stitch.t_nudge(other, 5.0);
    w.idle("nudge");
    assert!(w.app.stitch.t_offsets()[other][0] != 0.0);

    // Zoomed in, the visible part is re-rendered sharper after a pause.
    w.app.stitch.t_preview_px(500);
    w.idle("small preview");
    assert!(!w.app.stitch.t_detail());
    w.app.stitch.t_zoom(12.0);
    w.until("detail render", 30, |a| a.stitch.t_detail());

    // Save the full-resolution panorama (TIFF and FITS).
    for ext in ["tif", "fits"] {
        let out = pj(&d, &format!("pano.{ext}"));
        w.app.stitch.t_save(&out);
        w.idle("save");
        let img = panostitch::io::load_any(&out, &Default::default()).unwrap();
        assert!(img.w > 500 && img.h > 300, "{ext}: {}×{}", img.w, img.h);
    }

    // Closing asks first while the work is not saved in a project.
    assert!(w.app.close_reasons().iter().any(|r| r.starts_with("Panorama")), "{:?}", w.app.close_reasons());
    // Save the project and open it in a new window: same images, pairs, moves, crop.
    let proj = w.app.stitch.save_project(&pj(&d, &format!("test"))).unwrap();
    assert!(w.app.close_reasons().is_empty(), "{:?}", w.app.close_reasons());
    assert!(proj.ends_with(".panoproj"));
    let mut w2 = Win::new(vec![proj]);
    w2.idle("open project");
    assert_eq!(w2.app.stitch.t_items(), 3, "{}", w2.app.stitch.t_status());
    assert_eq!(w2.app.stitch.t_pairs_ok(), 2);
    assert_eq!(w2.app.stitch.t_offsets(), w.app.stitch.t_offsets());
    assert!(w2.app.stitch.t_cropped());
    assert!(w2.app.stitch.t_pano_size().is_some(), "{}", w2.app.stitch.t_status());
    assert!(w2.app.stitch.t_status().starts_with("Opened"), "{}", w2.app.stitch.t_status());
    assert!(w2.app.close_reasons().is_empty(), "just opened: {:?}", w2.app.close_reasons());

    // Reordering and removing images keeps the matching results of the others.
    w2.app.stitch.t_select(0, &[]);
    w2.app.stitch.t_move(1);
    w2.idle("move");
    assert_eq!(w2.app.stitch.t_names(), vec!["frame1.png", "frame0.png", "frame2.png"]);
    assert_eq!(w2.app.stitch.t_pairs_ok(), 2, "{}", w2.app.stitch.t_status());
    w2.app.stitch.t_select(1, &[]); // frame0, at one end of the row
    w2.app.stitch.t_remove();
    w2.idle("remove");
    assert_eq!(w2.app.stitch.t_names(), vec!["frame1.png", "frame2.png"]);
    assert_eq!(w2.app.stitch.t_pairs_ok(), 1, "{}", w2.app.stitch.t_status());

    // Simple and modern appearance draw without trouble.
    let ctx = w2.ctx.clone();
    w2.app.set_simple(&ctx, true, true);
    w2.idle("simple");
    w2.app.show_stack(true);
    w2.idle("simple stack tab");
}

/// Star frames: a field turning around a pole above the frame, a dark ground below the middle.
fn star_frames(n: usize) -> Vec<Img> {
    let (h, w) = (300usize, 420usize);
    let mut r = ChaCha8Rng::seed_from_u64(1);
    let stars: Vec<(f64, f64, f64)> = (0..300).map(|_| (r.gen::<f64>() * 672.0 - 126.0, r.gen::<f64>() * 420.0 - 90.0, 0.05 + r.gen::<f64>().powi(4) * 1.5)).collect();
    (0..n)
        .map(|k| {
            let mut rng = ChaCha8Rng::seed_from_u64(100 + k as u64);
            let mut img = Img::filled(h, w, [0.03, 0.035, 0.05]);
            let th = k as f64 * 0.002;
            let pole = (210.0, -450.0);
            for &(x0, y0, f) in &stars {
                let (dx, dy) = (x0 - pole.0, y0 - pole.1);
                let (x, y) = (pole.0 + th.cos() * dx - th.sin() * dy, pole.1 + th.sin() * dx + th.cos() * dy);
                if !(4.0..=416.0).contains(&x) || !(4.0..=170.0).contains(&y) {
                    continue;
                }
                for yy in (y as i64 - 3)..=(y as i64 + 4) {
                    for xx in (x as i64 - 3)..=(x as i64 + 4) {
                        let v = (f * (-((xx as f64 - x).powi(2) + (yy as f64 - y).powi(2)) / 2.42).exp()) as f32;
                        let p = &mut img[((yy - 1) as usize, (xx - 1) as usize)];
                        p[0] += v;
                        p[1] += v;
                        p[2] += v;
                    }
                }
            }
            for y in 0..h {
                for x in 0..w {
                    let p = &mut img[(y, x)];
                    if y > 180 {
                        *p = [0.01, 0.009, 0.008];
                    }
                    let n: f32 = 0.002 * (rng.gen::<f32>() - 0.5) * 3.4;
                    *p = [p[0] + n, p[1] + n, p[2] + n];
                }
            }
            img
        })
        .collect()
}

#[test]
fn stacking_tab_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy().to_string();
    let paths: Vec<String> = star_frames(6)
        .iter()
        .enumerate()
        .map(|(k, img)| {
            let p = pj(&d, &format!("star{k:02}.fits")); // linear, as stacking needs
            panostitch::io::save_panorama(&p, img).unwrap();
            p
        })
        .collect();
    let mut w = Win::new(vec![]);
    w.app.show_stack(true);
    w.app.stack.t_add(0, paths.clone());
    w.idle("base preview");
    assert!(w.app.stack.t_shown().is_some(), "{}", w.app.stack.t_status());
    {
        let c = w.app.stack.t_cfg();
        c.sky.mode = panostitch::stacker::SkyMode::BoundaryLine;
        c.sky.line = Some((0.0, 0.55, 1.0, 0.55));
        c.align_mode = panostitch::stacker::AlignMode::Freeze;
        c.light_pollution.filter = panostitch::stacker::Pollution::Uneven;
        c.tone = panostitch::stacker::Tone::Stretch;
    }
    // Quick preview, then Start.
    w.app.stack.t_run(true);
    w.idle("quick preview");
    let r = w.app.stack.t_result().unwrap_or_else(|| panic!("no preview: {}", w.app.stack.t_status()));
    assert!(r.preview && r.saved.is_empty());
    w.until("post-processing", 60, |a| a.stack.t_post_ready());
    let out = pj(&d, &format!("stacked.tif"));
    w.app.stack.t_cfg().output = out.clone();
    w.app.stack.t_exclude(&paths[0]);
    w.app.stack.t_run(false);
    w.idle("start");
    let r = w.app.stack.t_result().unwrap();
    assert_eq!(r.saved, vec![out.clone()], "{}", w.app.stack.t_status());
    assert!(r.report.iter().any(|f| f.status == panostitch::stacker::Status::Excluded));
    assert_eq!(r.report.iter().filter(|f| f.used()).count(), 5, "{}", panostitch::stacker::report_text(&r.report));
    assert!(std::path::Path::new(&out).is_file());

    // The project keeps files, exclusions, the sky line and the settings.
    assert!(w.app.close_reasons().iter().any(|r| r.starts_with("Star stacking")), "{:?}", w.app.close_reasons());
    let proj = w.app.stack.save_project(&pj(&d, &format!("stack"))).unwrap();
    assert!(w.app.close_reasons().is_empty(), "{:?}", w.app.close_reasons());
    let mut w2 = Win::new(vec![proj.clone()]);
    w2.idle("open stacking project");
    assert!(w2.app.close_reasons().is_empty(), "just opened: {:?}", w2.app.close_reasons());
    assert_eq!(w2.app.stack.t_files()[0].len(), 6, "{}", w2.app.stack.t_status());
    // The preview shows the base frame after opening a project.
    w2.until("base preview", 30, |a| a.stack.t_shown().is_some());
    assert_eq!(w2.app.stack.t_shown().map(|s| s.to_string()), w2.app.stack.t_base());
    let c = w2.app.stack.t_cfg().clone();
    assert_eq!(c.excluded, vec![paths[0].clone()]);
    assert_eq!(c.sky.line, Some((0.0, 0.55, 1.0, 0.55)));
    assert_eq!(c.align_mode, panostitch::stacker::AlignMode::Freeze);
    assert_eq!(c.light_pollution.filter, panostitch::stacker::Pollution::Uneven);
    assert_eq!(c.output, out);

    // A painted mask survives a save / open too.
    let mut m = panostitch::image::Bits::new(30, 40);
    for r in 0..12 {
        for c in 0..40 {
            m.set(r, c, true);
        }
    }
    m.set(20, 7, true);
    w2.app.stack.t_cfg().sky.mode = panostitch::stacker::SkyMode::IrregularMask;
    w2.app.stack.t_cfg().sky.mask = m.clone();
    let proj2 = w2.app.stack.save_project(&proj).unwrap();
    let mut w3 = Win::new(vec![proj2]);
    w3.idle("reopen");
    assert_eq!(w3.app.stack.t_cfg().sky.mask, m);
}

/// Every symbol on the buttons and labels has a glyph (else egui draws an empty box).
#[test]
fn button_symbols_have_glyphs() {
    let mut w = Win::new(vec![]);
    w.frame();
    w.frame();
    let mut used: Vec<char> = Vec::new();
    for f in ["src/gui/mod.rs", "src/gui/stitch.rs", "src/gui/stack.rs"] {
        let s = std::fs::read_to_string(f).unwrap();
        used.extend(s.chars().filter(|c| ('\u{2190}'..='\u{2BFF}').contains(c)));
    }
    used.sort();
    used.dedup();
    assert!(used.len() > 10);
    for fam in [egui::FontId::proportional(14.0), egui::FontId::monospace(14.0)] {
        let missing: String = w.ctx.fonts_mut(|f| used.iter().copied().filter(|c| !f.has_glyph(&fam, *c)).collect());
        assert!(missing.is_empty(), "{fam:?}: no glyph for {missing}");
    }
}

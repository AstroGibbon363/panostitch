//! Tests of the stacking engine on synthetic night landscapes (no files, no
//! window), the stitcher on crops of a synthetic scene, and the file formats.
//!
//! A scene: sky with stars that turn around a pole above the frame, a hilly dark
//! foreground that stays in place, background noise. Frames are made in memory
//! and handed to `stack` through its `loader`.

use panostitch::image::{blur, luminance, median_of, Bits, Gray, Img};
use panostitch::io;
use panostitch::stacker as sk;
use panostitch::stitcher as st;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

const H: usize = 300;
const W: usize = 420;

fn horizon_of(x: f64) -> f64 {
    0.62 * H as f64 + 18.0 * (x / 37.0).sin() + 10.0 * (x / 11.0 + 1.0).sin() // ground below
}

struct Scene {
    stars: Vec<(f64, f64, f64)>,
    ground: Gray,
}

fn randn(rng: &mut ChaCha8Rng) -> f64 {
    let u1: f64 = rng.gen_range(1e-12..1.0);
    let u2: f64 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

fn scene(nstars: usize, seed: u64) -> Scene {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let stars = (0..nstars)
        .map(|_| {
            let x: f64 = rng.gen::<f64>() * 1.6 * W as f64 - 0.3 * W as f64;
            let y: f64 = rng.gen::<f64>() * 1.4 * H as f64 - 0.3 * H as f64;
            let f: f64 = 0.05 + rng.gen::<f64>().powi(4) * 1.5;
            (x, y, f)
        })
        .collect();
    let g = Gray::from_vec(H, W, (0..H * W).map(|_| rng.gen::<f32>()).collect());
    Scene { stars, ground: blur(&g, 2.0) }
}

/// Frame k: the stars turned by k·dθ around the pole, each a short arc (exposure fraction `expo`).
#[allow(clippy::too_many_arguments)]
fn frame(sc: &Scene, k: usize, dth: f64, expo: f64, seed: u64, fgain: f32, sigma: f64) -> Img {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let pole = (W as f64 / 2.0, -1.5 * H as f64);
    let mut img = Img::filled(H, W, [0.03, 0.035, 0.05]);
    let nsub = if expo > 0.0 { 6 } else { 1 };
    for &(x0, y0, f) in &sc.stars {
        for j in 0..nsub {
            let th = (k as f64 + expo * j as f64 / (nsub - 1).max(1) as f64) * dth;
            let (dx, dy) = (x0 - pole.0, y0 - pole.1);
            let x = pole.0 + th.cos() * dx - th.sin() * dy;
            let y = pole.1 + th.sin() * dx + th.cos() * dy;
            if !(4.0 <= x && x <= W as f64 - 4.0 && 4.0 <= y && y <= H as f64 - 4.0) {
                continue;
            }
            for yy in (y.floor() as i64 - 3)..=(y.floor() as i64 + 4) {
                for xx in (x.floor() as i64 - 3)..=(x.floor() as i64 + 4) {
                    if !(1 <= yy && yy <= H as i64 && 1 <= xx && xx <= W as i64) {
                        continue;
                    }
                    let v = (f / nsub as f64 * (-((xx as f64 - x).powi(2) + (yy as f64 - y).powi(2)) / (2.0 * sigma * sigma)).exp()) as f32;
                    let p = &mut img[((yy - 1) as usize, (xx - 1) as usize)];
                    p[0] += v;
                    p[1] += v;
                    p[2] += v;
                }
            }
        }
    }
    for y in 1..=H {
        for x in 1..=W {
            let p = &mut img[(y - 1, x - 1)];
            if y as f64 > horizon_of(x as f64) {
                let g = 0.008 + 0.02 * sc.ground.at(y - 1, x - 1);
                *p = [g, 0.9 * g, 0.8 * g];
            }
            let n = 0.002 * randn(&mut rng) as f32;
            *p = [(p[0] + n) * fgain, (p[1] + n) * fgain, (p[2] + n) * fgain];
        }
    }
    img
}

/// A configuration over K in-memory frames: paths "mem/f01" … with a loader.
fn memstack(imgs: Vec<Img>) -> (sk::StackConfig, HashMap<String, Img>, tempfile::TempDir) {
    let paths: Vec<String> = (1..=imgs.len()).map(|k| format!("mem/f{k:02}.tif")).collect();
    let lookup: HashMap<String, Img> = paths.iter().cloned().zip(imgs).collect();
    let dir = tempfile::tempdir().unwrap();
    let cfg = sk::StackConfig { stars: paths, output: dir.path().join("stacked.tif").to_string_lossy().to_string(), ..Default::default() };
    (cfg, lookup, dir)
}

fn line_sky() -> sk::Sky {
    sk::Sky { mode: sk::SkyMode::BoundaryLine, line: Some((0.0, 0.5, 1.0, 0.5)), gradient: 0.0, mask: Bits::new(0, 0), feather: 0.01 }
}

fn true_mask(rows: usize, cols: usize) -> Bits {
    Bits::from_fn(rows, cols, |r, c| ((r as f64 + 0.5) / rows as f64 * H as f64) < horizon_of((c as f64 + 0.5) / cols as f64 * W as f64))
}

fn no_progress(_: &str) -> Result<(), String> {
    Ok(())
}

fn run(cfg: &sk::StackConfig, lookup: &HashMap<String, Img>) -> sk::StackResult {
    let loader = |p: &str| lookup.get(p).cloned().ok_or_else(|| format!("no frame {p}"));
    sk::stack(cfg, false, &no_progress, Some(&loader)).unwrap()
}

#[test]
fn automatic_sky_detection() {
    let sc = scene(300, 1);
    let img = frame(&sc, 0, 0.004, 0.0, 100, 1.0, 1.1);
    let (mask, _) = sk::detect_sky(&img, 200, None, 0.01);
    let truth = true_mask(mask.h, mask.w);
    let acc = mask.v.iter().zip(truth.v.iter()).filter(|(a, b)| a == b).count() as f64 / mask.len() as f64;
    let ground_as_sky = mask.v.iter().zip(truth.v.iter()).filter(|(a, b)| **a && !**b).count() as f64 / truth.v.iter().filter(|b| !**b).count() as f64;
    eprintln!("sky detection: acc {acc:.3}, ground as sky {ground_as_sky:.4}");
    assert!(acc > 0.95);
    assert!(ground_as_sky < 0.02); // stays off the foreground
    // A frame that is all sky: everything is sky.
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let sky_only = Img::from_vec(H, W, (0..H * W).map(|_| [0.03 + 0.002 * randn(&mut rng) as f32, 0.035, 0.05]).collect());
    let (m2, _) = sk::detect_sky(&sky_only, 100, None, 0.01);
    assert!(m2.count() as f64 / m2.len() as f64 > 0.97);
}

#[test]
fn mask_feather_inside_sky_only() {
    let m = Bits::from_fn(50, 100, |r, _| r < 25);
    let wm = sk::mask_weight(&m, 0.1);
    assert!((25..50).all(|r| (0..100).all(|c| wm.at(r, c) == 0.0))); // foreground untouched
    assert!((wm.at(0, 50) - 1.0).abs() < 1e-5);
    assert!(wm.at(23, 50) > 0.0 && wm.at(23, 50) < 0.5); // soft just inside the edge
    let w0 = sk::mask_weight(&m, 0.0);
    assert!((0..23).all(|r| (0..100).all(|c| w0.at(r, c) > 0.99)));
}

#[test]
fn trail_fades() {
    assert_eq!(sk::trail_fade(sk::Fade::Off, 5), vec![1.0; 5]);
    let (fin, fout, both) = (sk::trail_fade(sk::Fade::In, 9), sk::trail_fade(sk::Fade::Out, 9), sk::trail_fade(sk::Fade::Both, 9));
    assert!(fin.windows(2).all(|w| w[0] <= w[1]) && fin[8] == 1.0);
    assert!(fout.windows(2).all(|w| w[0] >= w[1]) && fout[0] == 1.0);
    assert!(both[0] < both[4] && both[8] < both[4] && both[4] == 1.0);
    let cfg = sk::StackConfig { motion_effect: true, ..Default::default() };
    assert_eq!(sk::fade_kind(&cfg), sk::Fade::In); // older projects
}

#[test]
fn align_stars_and_log() {
    let sc = scene(300, 1);
    let imgs: Vec<Img> = (1..=6).map(|k| frame(&sc, k, 0.002, 0.0, 100 + k as u64, 1.0, 1.1)).collect();
    let (mut cfg, lookup, _dir) = memstack(imgs);
    cfg.sky = line_sky();
    let r = run(&cfg, &lookup);
    eprintln!("{}", sk::report_text(&r.report));
    assert_eq!(r.report.iter().filter(|fi| fi.used()).count(), 6);
    assert!(r.report.iter().all(|fi| fi.status == sk::Status::Base || fi.err < 0.5));
    assert!(r.log.iter().any(|l| l.contains("Combined")) && r.log.last().unwrap().contains("in total"));
    assert!(std::path::Path::new(&cfg.output).is_file());
}

#[test]
fn star_trails_gap_filling() {
    // Point-like stars moving ~8 px per frame: dashes without gap filling.
    let sc = scene(300, 1);
    let dth = 0.015;
    let imgs: Vec<Img> = (0..8).map(|k| frame(&sc, k, dth, 0.0, 100 + k as u64, 1.0, 1.0)).collect();
    let (mut cfg0, lookup, _dir) = memstack(imgs);
    cfg0.composition = sk::Composition::StarTrails;
    cfg0.sky = line_sky();
    cfg0.reject.auto = false;
    cfg0.auto_brightness = false;
    cfg0.tone = sk::Tone::Linear;
    let mut cfg1 = cfg0.clone();
    cfg1.trails = (sk::Fade::Off, true);
    let r0 = run(&cfg0, &lookup);
    let r1 = run(&cfg1, &lookup);
    // Halfway between the positions of a bright star in frames 3 and 4.
    let pole = (W as f64 / 2.0, -1.5 * H as f64);
    let mut gains = Vec::new();
    for &(x0, y0, f) in sc.stars.iter().filter(|s| s.2 > 0.4) {
        let _ = f;
        let th = 3.5 * dth;
        let (dx, dy) = (x0 - pole.0, y0 - pole.1);
        let p = (pole.0 + th.cos() * dx - th.sin() * dy, pole.1 + th.sin() * dx + th.cos() * dy);
        let (x, y) = (p.0.round() as i64, p.1.round() as i64);
        if !(20 <= x && x <= W as i64 - 20 && 20 <= y && y as f64 <= 0.45 * H as f64) {
            continue;
        }
        let (yi, xi) = ((y - 1) as usize, (x - 1) as usize);
        gains.push(luminance(r1.raw.at(yi, xi)) / luminance(r0.raw.at(yi, xi)).max(1e-4));
    }
    let mg = median_of(gains.iter().copied());
    eprintln!("gap filling: n {} median gain {mg:.2}", gains.len());
    assert!(gains.len() >= 3);
    assert!(mg > 3.0); // the gap is filled
    assert!(r1.log.iter().any(|l| l.contains("Gap filling")));
    // The foreground is not touched by the trails.
    assert!((luminance(r1.raw.at(H - 6, 9)) - luminance(r0.raw.at(H - 6, 9))).abs() < 1e-4);
}

#[test]
fn foreground_image() {
    let sc = scene(300, 1);
    let imgs: Vec<Img> = (1..=5).map(|k| frame(&sc, k, 0.002, 0.0, 100 + k as u64, 1.0, 1.1)).collect();
    let fg = Img::filled(H, W, [0.2, 0.1, 0.05]); // a bright, brown "blue hour" ground
    let (mut cfg, mut lookup, _dir) = memstack(imgs);
    cfg.sky = sk::Sky { mode: sk::SkyMode::BoundaryLine, line: Some((0.0, 0.45, 1.0, 0.45)), gradient: 0.0, mask: Bits::new(0, 0), feather: 0.01 };
    cfg.auto_brightness = false;
    cfg.tone = sk::Tone::Linear;
    cfg.align_mode = sk::AlignMode::Accumulation;
    lookup.insert("mem/fg.tif".into(), fg);
    let mut c_match = cfg.clone();
    c_match.foreground = sk::Foreground { path: "mem/fg.tif".into(), match_brightness: true, ev: 0.0 };
    let mut c_raw = cfg.clone();
    c_raw.foreground = sk::Foreground { path: "mem/fg.tif".into(), match_brightness: false, ev: 1.0 };
    let r_match = run(&c_match, &lookup);
    let r_raw = run(&c_raw, &lookup);
    let r_none = run(&cfg, &lookup);
    let g = r_raw.raw.at(H - 4, 4);
    assert!((g[0] - 0.4).abs() < 0.004); // 2^1 × the foreground
    assert!((g[2] - 0.1).abs() < 0.002);
    assert!((g[0] / g[1] - 2.0).abs() < 0.02); // its colour
    // Matched: the brightness of the stacked ground.
    let lum_ground = |r: &sk::StackResult| -> f32 {
        let y0 = (0.47 * H as f64).round() as usize;
        median_of((y0..H).flat_map(|y| (0..W).map(move |x| (y, x))).map(|(y, x)| luminance(r.raw.at(y, x))))
    };
    let (a, b) = (lum_ground(&r_match), lum_ground(&r_none));
    assert!((a - b).abs() < 0.1 * b, "matched {a} vs stacked {b}");
    assert_eq!(r_match.raw.at(9, 9), r_none.raw.at(9, 9)); // the sky is the stack's
    let mut c_err = c_match.clone();
    c_err.sky.mode = sk::SkyMode::None;
    let loader = |p: &str| lookup.get(p).cloned().ok_or_else(|| format!("no frame {p}"));
    assert!(sk::stack(&c_err, false, &no_progress, Some(&loader)).is_err());
}

#[test]
fn automatic_sky_in_a_run() {
    let sc = scene(300, 1);
    let imgs: Vec<Img> = (1..=4).map(|k| frame(&sc, k, 0.002, 0.0, 100 + k as u64, 1.0, 1.1)).collect();
    let (mut cfg, lookup, _dir) = memstack(imgs);
    cfg.sky = sk::Sky { mode: sk::SkyMode::Automatic, line: None, gradient: 0.0, mask: Bits::new(0, 0), feather: 0.01 };
    cfg.align_mode = sk::AlignMode::Freeze;
    let r = run(&cfg, &lookup);
    let m = r.mask.as_ref().unwrap();
    let truth = true_mask(m.h, m.w);
    let acc = m.v.iter().zip(truth.v.iter()).filter(|(a, b)| a == b).count() as f64 / m.len() as f64;
    assert!(acc > 0.95);
    assert!(r.log.iter().any(|l| l.contains("Sky detected")));
}

#[test]
fn quick_preview_and_postprocessing() {
    let sc = scene(300, 1);
    let imgs: Vec<Img> = (1..=5).map(|k| frame(&sc, k, 0.002, 0.0, 100 + k as u64, 1.0, 1.1)).collect();
    let (mut cfg, lookup, _dir) = memstack(imgs);
    cfg.sky = line_sky();
    cfg.distortion = sk::Distortion::Complex;
    cfg.light_pollution.filter = sk::Pollution::Uneven;
    cfg.light_pollution.iaa = true;
    cfg.enhance_stars = (true, 50.0);
    cfg.bg_extraction = (true, 2);
    cfg.color_calibration = true;
    cfg.color_noise = (true, 50.0);
    cfg.tone = sk::Tone::Stretch;
    cfg.timelapse = (true, 3);
    cfg.color_space = sk::ColorSpace::AdobeRgb;
    let loader = |p: &str| lookup.get(p).cloned().ok_or_else(|| format!("no frame {p}"));
    let r = sk::stack(&cfg, true, &no_progress, Some(&loader)).unwrap();
    assert!(r.preview && r.saved.is_empty());
    assert!(r.background.is_some() && r.stretch.is_some());
    assert!(r.image.px.iter().all(|p| p.iter().all(|v| v.is_finite())));
}

// --------------------------------------------------------------------------
// Stitcher
// --------------------------------------------------------------------------

fn smooth_scene(h: usize, w: usize, seed: u64) -> Img {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let img = Img::from_vec(h, w, (0..h * w).map(|_| [rng.gen(), rng.gen(), rng.gen()]).collect());
    blur(&img, 2.0)
}

#[test]
fn features_match_a_shift() {
    let sc = smooth_scene(400, 900, 3);
    let a = sc.crop(0, 400, 0, 500);
    let b = sc.crop(0, 400, 300, 800);
    let dp = st::DetectParams { scale: 1.0, nkeypoints: 1500, ..Default::default() };
    let (fa, fb) = (st::detect(&a, &dp), st::detect(&b, &dp));
    assert!(fa.pts.len() > 100 && fb.pts.len() > 100);
    let (pa, pb) = st::match_pair(&fa, &fb, 0.8, 0.35);
    assert!(pa.len() >= 20, "{} matches", pa.len());
    let (h, inl) = st::ransac(&pb, &pa, st::Model::Translation, 500, 3.0).unwrap();
    assert!(inl.len() >= 15);
    assert!((h[(0, 2)] - 300.0).abs() < 1.0 && h[(1, 2)].abs() < 1.0, "H = {h}");
    let (hh, inl2) = st::ransac(&pb, &pa, st::Model::Homography, 500, 3.0).unwrap();
    assert!(inl2.len() >= 15 && (hh[(0, 2)] - 300.0).abs() < 2.0);
    assert!(st::accept_pair(pa.len(), inl2.len(), &hh, 400, 500, 10).0);
}

#[test]
fn stitch_three_crops() {
    let sc = smooth_scene(500, 1300, 5);
    let crops: Vec<Img> = (0..3).map(|k| sc.crop(0, 500, k * 400, k * 400 + 500)).collect();
    let mut log = Vec::new();
    for (proj, model, refine) in [
        (st::Projection::Rectilinear, st::Model::Translation, false),
        (st::Projection::Cylindrical, st::Model::Homography, true),
    ] {
        let p = st::StitchParams { projection: proj, model, refine, feature_scale: 1.0, nkeypoints: 2000, blend: st::Blend::Smart, ..Default::default() };
        let pano = st::stitch(&crops, &p, &mut |m| log.push(m)).unwrap();
        eprintln!("{proj:?}: {}×{}", pano.w, pano.h);
        assert!((pano.w as i64 - 1300).abs() < 40 && (pano.h as i64 - 500).abs() < 40, "{}×{}", pano.w, pano.h);
        let covered = pano.px.iter().filter(|p| luminance(**p) > 0.0).count() as f64 / pano.len() as f64;
        assert!(covered > 0.9);
    }
    assert!(log.iter().any(|l| l.contains("Pair 1-2")));
    assert!(log.iter().any(|l| l.contains("Pair 2-3")));
    // Every projection renders something finite.
    let (h, w) = (crops[0].h, crops[0].w);
    let g = vec![st::M3::identity()];
    for proj in st::Projection::ALL {
        let cp = st::ComposeParams { projection: proj, f: 600.0, c: [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0], scale: 0.25, ..Default::default() };
        let o = st::outline(&g[0], h, w, proj, 600.0, cp.c, (0.0, 600.0), 24);
        let (img, cov) = st::compose(&[&crops[0]], &g, st::bounds(&[o]), &cp, &[1.0], &[1.0], &[(h, w)], true);
        assert!(img.px.iter().all(|p| p.iter().all(|v| v.is_finite())), "{proj:?}");
        assert!(cov.unwrap().count() > 0, "{proj:?}");
    }
    let r = st::largest_rectangle(&Bits::from_fn(10, 10, |y, x| y >= 2 && x >= 3 && x < 8)).unwrap();
    assert_eq!(r, ((2, 9), (3, 7)));
}

// --------------------------------------------------------------------------
// File formats
// --------------------------------------------------------------------------

#[test]
fn file_round_trips() {
    let img = smooth_scene(40, 60, 9);
    let dir = tempfile::tempdir().unwrap();
    for ext in ["tif", "png", "jpg", "bmp", "fits", "xisf"] {
        let p = dir.path().join(format!("t.{ext}")).to_string_lossy().to_string();
        io::save_panorama(&p, &img).unwrap();
        let back = io::load_any(&p, &io::LoadOptions::default()).unwrap();
        assert_eq!((back.h, back.w), (40, 60), "{ext}");
        let err: f32 = back.px.iter().zip(img.px.iter()).map(|(a, b)| (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()).sum::<f32>() / (3 * img.len()) as f32;
        let tol = if ext == "jpg" { 0.03 } else if ext == "bmp" { 0.005 } else { 1e-3 };
        assert!(err < tol, "{ext}: mean error {err}");
    }
    assert!(io::is_astro("a.FITS") && io::is_raw("x.NEF") && !io::is_image("x.txt"));
    assert!(st::auto_stretch(&img, 0.25, 2.8).2 == 1.0);
    assert_eq!(st::lum_histogram(&img, 16).len(), 16);
}

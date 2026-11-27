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
    assert!(st::accept_pair(pa.len(), inl2.len(), &hh, 400, 500, 10, &inl2.iter().map(|&k| pb[k]).collect::<Vec<_>>()).0);
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
        let o = st::outline(&g[0], h, w, proj, 600.0, cp.c, st::Lens::new(0.0, 0.0, 600.0), 24);
        let (img, cov) = st::compose(&[&crops[0]], &g, st::bounds(&[o]), &cp, &[1.0], &[1.0], &[(h, w)], true);
        assert!(img.px.iter().all(|p| p.iter().all(|v| v.is_finite())), "{proj:?}");
        assert!(cov.unwrap().count() > 0, "{proj:?}");
    }
    let r = st::largest_rectangle(&Bits::from_fn(10, 10, |y, x| y >= 2 && x >= 3 && x < 8)).unwrap();
    assert_eq!(r, ((2, 9), (3, 7)));
}

#[test]
fn panorama_in_bands_matches_one_piece() {
    // Three overlapping crops placed by translation; one with a "moved" patch for the deghosting.
    let sc = smooth_scene(700, 1300, 8);
    let mut crops: Vec<Img> = (0..3).map(|k| sc.crop(0, 700, k * 400, k * 400 + 500)).collect();
    for y in 300..360 {
        for x in 420..480 {
            crops[1].px[y * 500 + x] = [0.9, 0.1, 0.1];
        }
    }
    let (h, w) = (700, 500);
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let g: Vec<st::M3> = (0..3).map(|k| st::M3::new(1.0, 0.0, 400.0 * k as f64, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)).collect();
    let o: Vec<_> = g.iter().map(|gi| st::outline(gi, h, w, st::Projection::Cylindrical, 900.0, c, st::Lens::new(0.0, 0.0, 900.0), 24)).collect();
    let box_ = st::bounds(&o);
    let refs: Vec<&Img> = crops.iter().collect();
    // The same photos kept in half precision.
    let halves: Vec<panostitch::image::HalfImg> = crops.iter().map(panostitch::image::HalfImg::from_img).collect();
    let hrefs: Vec<&panostitch::image::HalfImg> = halves.iter().collect();
    let sizes = vec![(h, w); 3];
    for blend in [st::Blend::Feather, st::Blend::Smart] {
        let cp = st::ComposeParams { projection: st::Projection::Cylindrical, f: 900.0, c, blend, sharpness: 3.0, ..Default::default() };
        let (whole, _) = st::compose(&refs, &g, box_, &cp, &[1.0; 3], &[1.0; 3], &sizes, false);
        let mut banded = Img::new(whole.h, whole.w);
        let mut nb = 0;
        let (bw, bh) = st::compose_bands(&hrefs, &g, box_, &cp, &[1.0; 3], &[1.0; 3], &sizes, 128, false, &mut |y0, band, _| {
            banded.px[y0 * band.w..(y0 + band.h) * band.w].copy_from_slice(&band.px);
            nb += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!((bw, bh), (whole.w, whole.h));
        assert!(nb >= 5, "{nb} bands");
        let diff: Vec<f32> = whole.px.iter().zip(&banded.px).map(|(a, b)| (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max)).collect();
        let max = diff.iter().cloned().fold(0.0, f32::max);
        let mean = diff.iter().sum::<f32>() / diff.len() as f32;
        eprintln!("{blend:?}: max {max:.4}, mean {mean:.6}");
        // Feathering is the same pixel for pixel (up to half precision); the smart blend nearly so.
        let (lim_max, lim_mean) = if blend == st::Blend::Feather { (0.002, 0.0005) } else { (0.05, 0.002) };
        assert!(max < lim_max && mean < lim_mean, "{blend:?}: max {max}, mean {mean}");
    }
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

/// Long background jobs must not hold up the window thread's own (small)
/// parallel work: they run in a separate, low-priority pool.
#[test]
fn background_work_does_not_block_the_window_thread() {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    // Occupy every worker of the background pool until told to stop.
    panostitch::sys::background(move || {
        (0..4 * rayon::current_num_threads()).into_par_iter().for_each(|_| {
            while !s.load(Ordering::Relaxed) {
                std::hint::spin_loop();
            }
        })
    });
    std::thread::sleep(Duration::from_millis(200));
    let t = Instant::now();
    let sum: u64 = (0..100_000u64).into_par_iter().map(|x| x % 7).sum();
    let took = t.elapsed();
    stop.store(true, Ordering::Relaxed);
    assert_eq!(sum, (0..100_000u64).map(|x| x % 7).sum::<u64>());
    assert!(took < Duration::from_secs(1), "window-side work waited {took:?}");
}

/// An alignment that flips, folds or hugely scales the frame is refused: stars
/// from a tripod or a mount only shift, turn and scale a little. (A degenerate
/// fit on a few repeated stars once smeared a whole frame across the stack.)
#[test]
fn implausible_alignments_are_refused() {
    let m = |a: [f64; 9]| st::M3::from_row_slice(&a);
    let w = 3000;
    assert!(sk::plausible_warp(&m([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]), w).is_ok());
    // A small turn and shift, and a 180° turn (meridian flip), are fine.
    let (c, s) = (0.02f64.cos(), 0.02f64.sin());
    assert!(sk::plausible_warp(&m([c, -s, 12.0, s, c, -7.0, 0.0, 0.0, 1.0]), w).is_ok());
    assert!(sk::plausible_warp(&m([-1.0, 0.0, 3000.0, 0.0, -1.0, 2000.0, 0.0, 0.0, 1.0]), w).is_ok());
    // A mirror image, a 3× scale and the folded transform seen in a real stack are not.
    assert!(sk::plausible_warp(&m([-1.0, 0.0, 3000.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]), w).is_err());
    assert!(sk::plausible_warp(&m([3.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 1.0]), w).is_err());
    assert!(sk::plausible_warp(&m([-1.0565, -0.7797, -0.00047, 2.9394, 2.1693, 0.0013, 2241.8, 100.0, 1.0]).transpose(), w).is_err());
}

// ---------------------------------------------------------------------------
// Night-sky panoramas: star matching, zoom groups, the turning sky
// ---------------------------------------------------------------------------

type V3 = nalgebra::Vector3<f64>;

fn rot(axis: [f64; 3], deg: f64) -> st::M3 {
    st::rodrigues(&(V3::new(axis[0], axis[1], axis[2]).normalize() * deg.to_radians()))
}

/// Pixel (1-based) of world direction `d` in a camera turned by `r` (world → camera) with focal `f`, if in an h×w frame.
fn project(r: &st::M3, d: &V3, f: f64, h: usize, w: usize) -> Option<st::Pt> {
    let c = r * d;
    if c[2] <= 0.0 {
        return None;
    }
    let (x, y) = ((w as f64 + 1.0) / 2.0 + f * c[0] / c[2], (h as f64 + 1.0) / 2.0 + f * c[1] / c[2]);
    (x >= 1.0 && x <= w as f64 && y >= 1.0 && y <= h as f64).then_some([x, y])
}

fn random_sky(n: usize, seed: u64) -> Vec<V3> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n).map(|_| V3::new(randn(&mut rng), randn(&mut rng), randn(&mut rng)).normalize()).collect()
}

#[test]
fn stars_match_across_turn_and_zoom() {
    let (h, w) = (1000usize, 1500usize);
    let sky = random_sky(20000, 7);
    // Second frame: camera turned 25° and rolled 10°, zoomed out (focal 900 instead of 1000),
    // and the sky turned 2° between the shots.
    let ra = st::M3::identity();
    let rb = rot([0.0, 0.0, 1.0], 10.0) * rot([0.0, 1.0, 0.0], 25.0);
    let turn = rot([0.3, -0.8, 0.5], 2.0);
    let (fa, fb) = (1000.0, 900.0);
    let pa: Vec<st::Pt> = sky.iter().filter_map(|d| project(&ra, d, fa, h, w)).take(150).collect();
    let pb: Vec<st::Pt> = sky.iter().filter_map(|d| project(&rb, &(turn * d), fb, h, w)).take(150).collect();
    let (a, b) = (st::stars::SkyIndex::new(&pa, h, w, fa), st::stars::SkyIndex::new(&pb, h, w, fb));
    let m = st::stars::match_stars(&a, &b, 2.0).expect("frames should match by their stars");
    assert!(m.p1.len() >= 20, "{} stars matched of {}", m.p1.len(), m.expected);
    assert!(m.err < 0.5, "mean error {} px", m.err);
    // The homography takes stars of b onto the same stars in a.
    for (p, q) in m.p1.iter().zip(&m.p2) {
        assert!(st::reproj_err(&m.h, *q, *p) < 1.0);
    }
    // A different patch of sky does not match.
    let rc = rot([0.0, 1.0, 0.0], 150.0);
    let pc: Vec<st::Pt> = sky.iter().filter_map(|d| project(&rc, d, fa, h, w)).take(150).collect();
    assert!(st::stars::match_stars(&a, &st::stars::SkyIndex::new(&pc, h, w, fa), 2.0).is_err());
}

#[test]
fn refinement_finds_zoom_and_turning_sky() {
    let (h, w) = (800usize, 1200usize);
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    // Four frames: two at focal 1000, two at 900 (the zoom crept); shot over 11 minutes.
    let rs = [st::M3::identity(), rot([0.0, 1.0, 0.0], 30.0), rot([1.0, 0.0, 0.0], -35.0), rot([1.0, 0.0, 0.0], -35.0) * rot([0.0, 1.0, 0.0], 30.0)];
    let fs = [1000.0, 1000.0, 900.0, 900.0];
    let ts = [0.0, 40.0, 600.0, 660.0];
    let omega = V3::new(0.2, -0.7, 0.4).normalize() * (15.041f64 / 3600.0).to_radians();
    let at = |t: f64| st::rodrigues(&(omega * t));
    let pts = random_sky(20000, 11);
    // Ground pairs see fixed points; star pairs see the sky as it was at each shot.
    let pairs: [(usize, usize, bool); 6] = [(0, 1, false), (2, 3, false), (0, 2, false), (1, 3, true), (0, 3, true), (1, 2, true)];
    let mut matches: Vec<st::MatchSet> = Vec::new();
    let mut skyflags = Vec::new();
    for &(i, j, sky) in &pairs {
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for d in &pts {
            let (di, dj) = if sky { (at(ts[i]) * d, at(ts[j]) * d) } else { (*d, *d) };
            if let (Some(p), Some(q)) = (project(&rs[i], &di, fs[i], h, w), project(&rs[j], &dj, fs[j], h, w)) {
                a.push(p);
                b.push(q);
            }
            if a.len() >= 60 {
                break;
            }
        }
        assert!(a.len() >= 20, "pair {i}-{j} overlaps too little");
        matches.push((i, j, a, b));
        skyflags.push(sky);
    }
    // Start a little off: rotations disturbed, focal 5 % long, the zoom ratio from EXIF.
    let k = |f: f64| st::kmat(f, c);
    let g0: Vec<st::M3> = (0..4).map(|i| k(1000.0) * (rot([0.3, 0.5, 0.1], 0.4 * i as f64) * rs[i]).transpose() * st::inv(&k(fs[i]))).collect();
    let opts = st::BundleOpts { group: vec![0, 0, 1, 1], focal: vec![1.0, 1.0, 0.9, 0.9], time: ts.to_vec(), sky: skyflags, ..Default::default() };
    let b = st::bundle_adjust_with(&g0, &[c; 4], 0, 1050.0, &[0, 1, 2, 3], &matches, 60, 5.0, 60, false, &opts);
    assert!(b.rms_after < 0.2, "rms {} → {}", b.rms_before, b.rms_after);
    assert!((b.f - 1000.0).abs() < 2.0, "focal {}", b.f);
    assert!((b.focal[2] - 0.9).abs() < 0.003 && (b.focal[3] - 0.9).abs() < 0.003, "zoom ratios {:?}", b.focal);
    let w = b.sky.expect("the sky's turning");
    let rate = V3::new(w[0], w[1], w[2]).norm().to_degrees() * 3600.0;
    assert!((rate - 15.041).abs() < 0.5, "sky turned {rate}°/h");
    // Without star pairs flagged, the same data cannot be fitted as well.
    let plain = st::BundleOpts { sky: vec![], ..opts };
    let b2 = st::bundle_adjust_with(&g0, &[c; 4], 0, 1050.0, &[0, 1, 2, 3], &matches, 60, 5.0, 60, false, &plain);
    assert!(b2.rms_after > 5.0 * b.rms_after.max(0.05), "rms without the turning sky {}", b2.rms_after);
}

#[test]
fn steep_pairs_are_judged_on_their_overlap() {
    // Cameras pointing 70° apart (a row looking up, one looking down): the far
    // corners of one photo lie behind the other camera, the overlap is fine.
    let (h, w, f) = (1500usize, 1000usize, 700.0);
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let hm = st::kmat(f, c) * rot([1.0, 0.0, 0.0], 70.0) * st::inv(&st::kmat(f, c));
    assert!(!st::plausible(&hm, h, w));
    // Points of the second photo that land in the first.
    let pts: Vec<st::Pt> = (0..40)
        .flat_map(|a| (0..40).map(move |b| [1.0 + a as f64 * (w as f64 - 1.0) / 39.0, 1.0 + b as f64 * (h as f64 - 1.0) / 39.0]))
        .filter(|p| {
            let q = hm * V3::new(p[0], p[1], 1.0);
            q[2] > 0.0 && (1.0..=w as f64).contains(&(q[0] / q[2])) && (1.0..=h as f64).contains(&(q[1] / q[2]))
        })
        .collect();
    assert!(pts.len() > 50, "{} points in the overlap", pts.len());
    assert!(st::plausible_overlap(&hm, &pts, h, w));
    assert!(st::accept_pair(pts.len(), pts.len(), &hm, h, w, 20, &pts).0);
    // A mirrored transform stays refused.
    let mirror = hm * st::M3::new(-1.0, 0.0, w as f64 + 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    assert!(!st::plausible_overlap(&mirror, &pts, h, w));
}

#[test]
fn sky_alignment_moves_only_the_stars() {
    // One photo: stars on a smooth gradient. Rendering it with its sky turned
    // (a shift here) must move the stars and keep the gradient in place.
    let (h, w) = (200usize, 300usize);
    let mut img = Img::from_fn(h, w, |_, j| { let v = 0.2 + 0.3 * j as f32 / w as f32; [v, v, v] });
    for k in 0..40 {
        let (y, x) = (10 + (k * 37) % 180, 10 + (k * 53) % 280);
        for (dy, dx) in (0..3).flat_map(|a| (0..3).map(move |b| (a, b))) {
            img.px[(y + dy) * w + x + dx] = [1.0; 3];
        }
    }
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let cp = st::ComposeParams { projection: st::Projection::Rectilinear, f: 500.0, c, blend: st::Blend::Average, ..Default::default() };
    let box_ = st::bounds(&[st::outline(&st::M3::identity(), h, w, cp.projection, cp.f, c, cp.lens(0), 24)]);
    let (plain, _) = st::compose(&[&img], &[st::M3::identity()], box_, &cp, &[1.0], &[1.0], &[(h, w)], false);
    let shift = st::translation(6.0, 0.0);
    let map = st::SkyMap { open: Bits { w: w / 4, h: h / 4, v: vec![true; (w / 4) * (h / 4)] }, bg: blur(&panostitch::image::resize(&img, 1.0 / 16.0), 1.0) };
    let sky = st::SkyAlign { g: vec![Some(shift)], maps: vec![Some(std::sync::Arc::new(map))] };
    let cp2 = st::ComposeParams { sky: Some(std::sync::Arc::new(sky)), ..cp.clone() };
    let (turned, _) = st::compose(&[&img], &[st::M3::identity()], box_, &cp2, &[1.0], &[1.0], &[(h, w)], false);
    // Every star now shows 6 px to the right; the background stays where it was.
    let stars: Vec<(usize, usize)> = (0..plain.h * plain.w).filter(|&k| luminance(plain.px[k]) > 0.9).map(|k| (k / plain.w, k % plain.w)).collect();
    assert!(stars.len() > 40, "{} star pixels", stars.len());
    for &(y, x) in stars.iter().filter(|p| p.1 + 6 < plain.w) {
        assert!(luminance(turned.at(y, x + 6)) > 0.9, "star at {y},{x} not moved: {:?}", turned.at(y, x + 6));
    }
    let (a, b) = (luminance(plain.at(150, 150)), luminance(turned.at(150, 150)));
    assert!((a - b).abs() < 0.01, "background moved: {a} → {b}");
    // No open sky: the photo as shot.
    let closed = st::SkyMap { open: Bits { w: w / 4, h: h / 4, v: vec![false; (w / 4) * (h / 4)] }, bg: Img::new(4, 4) };
    let sky = st::SkyAlign { g: vec![Some(shift)], maps: vec![Some(std::sync::Arc::new(closed))] };
    let (same, _) = st::compose(&[&img], &[st::M3::identity()], box_, &st::ComposeParams { sky: Some(std::sync::Arc::new(sky)), ..cp }, &[1.0], &[1.0], &[(h, w)], false);
    assert_eq!(same.px, plain.px);
}

#[test]
fn capture_times_and_zoom_groups() {
    let t = |s: &str| io::raw::exif_seconds(s).unwrap();
    assert_eq!(t("2026:11:14 22:08:30") - t("2026:11:14 21:08:30"), 3600.0);
    assert_eq!(t("2026:03:01 00:00:10") - t("2026:02:28 23:59:50"), 20.0);
    assert_eq!(t("2025:01:01 00:00:00") - t("2024:12:31 23:00:00"), 3600.0);
    assert!(io::raw::exif_seconds("garbage").is_none());
    let shot = |f: Option<f64>| io::ShotInfo { focal_mm: f, time: None, focal_px: None };
    let (g, r) = panostitch::gui::stitch::zoom(&[shot(Some(17.0)), shot(Some(17.0)), shot(Some(15.0)), shot(None), shot(Some(11.5))]);
    assert_eq!(g, vec![0, 0, 2, 0, 1]);
    assert!((r[2] - 15.0 / 17.0).abs() < 1e-9 && r[3] == 1.0 && (r[4] - 11.5 / 17.0).abs() < 1e-9, "{r:?}");
}

#[test]
fn display_range_low_and_high() {
    use panostitch::finish::{display_view, Display, ViewStretch};
    let img = Img::from_fn(1, 5, |_, x| { let v = [0.0, 0.005, 0.01, 0.05, 0.5][x]; [v, v, v] });
    // Linear with Low 0.005, High 0.01: below black, between linear, above white.
    let v = ViewStretch { mode: Display::Linear, low: 0.005, high: 0.01 };
    let out = display_view(&img, v, &img);
    let l: Vec<f32> = out.px.iter().map(|c| c[0]).collect();
    assert!(l[0] == 0.0 && l[1].abs() < 1e-6 && (l[2] - 1.0).abs() < 1e-6 && l[3] == 1.0 && l[4] == 1.0, "{l:?}");
    // The full range shows the image as it is.
    assert_eq!(display_view(&img, ViewStretch::default(), &img).px, img.px);
    // A narrow range of faint values keeps its steps (applied before the curve's table).
    let ramp = Img::from_fn(1, 100, |_, x| { let v = x as f32 * 1e-5; [v, v, v] });
    let a = display_view(&ramp, ViewStretch { mode: Display::Asinh, low: 0.0, high: 0.001 }, &ramp);
    let distinct: std::collections::HashSet<u32> = a.px.iter().map(|c| c[0].to_bits()).collect();
    assert!(distinct.len() > 95, "posterised: {} levels", distinct.len());
}

#[test]
fn control_points_give_the_rotation_and_snap_to_stars() {
    // Two views of one camera turned by 20° (yaw) and 3° (pitch): a handful of
    // points give back the homography between them.
    let (h, w, f) = (400usize, 600usize, 700.0);
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let k = st::kmat(f, c);
    let r = st::rotation_h(20f64.to_radians(), 3f64.to_radians(), f, c, 0.0);
    let truth = r; // pixels of the second view into the first
    let p2: Vec<st::Pt> = vec![[100.0, 80.0], [150.0, 320.0], [60.0, 200.0]];
    let p1: Vec<st::Pt> = p2.iter().map(|q| { let (x, y) = st::apply_h(&truth, q[0], q[1]); [x, y] }).collect();
    let _ = k;
    for n in [2, 3] {
        let hh = st::stars::rotation_from_points(&p1[..n], &p2[..n], (h, w), (h, w), f, f).expect("fit");
        for q in [[300.0, 200.0], [50.0, 50.0]] {
            let (a, b) = (st::apply_h(&hh, q[0], q[1]), st::apply_h(&truth, q[0], q[1]));
            assert!((a.0 - b.0).hypot(a.1 - b.1) < 0.05, "{n} points: {a:?} vs {b:?}");
        }
    }
    assert!(st::stars::rotation_from_points(&p1[..1], &p2[..1], (h, w), (h, w), f, f).is_none());
    // A click 4 px off a star lands on its centre; on a flat area it stays put.
    let mut img = Img::from_fn(60, 60, |_, _| [0.1; 3]);
    let (sy, sx) = (30.3f64, 25.6f64); // centre, 1-based
    for y in 0..60 {
        for x in 0..60 {
            let d2 = (x as f64 + 1.0 - sx).powi(2) + (y as f64 + 1.0 - sy).powi(2);
            img.px[y * 60 + x][0] += (0.8 * (-d2 / 2.0).exp()) as f32;
            img.px[y * 60 + x][1] = img.px[y * 60 + x][0];
            img.px[y * 60 + x][2] = img.px[y * 60 + x][0];
        }
    }
    let q = st::stars::snap_to_star(&img, sx + 3.0, sy - 2.5, 8.0).expect("snapped");
    assert!((q[0] - sx).hypot(q[1] - sy) < 0.3, "{q:?}");
    let flat = Img::from_fn(60, 60, |_, _| [0.3; 3]);
    assert!(st::stars::snap_to_star(&flat, 30.0, 30.0, 8.0).is_none());
}

#[test]
fn source_masks_force_one_photo() {
    // Two photos of the same view, red and green, rendered on top of each other:
    // where the green photo's mask is painted, only green shows; elsewhere the
    // two blend; outside the mask nothing changes.
    let (h, w) = (120usize, 160usize);
    let red = Img::from_fn(h, w, |_, _| [1.0, 0.0, 0.0]);
    let green = Img::from_fn(h, w, |_, _| [0.0, 1.0, 0.0]);
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let id = st::M3::identity();
    let mask = st::mask_raster(&[st::MaskDab { x: 40.0, y: 60.0, r: 20.0, erase: false }, st::MaskDab { x: 40.0, y: 60.0, r: 6.0, erase: true }], h, w).expect("mask");
    assert!(st::mask_raster(&[st::MaskDab { x: 1.0, y: 1.0, r: 5.0, erase: true }], h, w).is_none());
    for blend in [st::Blend::Average, st::Blend::Feather, st::Blend::Smart] {
        let cp = st::ComposeParams { projection: st::Projection::Rectilinear, f: 400.0, c, blend, ..Default::default() };
        let box_ = st::bounds(&[st::outline(&id, h, w, cp.projection, cp.f, c, cp.lens(0), 24)]);
        let force = std::sync::Arc::new(vec![None, Some(std::sync::Arc::new(mask.clone()))]);
        let cpm = st::ComposeParams { force: Some(force), ..cp.clone() };
        let (plain, _) = st::compose(&[&red, &green], &[id, id], box_, &cp, &[1.0, 1.0], &[1.0, 1.0], &[(h, w), (h, w)], false);
        let (masked, _) = st::compose(&[&red, &green], &[id, id], box_, &cpm, &[1.0, 1.0], &[1.0, 1.0], &[(h, w), (h, w)], false);
        let at = |im: &Img, x: usize, y: usize| im.at(y - 1, x - 1);
        // Inside the painted ring: green only.
        let g = at(&masked, 40 + 14, 60);
        assert!(g[1] > 0.97 && g[0] < 0.03, "{blend:?}: {g:?}");
        // In the erased centre (too small for the smart blend's pyramid to keep) and far away: as without masks.
        let spots: &[(usize, usize)] = if blend == st::Blend::Smart { &[(130, 30)] } else { &[(40, 60), (130, 30)] };
        for &(x, y) in spots {
            let (a, b) = (at(&plain, x, y), at(&masked, x, y));
            assert!((a[0] - b[0]).abs() < 0.02 && (a[1] - b[1]).abs() < 0.02, "{blend:?} at {x},{y}: {a:?} vs {b:?}");
        }
    }
    // Output point → photo pixel, and outside the photo nothing.
    let q = st::uv_to_pixel(&id, (h, w), st::Projection::Rectilinear, 400.0, c, st::Lens::default(), 0.0, 0.0).unwrap();
    assert!((q[0] - c[0]).abs() < 1e-9 && (q[1] - c[1]).abs() < 1e-9, "{q:?}");
    assert!(st::uv_to_pixel(&id, (h, w), st::Projection::Rectilinear, 400.0, c, st::Lens::default(), 500.0, 0.0).is_none());
}

#[test]
fn rbf_background_follows_complex_gradients() {
    // Sky with two broad glows (a town at the left edge, another lower right) on a
    // slope, stars, and a band of Milky Way: RBF leaves a much flatter sky than
    // the best polynomial, and keeps the band.
    let (h, w) = (300usize, 450usize);
    let sky = |y: usize, x: usize| {
        let (u, v) = (x as f64 / w as f64, y as f64 / h as f64);
        0.05 + 0.02 * u + 0.12 * (-((u - 0.0).powi(2) + (v - 0.9).powi(2)) / 0.08).exp() + 0.08 * (-((u - 0.8).powi(2) + (v - 0.75).powi(2)) / 0.05).exp()
    };
    let band = |y: usize, x: usize| 0.04 * (-((x as f64 - 0.6 * y as f64 - 150.0) / 18.0).powi(2)).exp();
    let mut img = Img::from_fn(h, w, |y, x| { let v = (sky(y, x) + band(y, x)) as f32; [v, v * 0.95, v * 1.05] });
    for k in 0..300 {
        let (y, x) = ((k * 97) % h, (k * 61) % w);
        img.px[y * w + x] = [0.9; 3];
    }
    let wt = Gray::from_fn(h, w, |_, _| 1.0);
    let mut cfg = sk::StackConfig { bg_extraction: (true, 4), ..Default::default() };
    cfg.sky.mode = sk::SkyMode::All; // RBF is for frames that are all sky
    // Flatness: spread of the corrected sky away from the band and the stars.
    let spread = |out: &Img| {
        let mut v: Vec<f32> = (0..h).step_by(3).flat_map(|y| (0..w).step_by(3).map(move |x| (y, x))).filter(|&(y, x)| band(y, x) < 0.002).map(|(y, x)| out.at(y, x)[1]).filter(|v| *v < 0.5).collect();
        let q = panostitch::image::quantiles(&mut v, &[0.02, 0.98]);
        q[1] - q[0]
    };
    cfg.bg_model = sk::BgModel::Polynomial;
    let poly = sk::post::extract_background_cfg(&img, &wt, &cfg);
    cfg.bg_model = sk::BgModel::Rbf;
    let rbf = sk::post::extract_background_cfg(&img, &wt, &cfg);
    let (sp, sr) = (spread(&poly), spread(&rbf));
    assert!(sr < 0.5 * sp && sr < 0.03, "RBF spread {sr}, polynomial {sp}");
    // The grid: a higher tolerance keeps more samples; the band's are left out at the default.
    let kept = |tol: f64| sk::post::rbf_grid(&img, &wt, 20, tol).unwrap().keep.iter().filter(|k| **k).count();
    let (strict, loose) = (kept(0.5), kept(4.0));
    assert!(strict < kept(1.0) && kept(1.0) <= loose, "kept: {strict}, {}, {loose}", kept(1.0));
    let g = sk::post::rbf_grid(&img, &wt, 20, 1.0).unwrap();
    let on_band = g.p.iter().zip(&g.keep).filter(|(q, _)| { let (x, y) = (q.0 * w as f64, q.1 * w as f64); (x - 0.6 * y - 150.0).abs() < 8.0 }).collect::<Vec<_>>();
    assert!(!on_band.is_empty() && on_band.iter().all(|(_, k)| !**k), "samples on the band used");
    // The Milky Way band stays (it is not taken for background).
    let on = rbf.at(150, (0.6 * 150.0 + 150.0) as usize)[1];
    let off = rbf.at(150, 60)[1];
    assert!(on - off > 0.025, "band lost: {on} vs {off}");
}

#[test]
fn manual_colour_calibration_and_stretches() {
    // A reddish background and a bluish "white" object: the areas picked by hand make both neutral.
    let (h, w) = (100usize, 100usize);
    let img = Img::from_fn(h, w, |y, x| if (40..60).contains(&y) && (40..60).contains(&x) { [0.30, 0.40, 0.55] } else { [0.12, 0.10, 0.10] });
    let wt = Gray::from_fn(h, w, |_, _| 1.0);
    let m = sk::ManualCal { background: Some([0.0, 0.0, 0.3, 0.3]), white: Some([0.42, 0.42, 0.58, 0.58]), temperature: 0.0, tint: 0.0 };
    let out = sk::post::calibrate_manual(&img, &wt, &m);
    let (b, c) = (out.at(10, 10), out.at(50, 50));
    assert!((b[0] - b[1]).abs() < 1e-4 && (b[2] - b[1]).abs() < 1e-4, "background {b:?}");
    assert!((c[0] - c[1]).abs() < 1e-4 && (c[2] - c[1]).abs() < 1e-4, "white {c:?}");
    // Warmer: more red than blue.
    let warm = sk::post::calibrate_manual(&img, &wt, &sk::ManualCal { temperature: 0.5, ..m.clone() }).at(50, 50);
    assert!(warm[0] > warm[2], "{warm:?}");
    // The new stretches brighten a faint linear image; the linear data stays as it was.
    let faint = Img::from_fn(h, w, |y, x| { let v = 0.01 + 0.002 * ((x + y) % 7) as f32; [v, v, v] });
    for tone in [sk::Tone::Stretch, sk::Tone::Asinh, sk::Tone::Ghs] {
        let cfg = sk::StackConfig { tone, auto_brightness: false, ..Default::default() };
        let f = sk::post::finish(&faint, &wt, &cfg, None);
        let (lin, img_) = (f.linear.at(50, 50)[1], f.image.at(50, 50)[1]);
        assert!(img_ > 5.0 * lin && img_ <= 1.0, "{tone:?}: {lin} → {img_}");
        assert!((lin - faint.at(50, 50)[1]).abs() < 1e-6);
    }
    // What the file holds: by format, or as chosen.
    let mut cfg = sk::StackConfig::default();
    assert!(cfg.saves_linear("a.fits") && !cfg.saves_linear("a.tif"));
    cfg.save_linear = Some(false);
    assert!(!cfg.saves_linear("a.fits"));
    cfg.save_linear = Some(true);
    assert!(cfg.saves_linear("a.jpg"));
}

#[test]
fn wavelet_noise_reduction_keeps_stars_and_brightness() {
    // A gradient with stars and noise that is 3× stronger in the dark half (as
    // after a stretch), plus colour speckles.
    let (h, w) = (200usize, 300usize);
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let mut n = || -> f32 { (0..6).map(|_| rng.gen::<f32>()).sum::<f32>() - 3.0 };
    let clean = |y: usize, x: usize| -> f32 {
        let mut v = 0.2 + 0.4 * x as f32 / w as f32;
        for k in 0..30 {
            let (sy, sx) = (10.0 + ((k * 53) % 180) as f32, 10.0 + ((k * 89) % 280) as f32);
            v += 0.5 * (-((y as f32 - sy).powi(2) + (x as f32 - sx).powi(2)) / 2.0).exp();
        }
        v
    };
    let mut img = Img::new(h, w);
    for y in 0..h {
        for x in 0..w {
            let s = if x < w / 2 { 0.03 } else { 0.01 };
            let (l, cr, cb) = (clean(y, x) + s * n(), 0.03 * n(), 0.03 * n());
            img.px[y * w + x] = [l + cr, l - 0.5 * (cr + cb), l + cb];
        }
    }
    let flat = |im: &Img, x0: usize| {
        // spread of the luminance minus the clean value, away from stars
        let v: Vec<f32> = (0..h).flat_map(|y| (x0..x0 + 60).map(move |x| (y, x))).filter(|&(y, x)| (clean(y, x) - (0.2 + 0.4 * x as f32 / w as f32)) < 0.01).map(|(y, x)| luminance(im.at(y, x)) - clean(y, x)).collect();
        let m = v.iter().sum::<f32>() / v.len() as f32;
        (v.iter().map(|a| (a - m).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
    };
    let out = panostitch::denoise::reduce_noise(&img, 0.7);
    for x0 in [20, 200] {
        let (a, b) = (flat(&img, x0), flat(&out, x0));
        assert!(b < 0.5 * a, "noise at x {x0}: {a} → {b}");
    }
    // Stars keep most of their peak.
    let (sy, sx) = (10usize, 10usize);
    let peak = |im: &Img| luminance(im.at(sy, sx)) - (0.2 + 0.4 * sx as f32 / w as f32);
    assert!(peak(&out) > 0.75 * (clean(sy, sx) - (0.2 + 0.4 * sx as f32 / w as f32)), "star {} → {}", peak(&img), peak(&out));
    // Colour noise: the colour speckles shrink, the luminance stays exactly.
    let cn = panostitch::denoise::reduce_color_noise(&img, 0.7);
    let chroma = |im: &Img| { let v: Vec<f32> = im.px.iter().map(|c| c[0] - luminance(*c)).collect(); let m = v.iter().sum::<f32>() / v.len() as f32; (v.iter().map(|a| (a - m).powi(2)).sum::<f32>() / v.len() as f32).sqrt() };
    assert!(chroma(&cn) < 0.4 * chroma(&img), "colour noise {} → {}", chroma(&img), chroma(&cn));
    assert!(img.px.iter().zip(&cn.px).all(|(a, b)| (luminance(*a) - luminance(*b)).abs() < 1e-4), "luminance changed");
    // Nothing at strength 0.
    assert_eq!(panostitch::denoise::reduce_noise(&img, 0.0).px, img.px);
}

/// A photo's stars made from the catalogue: camera turned by `r` (camera ray v
/// points at the sky direction r·v), focal length f, distortion k1. Stars are
/// listed by a brightness that the vignetting and the sky disturb, some are
/// missing, some false ones are added.
fn fake_photo_stars(r: &st::M3, f: f64, k1: f64, size: (usize, usize), seed: u64) -> Vec<st::Pt> {
    let cat = st::solve::catalog();
    let (h, w) = (size.0 as f64, size.1 as f64);
    let c = [(w + 1.0) / 2.0, (h + 1.0) / 2.0];
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut out: Vec<(f64, st::Pt)> = Vec::new();
    for (k, s) in cat.dir.iter().enumerate() {
        let v = r.transpose() * s;
        if v[2] < 0.1 {
            continue;
        }
        let (x, y) = st::distort(c[0] + f * v[0] / v[2], c[1] + f * v[1] / v[2], c, st::Lens::new(k1, 0.0, f));
        if x < 10.0 || y < 10.0 || x > w - 10.0 || y > h - 10.0 {
            continue;
        }
        let rr = ((x - c[0]).powi(2) + (y - c[1]).powi(2)).sqrt() / (0.5 * w.hypot(h));
        let m = cat.mag[k] as f64 + 1.5 * rr * rr + rng.gen_range(-0.6..0.6);
        if m > 7.0 || rng.gen::<f64>() < 0.2 {
            continue;
        }
        out.push((m, [x + rng.gen_range(-0.4..0.4), y + rng.gen_range(-0.4..0.4)]));
    }
    for _ in 0..30 {
        out.push((rng.gen_range(2.0..7.0), [rng.gen_range(10.0..w - 10.0), rng.gen_range(10.0..h - 10.0)]));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.truncate(400);
    out.into_iter().map(|(_, p)| p).collect()
}

#[test]
fn plate_solving_wide_fields() {
    let cat = st::solve::catalog();
    assert!(cat.dir.len() > 40_000 && cat.mag[0] < -1.0, "catalogue");
    // Sirius where it belongs (RA 6h45m, Dec −16.7°).
    let (ra, de) = st::solve::radec(&cat.dir[0]);
    assert!((ra - 101.29).abs() < 0.05 && (de + 16.72).abs() < 0.05, "{ra} {de}");
    let size = (1000usize, 1500usize);
    // A 15 mm lens on full frame at 1500 px (f ≈ 625 px, about 100° across), a
    // 24 mm one, and a 50 mm one; the focal length given is off by up to 10%.
    for (k, (f, k1, ra, dec, roll, guess)) in [(625.0, -0.04, 280.0, 10.0, 30.0, 0.92), (940.0, -0.02, 84.0, -5.0, -100.0, 1.08), (1950.0, 0.0, 200.0, 55.0, 170.0, 1.0)].into_iter().enumerate() {
        let z = st::solve::direction(ra, dec);
        let up = nalgebra::Vector3::new(0.0, 0.0, 1.0);
        let x0 = up.cross(&z).normalize();
        let y0 = z.cross(&x0);
        let (cr, sr) = (f64::to_radians(roll).cos(), f64::to_radians(roll).sin());
        let (x, y) = (x0 * cr + y0 * sr, -x0 * sr + y0 * cr);
        let r = st::M3::from_columns(&[x, y, z]);
        let stars = fake_photo_stars(&r, f, k1, size, k as u64);
        let t0 = std::time::Instant::now();
        let sol = st::solve::solve(&stars, size, f * guess).unwrap_or_else(|e| panic!("field {k}: {e}"));
        let secs = t0.elapsed().as_secs_f64();
        let (cra, cde) = sol.centre();
        let d = st::solve::direction(cra, cde).dot(&z).clamp(-1.0, 1.0).acos().to_degrees();
        let turn = ((sol.r.transpose() * r).trace() - 1.0).clamp(-2.0, 2.0) / 2.0;
        assert!(d < 0.02 && turn.acos().to_degrees() < 0.03, "field {k}: centre off by {d}°, turn {}°", turn.acos().to_degrees());
        assert!((sol.f / f - 1.0).abs() < 0.003, "field {k}: f {} vs {f}", sol.f);
        assert!((sol.k1 - k1).abs() < 0.01, "field {k}: k1 {} vs {k1}", sol.k1);
        assert!(sol.stars.len() > 100 && sol.rms < 0.6, "field {k}: {} stars, rms {}", sol.stars.len(), sol.rms);
        println!("field {k}: {} stars, rms {:.2} px, f {:.1}, k1 {:.3}, {:.2} s", sol.stars.len(), sol.rms, sol.f, sol.k1, secs);
    }
    // Random dots are not the sky.
    let mut rng = ChaCha8Rng::seed_from_u64(9);
    let junk: Vec<st::Pt> = (0..300).map(|_| [rng.gen_range(10.0..1490.0), rng.gen_range(10.0..990.0)]).collect();
    assert!(st::solve::solve(&junk, size, 625.0).is_err());
    // Two overlapping solved photos share their stars.
    let r1 = st::M3::from_columns(&[nalgebra::Vector3::new(1.0, 0.0, 0.0), nalgebra::Vector3::new(0.0, 0.0, -1.0), nalgebra::Vector3::new(0.0, 1.0, 0.0)]);
    let r2 = r1 * st::rodrigues(&nalgebra::Vector3::new(0.0, 0.6, 0.0));
    let a = st::solve::solve(&fake_photo_stars(&r1, 625.0, 0.0, size, 3), size, 625.0).expect("a");
    let b = st::solve::solve(&fake_photo_stars(&r2, 625.0, 0.0, size, 4), size, 625.0).expect("b");
    let (p1, p2) = st::solve::common_stars(&a, &b);
    assert!(p1.len() > 30, "{} common stars", p1.len());
    let hh = st::stars::rotation_from_points(&p1, &p2, size, size, 625.0, 625.0).unwrap();
    let err = p1.iter().zip(&p2).map(|(x, y)| st::reproj_err(&hh, *y, *x)).sum::<f64>() / p1.len() as f64;
    assert!(err < 1.0, "pair error {err}");
}

#[test]
fn frame_gradients_and_normalisation() {
    // One frame with a plane (light pollution) and a galaxy: the plane goes, the
    // galaxy and the mean level stay.
    let (h, w) = (200usize, 300usize);
    let plane = |y: usize, x: usize, a: f64, b: f64| (a * ((x as f64 + 0.5) / w as f64 - 0.5) + b * ((y as f64 + 0.5) / h as f64 - 0.5)) as f32;
    let galaxy = |y: usize, x: usize| (0.05 * (-(((x as f64 - 200.0) / 25.0).powi(2) + ((y as f64 - 80.0) / 15.0).powi(2))).exp()) as f32;
    let mut img = Img::from_fn(h, w, |y, x| { let v = 0.1 + plane(y, x, 0.04, -0.02) + galaxy(y, x); [v, v * 0.9, v * 1.1] });
    let wt = Gray::filled(h, w, 1.0);
    let sl = sk::remove_frame_gradient(&mut img, &wt);
    assert!((sl[0][0] - 0.04).abs() < 0.004 && (sl[0][1] + 0.02).abs() < 0.004, "{sl:?}");
    let off = img.at(20, 20)[0] - 0.1;
    assert!(off.abs() < 0.003 && (img.at(180, 280)[0] - 0.1).abs() < 0.003, "plane left: {off}");
    assert!((img.at(80, 200)[0] - 0.1 - 0.05).abs() < 0.004, "galaxy {}", img.at(80, 200)[0]);
    // Only the sky region is corrected.
    let mut half = Img::from_fn(h, w, |y, x| { let v = 0.1 + plane(y, x, 0.04, 0.0); [v; 3] });
    let wt2 = Gray::from_fn(h, w, |y, _| if y < h / 2 { 1.0 } else { 0.0 });
    sk::remove_frame_gradient(&mut half, &wt2);
    assert!((half.at(50, 10)[0] - 0.1).abs() < 0.003 && (half.at(150, 10)[0] - (0.1 + plane(150, 10, 0.04, 0.0))).abs() < 1e-6);

    // Stacking: every frame with its own gradient (turning through 90°) and its own sky level.
    let sc = scene(300, 1);
    let imgs: Vec<Img> = (0..8)
        .map(|k| {
            let mut f = frame(&sc, 0, 0.0, 0.0, 200 + k as u64, 1.0, 1.1);
            let phi = k as f64 / 7.0 * std::f64::consts::FRAC_PI_2;
            let (a, b) = (0.03 * phi.cos(), 0.03 * phi.sin());
            for y in 0..H {
                for x in 0..W {
                    let d = (a * ((x as f64 + 0.5) / W as f64 - 0.5) + b * ((y as f64 + 0.5) / H as f64 - 0.5)) as f32 + 0.004 * k as f32;
                    let p = &mut f.px[y * W + x];
                    *p = [p[0] + d, p[1] + d, p[2] + d];
                }
            }
            f
        })
        .collect();
    let (mut cfg, lookup, _dir) = memstack(imgs);
    cfg.sky = line_sky();
    cfg.align_mode = sk::AlignMode::Best;
    cfg.auto_brightness = false;
    let spread = |r: &sk::StackResult| {
        let mut v: Vec<f32> = (5..(0.45 * H as f64) as usize).flat_map(|y| (5..W - 5).map(move |x| (y, x))).map(|(y, x)| r.raw.at(y, x)[1]).filter(|v| *v < 0.07).collect();
        let q = panostitch::image::quantiles(&mut v, &[0.05, 0.95]);
        q[1] - q[0]
    };
    let level = |r: &sk::StackResult| { let mut v: Vec<f32> = (5..60).flat_map(|y| (5..W - 5).map(move |x| (y, x))).map(|(y, x)| r.raw.at(y, x)[1]).collect(); median_of(v.drain(..)) };
    let plain = run(&cfg, &lookup);
    cfg.frame_gradient = true;
    let flat = run(&cfg, &lookup);
    assert!(spread(&flat) < 0.4 * spread(&plain), "spread {} → {}", spread(&plain), spread(&flat));
    assert!(flat.log.iter().any(|l| l.contains("each frame's gradient removed")));
    // Normalisation: the frames are brought to the base frame's sky level (the base is frame 4 of 8).
    let base_level = flat.report.iter().find(|f| f.status == sk::Status::Base).unwrap().level[1];
    for n in [sk::Normalize::Additive, sk::Normalize::Multiplicative, sk::Normalize::AdditiveScaling] {
        cfg.normalize = n;
        let r = run(&cfg, &lookup);
        assert!((level(&r) - base_level).abs() < 0.0015, "{n:?}: level {} vs base {base_level}", level(&r));
        let used: Vec<&sk::FrameInfo> = r.report.iter().filter(|f| f.used()).collect();
        assert!(used.iter().all(|f| (f.level[1] * f.gain[1] + f.offset[1] - base_level).abs() < 1e-4), "{n:?}");
        assert!(r.log.iter().any(|l| l.contains("frames normalised")));
    }
    // Without it the level is the frames' mean: the base is the 4th of 8 frames
    // whose levels rise by 0.004 each, so the mean is 0.002 above it.
    cfg.normalize = sk::Normalize::Off;
    let r = run(&cfg, &lookup);
    assert!(level(&r) - base_level > 0.0012, "off: level {} vs base {base_level}", level(&r));
}

#[test]
fn filling_empty_areas() {
    // A night sky (gradient, stars, noise) with a ground below; the corners and a
    // band at the top are empty, as around a panorama.
    let (h, w) = (240usize, 360usize);
    let mut rng = ChaCha8Rng::seed_from_u64(5);
    let mut img = Img::from_fn(h, w, |y, x| {
        let v = 0.08 + 0.05 * y as f32 / h as f32 + 0.02 * x as f32 / w as f32;
        if y > 180 { [0.02, 0.018, 0.012] } else { [v * 0.9, v, v * 1.1] }
    });
    for p in img.px.iter_mut() {
        let n = 0.004 * randn(&mut rng) as f32;
        *p = [p[0] + n, p[1] + n, p[2] + n];
    }
    for _ in 0..250 {
        let (y, x) = (rng.gen_range(2..178), rng.gen_range(2..w - 2));
        img.px[y * w + x] = [0.9; 3];
    }
    let truth = img.clone();
    let mut cover = panostitch::fill::Cover::new(h, w);
    for y in 0..h {
        for x in 0..w {
            let corner = (x < 80 && y < 60 && (x as f64 / 80.0 + y as f64 / 60.0) < 1.0) || (x > 280 && y > 200);
            cover.set(y, x, !(corner || y < 12));
            if !cover.get(y, x) {
                img.px[y * w + x] = [0.0; 3];
            }
        }
    }
    let holes = h * w - cover.count();
    for work in [400usize, 120] {
        let mut out = img.clone();
        let n = panostitch::fill::complete_img(&mut out, &cover, work);
        assert_eq!(n, holes);
        // Known pixels untouched; filled ones near what was there (level and colour), not black.
        assert!((0..h * w).filter(|&k| cover.get(k / w, k % w)).all(|k| out.px[k] == img.px[k]));
        let region = |y0: usize, y1: usize, x0: usize, x1: usize, im: &Img| {
            let v: Vec<f32> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (y, x))).filter(|&(y, x)| !cover.get(y, x)).map(|(y, x)| im.at(y, x)[1]).collect();
            median_of(v.into_iter())
        };
        let (a, b) = (region(0, 40, 0, 40, &out), region(0, 40, 0, 40, &truth));
        assert!((a - b).abs() < 0.012, "work {work}: top left {a} vs {b}");
        let (a, b) = (region(205, 240, 290, 360, &out), region(205, 240, 290, 360, &truth));
        assert!((a - b).abs() < 0.008, "work {work}: ground {a} vs {b}");
        let (a, b) = (region(0, 12, 100, 300, &out), region(0, 12, 100, 300, &truth));
        assert!((a - b).abs() < 0.012, "work {work}: top band {a} vs {b}");
        // Texture: the fill is not a flat blur (noise like the sky's), and at full size it has stars.
        let tex = |im: &Img| { let v: Vec<f32> = (2..10).flat_map(|y| (100..300).map(move |x| (y, x))).map(|(y, x)| im.at(y, x)[1] - im.at(y, x + 1)[1]).filter(|d| d.abs() < 0.1).collect(); (v.iter().map(|d| d * d).sum::<f32>() / v.len() as f32).sqrt() };
        assert!(tex(&out) > 0.5 * tex(&truth), "work {work}: texture {} vs {}", tex(&out), tex(&truth));
        let stars = (0..h * w).filter(|&k| !cover.get(k / w, k % w) && out.px[k][1] > 0.4).count();
        assert!(stars > 0, "work {work}: no stars in the fill");
    }
    // Nothing to fill, or nothing to fill from.
    let mut full = panostitch::fill::Cover::new(4, 4);
    assert_eq!(panostitch::fill::complete_img(&mut Img::new(4, 4), &full, 100), 0);
    for k in 0..16 { full.set(k / 4, k % 4, true); }
    assert_eq!(panostitch::fill::complete_img(&mut Img::new(4, 4), &full, 100), 0);
}

/// Matches between a row of photos (turned `step`° apart, focal `f`) whose
/// lenses distort as `lens[i]`: the ideal pixel of each sky point, distorted.
fn distorted_row(lens: &[st::Lens], f: f64, step: f64, h: usize, w: usize) -> (Vec<st::M3>, Vec<st::MatchSet>) {
    let n = lens.len();
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let rs: Vec<st::M3> = (0..n).map(|i| rot([0.0, 1.0, 0.0], step * i as f64)).collect();
    let pts = random_sky(40000, 5);
    let seen = |i: usize, d: &V3| -> Option<st::Pt> {
        let p = project(&rs[i], d, f, 4 * h, 4 * w)?; // ideal pixel, anywhere in a big frame
        let q = [p[0] - 1.5 * w as f64, p[1] - 1.5 * h as f64];
        let (x, y) = st::distort(q[0], q[1], c, lens[i]);
        (x >= 1.0 && x <= w as f64 && y >= 1.0 && y <= h as f64).then_some([x, y])
    };
    let mut m = Vec::new();
    for i in 0..n {
        for j in i + 1..(i + 3).min(n) {
            let (mut a, mut b) = (Vec::new(), Vec::new());
            for d in &pts {
                if let (Some(p), Some(q)) = (seen(i, d), seen(j, d)) {
                    a.push(p);
                    b.push(q);
                }
                if a.len() >= 80 {
                    break;
                }
            }
            if a.len() >= 20 {
                m.push((i, j, a, b));
            }
        }
    }
    let k = st::kmat(f, c);
    // Start a little off.
    let g0 = (0..n).map(|i| k * (rot([0.3, 0.5, 0.1], 0.3 * i as f64) * rs[i]).transpose() * st::inv(&k)).collect();
    (g0, m)
}

#[test]
fn refinement_finds_moustache_and_per_image_distortion() {
    let (h, w, f) = (800usize, 1200usize, 1000.0);
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let all: Vec<usize> = (0..5).collect();
    // The inverse round trip of a moustache lens.
    let l = st::Lens::new(-0.12, 0.06, f);
    let (x, y) = st::distort(100.0, 50.0, c, l);
    let (u, v) = st::undistort(x, y, c, l);
    assert!((u - 100.0).abs() < 1e-6 && (v - 50.0).abs() < 1e-6, "round trip {u}, {v}");
    assert!(l.monotonic(0.8) && !st::Lens::new(-0.9, 0.0, f).monotonic(0.8));

    // One moustache lens for all: the polynomial finds k1 and k2; k1 alone cannot fit it.
    let (g0, m) = distorted_row(&[l; 5], f, 25.0, h, w);
    let poly = st::BundleOpts { poly: true, ..Default::default() };
    let b = st::bundle_adjust_with(&g0, &[c; 5], 0, 1030.0, &all, &m, 60, 5.0, 80, true, &poly);
    assert!(b.rms_after < 0.1, "rms {} → {}", b.rms_before, b.rms_after);
    assert!((b.k1 + 0.12).abs() < 0.01 && (b.k2 - 0.06).abs() < 0.015, "k1 {}, k2 {}", b.k1, b.k2);
    assert!((b.f - f).abs() < 3.0, "focal {}", b.f);
    let radial = st::bundle_adjust_with(&g0, &[c; 5], 0, 1030.0, &all, &m, 60, 5.0, 80, true, &st::BundleOpts::default());
    assert!(radial.rms_after > 3.0 * b.rms_after.max(0.02), "k1 alone: rms {}", radial.rms_after);
    assert_eq!(radial.k2, 0.0);
    println!("moustache: rms {:.3} px, k1 {:.4}, k2 {:.4}; k1 alone rms {:.3}", b.rms_after, b.k1, b.k2, radial.rms_after);

    // Two lenses (three photos strongly barrel-distorted, two nearly not): each image its own.
    let (la, lb) = (st::Lens::new(-0.10, 0.0, f), st::Lens::new(-0.02, 0.0, f));
    let (g0, m) = distorted_row(&[la, la, la, lb, lb], f, 25.0, h, w);
    let own = st::BundleOpts { per_image: true, ..Default::default() };
    let b = st::bundle_adjust_with(&g0, &[c; 5], 0, 1030.0, &all, &m, 60, 5.0, 80, true, &own);
    let shared = st::bundle_adjust_with(&g0, &[c; 5], 0, 1030.0, &all, &m, 60, 5.0, 80, true, &st::BundleOpts::default());
    let ks: Vec<f64> = b.lens.iter().map(|l| l.k1).collect();
    println!("per image: rms {:.3} px, k1 {ks:.4?}; shared rms {:.3}", b.rms_after, shared.rms_after);
    assert!(b.rms_after < 0.3 && shared.rms_after > 2.0 * b.rms_after, "rms own {} vs shared {}", b.rms_after, shared.rms_after);
    for (i, want) in [-0.10, -0.10, -0.10, -0.02, -0.02].into_iter().enumerate() {
        assert!((ks[i] - want).abs() < 0.015, "image {i}: k1 {} vs {want}", ks[i]);
    }
    // Rendering uses each image's own lens: a straight-on render of the photo through its lens is the photo again.
    let img = smooth_scene(h / 4, w / 4, 3);
    let lq = st::Lens::new(-0.1, 0.05, f / 4.0);
    let cq = [(img.w as f64 + 1.0) / 2.0, (img.h as f64 + 1.0) / 2.0];
    let cp = st::ComposeParams { projection: st::Projection::Rectilinear, f: f / 4.0, c: cq, blend: st::Blend::Average, lenses: vec![lq], ..Default::default() };
    let box_ = st::bounds(&[st::outline(&st::M3::identity(), img.h, img.w, cp.projection, cp.f, cq, cp.lens(0), 24)]);
    let (out, _) = st::compose(&[&img], &[st::M3::identity()], box_, &cp, &[1.0], &[1.0], &[(img.h, img.w)], false);
    assert!(out.w > img.w, "the undistorted photo is wider ({} vs {})", out.w, img.w);
}

#[test]
fn auto_horizon_levels_a_tilted_panorama() {
    use st::horizon;
    // A horizon 2° above eye level, seen by a tripod that leaned 5° forward and 3° to the side.
    let tilt = rot([1.0, 0.0, 0.0], 5.0) * rot([0.0, 0.0, 1.0], -3.0);
    let world = |az: f64, el: f64| V3::new(el.to_radians().cos() * az.to_radians().sin(), -el.to_radians().sin(), el.to_radians().cos() * az.to_radians().cos());
    for (proj, span) in [(st::Projection::Cylindrical, 120.0), (st::Projection::Spherical, 360.0), (st::Projection::Mercator, 200.0)] {
        let dirs: Vec<V3> = (0..60).map(|k| tilt * world(-span / 2.0 + span * k as f64 / 59.0, 2.0)).collect();
        let (p, r, before, after) = horizon::level(&dirs, proj, 0.0, 0.0, 0.0).unwrap();
        println!("{proj:?} {span}°: pitch {p:.2}, roll {r:.2}; spread {before:.2}° → {after:.4}°");
        assert!(before > 1.0 && after < 0.02, "{proj:?}: spread {before} → {after}");
        // Straight in the picture: every point at the same v.
        let f = 1000.0;
        let vs: Vec<f64> = dirs.iter().filter_map(|d| horizon::to_output(d, proj, f, 0.0, p, r)).map(|q| q[1]).collect();
        let (lo, hi) = vs.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(hi - lo < 0.5, "{proj:?}: v from {lo} to {hi}");
    }
    // Rectilinear: straight only at eye level, so the horizon ends on the middle line.
    let dirs: Vec<V3> = (0..30).map(|k| tilt * world(-40.0 + 80.0 * k as f64 / 29.0, 0.0)).collect();
    let (p, r, _, after) = horizon::level(&dirs, st::Projection::Rectilinear, 0.0, 0.0, 0.0).unwrap();
    assert!(after < 0.02, "rectilinear spread {after}");
    for d in &dirs {
        let q = horizon::to_output(d, st::Projection::Rectilinear, 1000.0, 0.0, p, r).unwrap();
        assert!(q[1].abs() < 0.5, "rectilinear horizon at v = {}", q[1]);
    }
    assert!(horizon::supported(st::Projection::Mercator) && !horizon::supported(st::Projection::LittlePlanet) && !horizon::supported(st::Projection::Tunnel));

    // Finding it: bright sky over dark ground with a wavy horizon and a "tree".
    let (h, w) = (300usize, 900usize);
    let truth = |x: f64| 150.0 + 30.0 * (x / w as f64 * std::f64::consts::TAU).sin();
    let mut rng = ChaCha8Rng::seed_from_u64(9);
    let img = Img::from_vec(h, w, (0..h * w).map(|k| {
        let (y, x) = ((k / w) as f64, (k % w) as f64);
        let tree = (x - 400.0).abs() < 15.0 && y > truth(x) - 60.0;
        let v = if y < truth(x) && !tree { 0.30 - 0.0003 * y } else { 0.04 };
        let v = (v + 0.01 * randn(&mut rng)) as f32;
        [v, v, v]
    }).collect());
    let pts = horizon::find_horizon(&img, None);
    assert!(pts.len() > 100, "{} points", pts.len());
    let (curve, spread, wts) = horizon::fit_curve(&pts, 4).unwrap();
    let err: Vec<f64> = (0..w).step_by(10).map(|x| (curve.at(x as f64) - truth(x as f64)).abs()).collect();
    let worst = err.iter().copied().fold(0.0, f64::max);
    // The points kept lie on the true horizon (not on the tree).
    let kept: Vec<&st::Pt> = pts.iter().zip(&wts).filter(|(_, &w)| w > 0.5).map(|(p, _)| p).collect();
    let off = kept.iter().map(|p| (p[1] - truth(p[0])).abs()).fold(0.0, f64::max);
    println!("horizon found: {} points ({} kept), spread {spread:.1} px, curve within {worst:.1} px, kept points within {off:.1} px", pts.len(), kept.len());
    assert!(worst < 12.0, "curve off by {worst} px");
    assert!(off < 5.0 && kept.len() > pts.len() * 3 / 4, "kept {} points, off by {off} px", kept.len());
}

#[test]
fn medians_survive_values_that_are_not_numbers() {
    // A NaN must not stop the sort (Rust panics on a comparison that is not a total order).
    let mut v: Vec<f32> = (0..40).map(|k| if k % 7 == 0 { f32::NAN } else { k as f32 }).collect();
    let m = panostitch::image::median(&mut v);
    assert!(m.is_finite(), "median {m}");
    let m2 = panostitch::image::median_f64((0..40).map(|k| if k % 5 == 0 { f64::NAN } else { k as f64 }));
    assert!(m2.is_finite(), "median {m2}");
}

#[test]
fn a_photo_of_the_zenith_fills_the_top_of_a_spherical_panorama() {
    // A camera looking straight up (and one straight down): in a spherical (or
    // cylindrical) panorama its outline goes round the pole, and the cap
    // between its edge and the pole must be rendered too (it was left empty).
    let (h, w, f) = (300usize, 300usize, 250.0);
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let img = smooth_scene(h, w, 4).map(|p| [p[0] + 0.1, p[1] + 0.1, p[2] + 0.1]);
    for (proj, up) in [(st::Projection::Spherical, 90.0), (st::Projection::Spherical, -90.0), (st::Projection::Mollweide, 90.0), (st::Projection::TransverseCylindrical, 0.0)] {
        // Pointing at the pole: up/down for the others, sideways for the transverse projection.
        let r = if proj == st::Projection::TransverseCylindrical { rot([0.0, 1.0, 0.0], 90.0) } else { rot([1.0, 0.0, 0.0], up) };
        let g = st::kmat(f, c) * r.transpose() * st::inv(&st::kmat(f, c));
        let cp = st::ComposeParams { projection: proj, f, c, blend: st::Blend::Average, scale: 0.5, ..Default::default() };
        let o = st::outline(&g, h, w, proj, f, c, cp.lens(0), 24);
        let b = st::bounds(&[o]);
        let (out, cov) = st::compose(&[&img], &[g], b, &cp, &[1.0], &[1.0], &[(h, w)], true);
        let cov = cov.unwrap();
        // The rows (columns, transverse) at the pole side are covered across.
        // The pole's side: the one where the box reaches the projection's limit.
        let (la, lb) = (4.0 * f, if proj == st::Projection::Spherical { std::f64::consts::FRAC_PI_2 * f } else { 2f64.sqrt() * f });
        let (lines, len, at): (usize, usize, Box<dyn Fn(usize, usize) -> bool>) = if proj == st::Projection::TransverseCylindrical {
            assert!(b.0 <= -la + 1.0 || b.1 >= la - 1.0, "{proj:?}: box {b:?} does not reach the pole");
            let left = b.0 <= -la + 1.0;
            (out.w, out.h, Box::new(move |line, k| cov.v[k * cov.w + if left { line } else { cov.w - 1 - line }]))
        } else {
            assert!(b.2 <= -lb + 1.0 || b.3 >= lb - 1.0, "{proj:?}: box {b:?} does not reach the pole");
            let top = b.2 <= -lb + 1.0;
            (out.h, out.w, Box::new(move |line, k| cov.v[(if top { line } else { cov.h - 1 - line }) * cov.w + k]))
        };
        let edge: Vec<f64> = (0..3.min(lines)).map(|l| (0..len).filter(|&k| at(l, k)).count() as f64 / len as f64).collect();
        println!("{proj:?} {up}: box {b:?}, pole side covered {edge:.2?}");
        // Mollweide's pole is a single point: only its middle is covered there.
        let need = if proj == st::Projection::Mollweide { 0.02 } else { 0.95 };
        assert!(edge.iter().all(|&e| e >= need), "{proj:?} {up}: the pole side is covered only {edge:?}");
    }
}

#[test]
fn sky_grid_mapping_and_names() {
    use st::sky_grid;
    // Names: bright stars and the Messier objects, with sizes for the deep-sky ones.
    let l = sky_grid::labels();
    assert!(l.iter().filter(|x| x.star).count() > 150 && l.iter().filter(|x| !x.star).count() > 100);
    let vega = l.iter().find(|x| x.name == "Vega").expect("Vega");
    let (ra, dec) = st::solve::radec(&vega.dir);
    assert!((ra - 279.23).abs() < 0.1 && (dec - 38.78).abs() < 0.1, "Vega at {ra}, {dec}");
    let m31 = l.iter().find(|x| x.name.starts_with("M31 ")).expect("M31");
    assert!(m31.size > 2.0, "M31 {}°", m31.size);
    // The rotation from pairs of sky and panorama directions, with a little noise.
    let r = rot([0.2, 0.9, -0.4], 73.0);
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    let pairs: Vec<(V3, V3)> = random_sky(300, 8).into_iter().map(|d| (d, r * d + V3::new(randn(&mut rng), randn(&mut rng), randn(&mut rng)) * 1e-3)).collect();
    let (fit, spread) = sky_grid::fit(&pairs).unwrap();
    assert!((fit - r).norm() < 2e-3 && spread < 0.15, "spread {spread}°");
    assert_eq!(sky_grid::ra_text(50.0), "3h20m");
    assert_eq!(sky_grid::ra_text(90.0), "6h");
    assert_eq!(sky_grid::dec_text(-12.5), "−12.5°");
    assert_eq!(sky_grid::dec_text(30.0), "+30°");
    assert!(sky_grid::grid_step(3.0) == 5.0 && sky_grid::ra_step(3.0) == 5.0);
}

//! Post-processing engine (src/finish.rs): geometry, stretch curves, white
//! balance, curves, and that the preview copy matches the full image.

use panostitch::finish::{self as fin, Display, FinishParams, Measured, StretchMode};
use panostitch::image::{Image, Img};

/// A smooth test scene with a bluish cast and some "stars".
fn scene(h: usize, w: usize) -> Img {
    Image::from_fn(h, w, |y, x| {
        let g = 0.05 + 0.3 * (x as f32 / w as f32) + 0.1 * (y as f32 / h as f32);
        let star = if (x * 7 + y * 13) % 97 == 0 { 0.5 } else { 0.0 };
        [g * 0.8 + star, g * 0.9 + star, g * 1.2 + star]
    })
}

fn rising_0_to_1(f: impl Fn(f64) -> f64) {
    let mut last = -1.0;
    for i in 0..=1000 {
        let v = f(i as f64 / 1000.0);
        assert!(v >= last - 1e-9, "not rising at {i}: {v} < {last}");
        last = v;
    }
    assert!(f(0.0).abs() < 1e-6, "f(0) = {}", f(0.0));
    assert!((f(1.0) - 1.0).abs() < 1e-6, "f(1) = {}", f(1.0));
}

#[test]
fn turning_and_cropping() {
    let img = scene(60, 90);
    let mut p = FinishParams::default();
    p.quarter = 1;
    let t = fin::geometry(&img, &p);
    assert_eq!((t.w, t.h), (60, 90));
    // Turned clockwise: the old bottom-left corner is now top-left.
    assert_eq!(t.at(0, 0), img.at(59, 0));
    p.quarter = 0;
    p.crop = [0.1, 0.0, 0.1, 0.4];
    let c = fin::geometry(&img, &p);
    assert_eq!((c.w, c.h), (72, 36));
    assert_eq!(fin::output_size(90, 60, &p), (72, 36));
    // A fine rotation with the empty corners cut away leaves no black pixels.
    p.crop = [0.0; 4];
    p.angle = 7.0;
    let r = fin::geometry(&img, &p);
    assert!(r.px.iter().all(|c| c[1] > 0.0), "black corner left after auto crop");
    let (ow, oh) = fin::output_size(90, 60, &p);
    assert!((ow as i64 - r.w as i64).abs() <= 1 && (oh as i64 - r.h as i64).abs() <= 1, "{ow}×{oh} vs {}×{}", r.w, r.h);
}

#[test]
fn stretch_curves_rise_and_invert() {
    for b in [-1.0, -0.5, 0.0, 2.0, 10.0] {
        rising_0_to_1(fin::ghs(3.0, b, 0.1, 0.0, 1.0));
        rising_0_to_1(fin::ghs(5.0, b, 0.2, 0.05, 0.8));
    }
    // GHS stretches: the dark end is lifted.
    assert!(fin::ghs(3.0, 0.0, 0.0, 0.0, 1.0)(0.05) > 0.2);
    // A LUT and its inverse undo each other.
    let fwd = fin::Lut::new(fin::ghs(4.0, 1.0, 0.1, 0.0, 1.0));
    let inv = fwd.inverse();
    for i in 1..20 {
        let x = i as f32 / 20.0;
        assert!((inv.at(fwd.at(x)) - x).abs() < 0.01, "inverse at {x}");
    }
    // Asinh with "keep star colours" keeps the colour ratios.
    let mut p = FinishParams::for_input(true);
    p.stretch = StretchMode::Asinh;
    p.asinh = 200.0;
    let img = Image::filled(4, 4, [0.01f32, 0.02, 0.04]);
    let out = fin::process(&img, &p, 1.0, &mut Measured::default());
    let c = out.at(0, 0);
    assert!(c[2] > 0.1, "not stretched: {c:?}");
    assert!((c[1] / c[0] - 2.0).abs() < 0.05 && (c[2] / c[0] - 4.0).abs() < 0.1, "colour changed: {c:?}");
    // Inverse asinh darkens the middle instead.
    p.inverse = true;
    p.keep_colour = false;
    let mid = Image::filled(2, 2, [0.5f32; 3]);
    assert!(fin::process(&mid, &p, 1.0, &mut Measured::default()).at(0, 0)[0] < 0.2);
}

#[test]
fn curves_tone_and_white_balance() {
    // A curve through its points, no overshoot between them.
    let pts = vec![[0.0, 0.0], [0.25, 0.4], [0.75, 0.8], [1.0, 1.0]];
    let f = fin::curve_fn(&pts);
    for q in &pts {
        assert!((f(q[0]) - q[1]).abs() < 1e-9);
    }
    rising_0_to_1(&f);
    // Tone sliders at 0 change nothing; shadows lift the dark end.
    let p = FinishParams::default();
    let id = fin::tone_fn(&p);
    assert!((id(0.3) - 0.3).abs() < 1e-12);
    let mut p2 = p.clone();
    p2.shadows = 1.0;
    assert!(fin::tone_fn(&p2)(0.25) > 0.3);
    p2.shadows = -1.0;
    rising_0_to_1(fin::tone_fn(&p2));
    // A grey sample neutralises a colour cast: on a finished (sRGB) image and on linear data
    // (white balance runs before the stretch there).
    let neutral = |c: [f32; 3]| (c[0] - c[1]).abs() < 0.01 && (c[1] - c[2]).abs() < 0.01;
    for linear in [false, true] {
        let mut p = FinishParams::for_input(linear);
        p.unlinked = false;
        let img = Image::filled(8, 8, if linear { [0.02f32, 0.03, 0.05] } else { [0.3f32, 0.4, 0.6] });
        let prepared = fin::prepare(&img, &p);
        p.wb = fin::sample_wb(&prepared, (2.0, 2.0), (6.0, 6.0), !linear).expect("sample");
        let c = fin::process(&img, &p, 1.0, &mut Measured::default()).at(4, 4);
        assert!(neutral(c), "linear {linear}: not neutral: {c:?}");
    }
    // Outside the image: no sample.
    assert!(fin::sample_wb(&Image::filled(4, 4, [0.5f32; 3]), (10.0, 10.0), (12.0, 12.0), true).is_none());
    // Compare's starting point keeps the turn and the stretch, drops the rest.
    let mut p = FinishParams::for_input(true);
    p.quarter = 1;
    p.stretch = StretchMode::Ghs;
    p.ghs_sp = 0.1;
    p.saturation = 1.5;
    p.wb = [1.2, 1.0, 0.8];
    let sp = fin::starting_point(&p);
    assert_eq!((sp.quarter, sp.stretch, sp.ghs_sp, sp.saturation, sp.wb), (1, StretchMode::Ghs, 0.1, 1.0, [1.0; 3]));
}

#[test]
fn preview_matches_full_image() {
    let full = scene(400, 600);
    let scale = 0.25;
    let small = panostitch::image::resize(&full, scale);
    let mut p = FinishParams::for_input(true);
    p.neutral_bg = true;
    p.contrast = 0.3;
    p.saturation = 1.3;
    p.noise = 0.5;
    let mut m = Measured::default();
    let a = fin::process(&small, &p, scale, &mut m);
    assert!(m.stretch.is_some() && m.bg.is_some());
    let b = fin::process(&full, &p, 1.0, &mut m);
    let b_small = panostitch::image::resize(&b, scale);
    let mean = |img: &Img| img.px.iter().map(|c| (c[0] + c[1] + c[2]) as f64 / 3.0).sum::<f64>() / img.len() as f64;
    assert!((mean(&a) - mean(&b_small)).abs() < 0.02, "preview {} vs full {}", mean(&a), mean(&b_small));
    // Export size: longest side as asked.
    p.long_edge = 300;
    let e = fin::export_size(&b, &p);
    assert_eq!(e.w.max(e.h), 300);
}

#[test]
fn display_modes_change_only_the_view() {
    let img = scene(50, 80).map(|c| c.map(|v| v.min(1.0) * 0.2));
    for d in Display::ALL {
        let v = fin::display(&img, d);
        assert_eq!((v.w, v.h), (img.w, img.h));
        assert!(v.px.iter().all(|c| c.iter().all(|x| (0.0..=1.0).contains(x))), "{d:?} out of range");
    }
    assert_eq!(fin::display(&img, Display::Linear), img);
    let mean = |img: &Img| img.px.iter().map(|c| c[1] as f64).sum::<f64>() / img.len() as f64;
    assert!(mean(&fin::display(&img, Display::AutoStretch)) > mean(&img));
}

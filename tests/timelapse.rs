//! Time-lapse engine (src/timelapse.rs): frame shape, camera move, deflicker,
//! and writing GIF, numbered images and (with ffmpeg) MP4.

use panostitch::image::{Image, Img};
use panostitch::io;
use panostitch::timelapse::{self as tl, Deflicker, Format, Pan, Resolution, Settings};

/// `n` frames of 160 × 120: a grey scene whose brightness flickers, a "star"
/// that moves four pixels to the right per frame, and a still vertical line at x = 80.
fn frames(dir: &std::path::Path, n: usize) -> Vec<String> {
    (0..n).map(|i| {
        let flicker = if i % 2 == 0 { 1.0 } else { 1.3 };
        let img: Img = Image::from_fn(120, 160, |y, x| {
            if (y == 30 && x == 20 + i * 4) || x == 80 { [1.0; 3] } else { [0.25 * flicker; 3] }
        });
        let p = dir.join(format!("f{i:03}.png")).to_string_lossy().to_string();
        io::save_image(&p, &img, 95).unwrap();
        p
    }).collect()
}

fn no_progress(_: &str, _: f64, _: f64) -> Result<(), String> {
    Ok(())
}

#[test]
fn frame_shape_and_size() {
    let s = Settings { resolution: Resolution::Hd, ..Settings::default() };
    // A 3:2 frame cut to 16:9 and scaled to 720p.
    assert_eq!(tl::video_size(6000, 4000, &s), (1280, 720));
    // Rounding the cut does not cost a line: exactly 1920 × 1080.
    assert_eq!(tl::video_size(3016, 2008, &Settings { resolution: Resolution::FullHd, ..s.clone() }), (1920, 1080));
    // Never scaled up; always even.
    assert_eq!(tl::video_size(801, 451, &Settings { resolution: Resolution::FullHd, ..s.clone() }), (800, 450));
    // Portrait frames make a portrait video.
    let (w, h) = tl::video_size(4000, 6000, &s);
    assert!(h > w && h == 1280, "{w}×{h}");
    // The zoom cuts more away at the end.
    let s = Settings { zoom: 20.0, ..s };
    let (a, b) = (tl::frame_crop(6000, 4000, &s, 0.0), tl::frame_crop(6000, 4000, &s, 1.0));
    assert!(b.crop[0] > a.crop[0] && b.crop[1] > a.crop[1]);
    // Slider move: a view 10 % smaller travels from the left edge to the right edge.
    let s = Settings { zoom: 0.0, crop_16_9: false, pan: 10.0, pan_dir: Pan::LeftToRight, ..s };
    let (a, b) = (tl::frame_crop(6000, 4000, &s, 0.0).crop, tl::frame_crop(6000, 4000, &s, 1.0).crop);
    assert!(a[0].abs() < 1e-9 && b[2].abs() < 1e-9, "{a:?} {b:?}");
    assert!((1.0 - a[0] - a[2] - 1.0 / 1.1).abs() < 1e-9, "view width {a:?}");
    assert!((a[1] - b[1]).abs() < 1e-9, "moved up or down");
    // Gentle start: a quarter of the way in, less than a quarter of the move is done.
    let q = tl::frame_crop(6000, 4000, &s, 0.25).crop[0] / b[0];
    assert!(q < 0.2, "{q}");
    let lin = tl::frame_crop(6000, 4000, &Settings { ease: false, ..s.clone() }, 0.25).crop[0] / b[0];
    assert!((lin - 0.25).abs() < 1e-9);
    let up = Settings { pan_dir: Pan::Up, ..s };
    assert!(tl::frame_crop(6000, 4000, &up, 1.0).crop[1] < tl::frame_crop(6000, 4000, &up, 0.0).crop[1]);
}

#[test]
fn deflicker_evens_out_brightness() {
    let levels: Vec<f64> = (0..20).map(|i| if i % 2 == 0 { 0.0 } else { 0.3 }).collect();
    let g = tl::deflicker_gains(&levels, 5, Deflicker::Light);
    let after: Vec<f64> = levels.iter().zip(&g).map(|(l, g)| l + (*g as f64).ln()).collect();
    let spread = |v: &[f64]| v[2..18].iter().cloned().fold(f64::MIN, f64::max) - v[2..18].iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread(&after) < 0.1 * spread(&levels), "still flickers: {after:?}");
    // A slow change (dusk) stays.
    let ramp: Vec<f64> = (0..20).map(|i| i as f64 * 0.1).collect();
    assert!(tl::deflicker_gains(&ramp, 7, Deflicker::Strong).iter().all(|g| (g - 1.0).abs() < 1e-4));
    assert!(tl::deflicker_gains(&levels, 5, Deflicker::Off).iter().all(|g| *g == 1.0));
}

#[test]
fn gif_and_numbered_images() {
    let dir = tempfile::tempdir().unwrap();
    let fr = frames(dir.path(), 6);
    // Numbered PNGs with a slider move, the last frame held and the way back.
    let out = dir.path().join("move.png").to_string_lossy().to_string();
    let s = Settings { frames: fr.clone(), deflicker: Deflicker::Off, format: Format::Sequence, resolution: Resolution::Frames, crop_16_9: false, fps: 2, hold_last: 1.0, bounce: true, pan: 20.0, ease: false, out, ..Settings::default() };
    let d = tl::make(&s, &no_progress).unwrap();
    assert_eq!((d.w, d.h, d.frames), (132, 100, 6));
    assert_eq!(d.video_frames, 6 + 2 + 4);
    assert_eq!(d.files, d.video_frames);
    // The still line drifts left as the view moves right: at x ≈ 80 at the start, ≈ 80 − 26.7 at the end.
    let line_x = |k: usize| {
        let img = io::load_any(&dir.path().join(format!("move_{k:04}.png")).to_string_lossy(), &Default::default()).unwrap();
        let row = 80;
        (0..img.w).max_by(|&a, &b| img.at(row, a)[0].total_cmp(&img.at(row, b)[0])).unwrap() as f64
    };
    let (x1, x6) = (line_x(1), line_x(6));
    assert!((x1 - 80.0 * 132.0 / 133.3).abs() <= 1.5 && (x6 - (80.0 - 26.7) * 132.0 / 133.3).abs() <= 1.5, "line at {x1} → {x6}");
    // Deflicker: the frames come out about equally bright.
    let mean = |img: &Img| img.px.iter().map(|c| c[1] as f64).sum::<f64>() / img.len() as f64;
    let s2 = Settings { deflicker: Deflicker::Light, deflicker_window: 3, hold_last: 0.0, bounce: false, pan: 0.0, out: dir.path().join("even.png").to_string_lossy().to_string(), ..s.clone() };
    tl::make(&s2, &no_progress).unwrap();
    let m: Vec<f64> = (2..=5).map(|i| mean(&io::load_any(&dir.path().join(format!("even_{i:04}.png")).to_string_lossy(), &Default::default()).unwrap())).collect();
    assert!((m[0] - m[1]).abs() < 0.03 && (m[1] - m[2]).abs() < 0.03, "flicker left: {m:?}");
    // GIF.
    let gif = dir.path().join("t.gif").to_string_lossy().to_string();
    let d = tl::make(&Settings { format: Format::Gif, out: gif.clone(), ..s2.clone() }, &no_progress).unwrap();
    assert_eq!(d.video_frames, 6);
    assert!(std::fs::metadata(&gif).unwrap().len() > 1000);
    // Cancelling removes what was written.
    let gif2 = dir.path().join("c.gif").to_string_lossy().to_string();
    let r = tl::make(&Settings { format: Format::Gif, out: gif2.clone(), ..s2 }, &|m: &str, _, _| if m.starts_with("Frame 2") { Err("Cancelled".into()) } else { Ok(()) });
    assert_eq!(r.unwrap_err(), "Cancelled");
    assert!(!std::path::Path::new(&gif2).exists());
}

#[test]
fn mp4_with_ffmpeg() {
    if tl::find_ffmpeg().is_none() {
        eprintln!("ffmpeg not found: MP4 not tested");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let fr = frames(dir.path(), 8);
    let out = dir.path().join("t.mp4").to_string_lossy().to_string();
    let d = tl::make(&Settings { frames: fr, format: Format::Mp4, resolution: Resolution::Hd, out: out.clone(), ..Settings::default() }, &no_progress).unwrap();
    assert_eq!((d.w, d.h), (160, 90));
    assert!(std::fs::metadata(&out).unwrap().len() > 500);
}

#[test]
fn deflicker_by_colour_and_region() {
    // 12 frames: the left half's colour jumps (warm, then cool) frame by frame; the right half stays.
    let frame = |i: usize| -> Img {
        Image::from_fn(60, 90, |_, x| {
            if x < 45 {
                if i.is_multiple_of(2) { [0.30, 0.20, 0.15] } else { [0.18, 0.20, 0.32] }
            } else {
                [0.2, 0.2, 0.2]
            }
        })
    };
    let (gx, gy) = tl::grid(90, 60, true);
    assert_eq!((gx, gy), (6, 4));
    let lv: Vec<_> = (0..12).map(|i| tl::levels(&frame(i), true, gx, gy, true)).collect();
    let gains = tl::deflicker(&lv, gx, gy, 5, Deflicker::Light);
    let out: Vec<Img> = (0..12).map(|i| tl::apply_gains(frame(i), &gains[i], [0.0; 4], true)).collect();
    // In the middle of the sequence the left half keeps one colour; the right half is untouched.
    let (a, b) = (out[5].at(30, 10), out[6].at(30, 10));
    for k in 0..3 {
        assert!((a[k] - b[k]).abs() < 0.02, "colour still jumps: {a:?} vs {b:?}");
    }
    assert!((out[6].at(30, 85)[0] - 0.2).abs() < 0.01, "right half changed: {:?}", out[6].at(30, 85));
    // Brightness only: the colour jump stays (the two colours have about the same brightness).
    let lv: Vec<_> = (0..12).map(|i| tl::levels(&frame(i), true, 1, 1, false)).collect();
    let g = tl::deflicker(&lv, 1, 1, 5, Deflicker::Light);
    let (a, b) = (tl::apply_gains(frame(5), &g[5], [0.0; 4], true).at(30, 10), tl::apply_gains(frame(6), &g[6], [0.0; 4], true).at(30, 10));
    assert!((a[2] - b[2]).abs() > 0.1, "{a:?} vs {b:?}");
}

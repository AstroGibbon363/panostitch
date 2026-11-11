//! Everything after combining: the foreground image, light pollution,
//! background extraction, colour calibration, colour noise, star enhancement,
//! brightness, HDR and tone.

use super::{detect_stars, linear_to_srgb, ColorSpace, Foreground, Pollution, StackConfig, Tone};
use crate::image::{blur, luminance, median, median_of, quantiles, resize, Gray, Img, Rgb};
use crate::io::basename;
use crate::stitcher::{auto_stretch, hist_stretch};
use rayon::prelude::*;

// --------------------------------------------------------------------------
// Foreground image
// --------------------------------------------------------------------------

/// The foreground image at the stack's size (same framing; scaled if only the resolution differs).
pub fn load_foreground(cfg: &StackConfig, load: super::Loader, h: usize, w: usize) -> Result<Img, String> {
    let mut img = load(&cfg.foreground.path)?;
    if cfg.merge4 {
        img = super::merge4(&img);
    }
    let (fh, fw) = (img.h, img.w);
    if (fh, fw) == (h, w) {
        return Ok(img);
    }
    let ar = w as f64 / h as f64;
    if (fw as f64 / fh as f64 - ar).abs() >= 0.01 * ar {
        return Err(format!("The foreground image is {fw}×{fh}, not the shape of the light frames ({w}×{h})."));
    }
    let r = resize(&img, h as f64 / fh as f64);
    Ok(Img::from_fn(h, w, |y, x| r.at(y.min(r.h - 1), x.min(r.w - 1))))
}

/// Sky from the stack, ground from `fg` (both linear), across the sky weight W.
/// The foreground is scaled to the brightness of the stacked ground (`match`)
/// and by 2^ev. Returns (image, scale used).
pub fn blend_foreground(stacked: &Img, fg: &Img, wt: &Gray, f: &Foreground) -> (Img, f64) {
    let mut k = 2f64.powf(f.ev);
    let step = wt.step_for(200_000);
    let gnd: Vec<usize> = (0..wt.len()).step_by(step).filter(|&i| wt.px[i] <= 0.01).collect();
    if f.match_brightness && gnd.len() >= 50 {
        let a = median_of(gnd.iter().map(|&i| luminance(stacked.px[i]))) as f64;
        let b = median_of(gnd.iter().map(|&i| luminance(fg.px[i]))) as f64;
        if a > 0.0 && b > 0.0 {
            k *= a / b;
        }
    }
    let kf = k as f32;
    let px: Vec<Rgb> = (0..stacked.len())
        .into_par_iter()
        .map(|i| {
            let (s, g, m) = (stacked.px[i], fg.px[i], wt.px[i]);
            [m * s[0] + (1.0 - m) * kf * g[0], m * s[1] + (1.0 - m) * kf * g[1], m * s[2] + (1.0 - m) * kf * g[2]]
        })
        .collect();
    (Img::from_vec(stacked.h, stacked.w, px), k)
}

// --------------------------------------------------------------------------
// Light pollution
// --------------------------------------------------------------------------

/// Per-channel level (quantile q) of the sky on `n` tiles across: (levels (NaN where no sky), tile size).
fn tile_levels(img: &Img, wt: &Gray, n: usize, q: f64) -> (Vec<[f32; 3]>, usize, usize, usize) {
    let (h, w) = (img.h, img.w);
    let t = (w / n).max(8);
    let (ny, nx) = (h.div_ceil(t), w.div_ceil(t));
    let mut l = vec![[f32::NAN; 3]; ny * nx];
    l.par_chunks_mut(nx).enumerate().for_each(|(i, row)| {
        for (j, cell) in row.iter_mut().enumerate() {
            let mut px: Vec<Rgb> = Vec::new();
            let (mut ny_, mut nx_) = (0usize, 0usize);
            let mut y = i * t;
            while y < ((i + 1) * t).min(h) {
                ny_ += 1;
                let mut x = j * t;
                nx_ = 0;
                while x < ((j + 1) * t).min(w) {
                    nx_ += 1;
                    if wt.at(y, x) >= 0.99 {
                        px.push(img.at(y, x));
                    }
                    x += 2;
                }
                y += 2;
            }
            if (px.len() as f64) < 0.3 * (ny_ * nx_) as f64 {
                continue;
            }
            for c in 0..3 {
                let mut v: Vec<f32> = px.iter().map(|p| p[c]).collect();
                cell[c] = quantiles(&mut v, &[q])[0];
            }
        }
    });
    (l, ny, nx, t)
}

/// Gaussian smoothing of a tile grid that ignores (and fills) NaN tiles.
fn smooth_tiles(l: &[[f32; 3]], ny: usize, nx: usize, sigma: f64) -> Vec<[f32; 3]> {
    let r = (3.0 * sigma).ceil() as i64;
    let mut out = vec![[f32::NAN; 3]; ny * nx];
    for c in 0..3 {
        for i in 0..ny {
            for j in 0..nx {
                let (mut s, mut sw) = (0.0f64, 0.0f64);
                for di in -r..=r {
                    for dj in -r..=r {
                        let (ii, jj) = (i as i64 + di, j as i64 + dj);
                        if ii < 0 || jj < 0 || ii >= ny as i64 || jj >= nx as i64 {
                            continue;
                        }
                        let v = l[ii as usize * nx + jj as usize][c];
                        if v.is_nan() {
                            continue;
                        }
                        let wk = (-((di * di + dj * dj) as f64) / (2.0 * sigma * sigma)).exp();
                        s += wk * v as f64;
                        sw += wk;
                    }
                }
                out[i * nx + j][c] = if sw > 1e-6 { (s / sw) as f32 } else { f32::NAN };
            }
        }
    }
    out
}

/// Tile grid → full-size image (bilinear between tile centres).
fn upsample(l: &[[f32; 3]], ny: usize, nx: usize, t: usize, h: usize, w: usize) -> Img {
    let planes: Vec<Gray> = (0..3).map(|c| Gray::from_vec(ny, nx, l.iter().map(|v| v[c]).collect())).collect();
    Img::from_fn(h, w, |y, x| {
        let (yy, xx) = ((y as f64 + 0.5) / t as f64 + 0.5, (x as f64 + 0.5) / t as f64 + 0.5);
        [crate::image::bilinear(&planes[0], yy, xx), crate::image::bilinear(&planes[1], yy, xx), crate::image::bilinear(&planes[2], yy, xx)]
    })
}

fn uneven_background(img: &Img, wt: &Gray, tiles: usize) -> Option<Img> {
    let (mut l, ny, nx, t) = tile_levels(img, wt, tiles, 0.3);
    if l.iter().all(|v| v[0].is_nan()) {
        return None;
    }
    let mut sigma = 1.5;
    while l.iter().any(|v| v.iter().any(|x| x.is_nan())) {
        // fill the foreground from the sky
        let f = smooth_tiles(&l, ny, nx, sigma);
        for (a, b) in l.iter_mut().zip(f.iter()) {
            for c in 0..3 {
                if a[c].is_nan() {
                    a[c] = b[c];
                }
            }
        }
        sigma *= 2.0;
        if sigma > 1e6 {
            break;
        }
    }
    // 3×3 median: ignores single odd tiles
    let mut m = vec![[0f32; 3]; ny * nx];
    for c in 0..3 {
        for i in 0..ny {
            for j in 0..nx {
                let mut v: Vec<f32> = Vec::new();
                for ii in i.saturating_sub(1)..=(i + 1).min(ny - 1) {
                    for jj in j.saturating_sub(1)..=(j + 1).min(nx - 1) {
                        v.push(l[ii * nx + jj][c]);
                    }
                }
                m[i * nx + j][c] = median(&mut v);
            }
        }
    }
    Some(upsample(&smooth_tiles(&m, ny, nx, 1.0), ny, nx, t, img.h, img.w))
}

fn poly_terms(u: f64, v: f64, degree: usize) -> Vec<f64> {
    let mut t = Vec::new();
    for a in 0..=degree {
        for b in 0..=(degree - a) {
            t.push(u.powi(a as i32) * v.powi(b as i32));
        }
    }
    t
}

fn poly_background(img: &Img, wt: &Gray, degree: usize) -> Option<Img> {
    let (l, ny, nx, t) = tile_levels(img, wt, 32, 0.5);
    let (h, w) = (img.h, img.w);
    let pts: Vec<(usize, usize)> = (0..nx).flat_map(|j| (0..ny).map(move |i| (i, j))).filter(|&(i, j)| !l[i * nx + j][0].is_nan()).collect();
    if pts.len() < 6 {
        return None;
    }
    let nu = |x: f64| 2.0 * (x - 0.5) / w as f64 - 1.0;
    let nv = |y: f64| 2.0 * (y - 0.5) / h as f64 - 1.0;
    let rows: Vec<Vec<f64>> = pts.iter().map(|&(i, j)| poly_terms(nu((j as f64 + 0.5) * t as f64), nv((i as f64 + 0.5) * t as f64), degree)).collect();
    let nt = rows[0].len();
    let solve = |keep: &[bool], z: &[f64]| -> Option<Vec<f64>> {
        let n = keep.iter().filter(|k| **k).count();
        let mut a = nalgebra::DMatrix::<f64>::zeros(n, nt);
        let mut b = nalgebra::DVector::<f64>::zeros(n);
        let mut r = 0;
        for (k, row) in rows.iter().enumerate() {
            if !keep[k] {
                continue;
            }
            for c in 0..nt {
                a[(r, c)] = row[c];
            }
            b[r] = z[k];
            r += 1;
        }
        a.svd(true, true).solve(&b, 1e-10).ok().map(|v| v.iter().copied().collect())
    };
    let mut coef: Vec<Vec<f64>> = Vec::new();
    for c in 0..3 {
        let z: Vec<f64> = pts.iter().map(|&(i, j)| l[i * nx + j][c] as f64).collect();
        let mut keep = vec![true; z.len()];
        let mut k = solve(&keep, &z)?;
        for _ in 0..3 {
            // drop tiles far above the fit (Milky Way, nebulae)
            let r: Vec<f64> = (0..z.len()).map(|p| z[p] - rows[p].iter().zip(k.iter()).map(|(a, b)| a * b).sum::<f64>()).collect();
            let mut kept: Vec<f32> = r.iter().zip(keep.iter()).filter(|(_, k)| **k).map(|(v, _)| v.abs() as f32).collect();
            let sigma = 1.4826 * median(&mut kept) as f64 + 1e-9;
            keep = r.iter().map(|&v| -4.0 * sigma < v && v < 2.5 * sigma).collect();
            if keep.iter().filter(|k| **k).count() < nt + 2 {
                break;
            }
            k = solve(&keep, &z)?;
        }
        coef.push(k);
    }
    Some(Img::from_fn(h, w, |y, x| {
        let tv = poly_terms(nu(x as f64 + 1.0), nv(y as f64 + 1.0), degree);
        let d = |c: usize| tv.iter().zip(coef[c].iter()).map(|(a, b)| a * b).sum::<f64>() as f32;
        [d(0), d(1), d(2)]
    }))
}

fn chroma(c: Rgb) -> (f32, f32) {
    let s = c[0] + c[1] + c[2];
    if s > 0.0 {
        (c[0] / s, c[1] / s)
    } else {
        (1.0 / 3.0, 1.0 / 3.0)
    }
}

/// Returns (corrected image, background model or None).
pub fn remove_pollution(img: &Img, wt: &Gray, cfg: &StackConfig) -> (Img, Option<Img>) {
    let lp = &cfg.light_pollution;
    if lp.filter == Pollution::Off {
        return (img.clone(), None);
    }
    let b = if lp.filter == Pollution::DeepSky { poly_background(img, wt, lp.degree) } else { uneven_background(img, wt, lp.tiles) };
    let Some(b) = b else { return (img.clone(), None) };
    let s = (lp.strength / 100.0) as f32;
    let ped = 0.4 * median_of(b.subsample(b.step_for(100_000)).map(luminance));
    let iaa = lp.iaa && lp.filter == Pollution::Uneven && !matches!(cfg.sky.mode, super::SkyMode::None | super::SkyMode::All);
    let px: Vec<Rgb> = (0..img.len())
        .into_par_iter()
        .map(|i| {
            let mut m = wt.px[i];
            let (p, bb) = (img.px[i], b.px[i]);
            if iaa && m < 1.0 {
                let sim = if luminance(p) >= 0.5 * luminance(bb) {
                    let (cp, cb) = (chroma(p), chroma(bb));
                    (1.0 - (cp.0 - cb.0).hypot(cp.1 - cb.1) / 0.08).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                m += (1.0 - m) * sim;
            }
            let k = m * s;
            [p[0] - k * (bb[0] - ped), p[1] - k * (bb[1] - ped), p[2] - k * (bb[2] - ped)]
        })
        .collect();
    (Img::from_vec(img.h, img.w, px), Some(b))
}

/// Stars brought out: the part of each star above its surroundings, amplified (sky region only).
pub fn enhance_stars(img: &Img, wt: &Gray, strength: f64, fwhm: f64) -> Img {
    let sigma = (1.5 * if fwhm.is_finite() { fwhm } else { 2.5 }).max(1.5);
    let hp = crate::image::sub(img, &blur(img, 2.0 * sigma));
    let l: Vec<f32> = hp.subsample(hp.step_for(200_000)).map(luminance).collect();
    let (_, noise) = crate::image::median_mad(&l);
    let noise = noise + 1e-6;
    let a = (2.0 * strength / 100.0) as f32;
    let px: Vec<Rgb> = (0..img.len())
        .into_par_iter()
        .map(|i| {
            let h = hp.px[i];
            let wgt = ((luminance(h) / noise - 2.0) / 3.0).clamp(0.0, 1.0) * wt.px[i];
            let p = img.px[i];
            [p[0] + a * wgt * h[0].max(0.0), p[1] + a * wgt * h[1].max(0.0), p[2] + a * wgt * h[2].max(0.0)]
        })
        .collect();
    Img::from_vec(img.h, img.w, px)
}

// --------------------------------------------------------------------------
// Background extraction (as Siril's) and basic colour calibration
// --------------------------------------------------------------------------

pub fn extract_background(img: &Img, wt: &Gray, degree: usize) -> Img {
    let Some(b) = poly_background(img, wt, degree) else { return img.clone() };
    let step = b.step_for(100_000);
    let m = [
        median_of(b.subsample(step).map(|p| p[0])),
        median_of(b.subsample(step).map(|p| p[1])),
        median_of(b.subsample(step).map(|p| p[2])),
    ];
    img.zip_map(&b, move |p, q| [p[0] - q[0] + m[0], p[1] - q[1] + m[1], p[2] - q[2] + m[2]])
}

pub fn calibrate_color(img: &Img, wt: &Gray) -> Img {
    let step = img.step_for(200_000);
    let mut idx: Vec<usize> = (0..img.len()).step_by(step).filter(|&i| wt.px[i] >= 0.5).collect();
    if idx.is_empty() {
        idx = (0..img.len()).step_by(step).collect();
    }
    let bg = [
        median_of(idx.iter().map(|&i| img.px[i][0])),
        median_of(idx.iter().map(|&i| img.px[i][1])),
        median_of(idx.iter().map(|&i| img.px[i][2])),
    ];
    // Average star colour, from aperture sums above the background.
    let (h, w) = (img.h as i64, img.w as i64);
    let stars = detect_stars(img, wt);
    let top = stars.iter().map(|s| s.peak).fold(f64::MIN, f64::max);
    let top = if stars.is_empty() { 1.0 } else { top };
    let (mut rg, mut bgr) = (Vec::new(), Vec::new());
    for st in stars.iter().take(400) {
        if st.peak >= 0.8 * top {
            continue;
        }
        let r = (1.5 * st.fwhm).max(3.0).ceil() as i64;
        let (xi, yi) = (st.x.round() as i64, st.y.round() as i64);
        if !(r < xi && xi <= w - r && r < yi && yi <= h - r) {
            continue;
        }
        let mut f = [0f64; 3];
        for y in yi - r..=yi + r {
            for x in xi - r..=xi + r {
                let c = img.at((y - 1) as usize, (x - 1) as usize);
                for k in 0..3 {
                    f[k] += (c[k] - bg[k]) as f64;
                }
            }
        }
        if f.iter().all(|v| *v > 0.0) {
            rg.push(f[0] / f[1]);
            bgr.push(f[2] / f[1]);
        }
    }
    let (kr, kb) = if rg.len() >= 15 { (1.0 / crate::image::median_f64(rg), 1.0 / crate::image::median_f64(bgr)) } else { (1.0, 1.0) };
    let (kr, kb) = (kr.clamp(0.2, 5.0) as f32, kb.clamp(0.2, 5.0) as f32);
    let l = luminance(bg);
    img.map(move |c| [(c[0] - bg[0]) * kr + l, c[1] - bg[1] + l, (c[2] - bg[2]) * kb + l])
}

/// Colour noise reduction: the colour (difference from the luminance) is smoothed, the luminance kept.
pub fn reduce_color_noise(img: &Img, strength: f64) -> Img {
    let sigma = 0.5 + 2.5 * strength / 100.0;
    let l = img.luminance();
    let c = blur(&img.zip_map(&l, |c, l| [c[0] - l, c[1] - l, c[2] - l]), sigma);
    c.zip_map(&l, |c, l| [c[0] + l, c[1] + l, c[2] + l])
}

// --------------------------------------------------------------------------
// Output: brightness, HDR, tone (sRGB / Adobe RGB curve, stretch, linear)
// --------------------------------------------------------------------------

/// Linear sRGB → linear Adobe RGB (1998), both D65.
pub fn to_adobe(c: Rgb) -> Rgb {
    [0.7152 * c[0] + 0.2848 * c[1], c[1], 0.0412 * c[1] + 0.9588 * c[2]]
}

pub struct Finished {
    pub linear: Img,
    pub image: Img,
    pub gain: f64,
    pub stretch: Option<(f64, f64, f64)>,
}

/// Linear image × brightness gain, the image for TIFF/JPEG, the gain and the
/// stretch used. Auto brightness puts the sky background at a fixed level,
/// without letting more than the brightest 0.1 % of the sky clip.
pub fn finish(img: &Img, wt: &Gray, cfg: &StackConfig, gain: Option<f64>) -> Finished {
    let step = img.step_for(300_000);
    let mut idx: Vec<usize> = (0..img.len()).step_by(step).filter(|&i| wt.px[i] >= 0.5).collect();
    if idx.is_empty() {
        idx = (0..img.len()).step_by(step).collect();
    }
    let mut lum: Vec<f32> = idx.iter().map(|&i| luminance(img.px[i])).collect();
    let fixed = gain.is_some(); // time-lapse: the same for every frame
    let mut gain = gain.unwrap_or(1.0);
    if cfg.auto_brightness && !fixed {
        let q = quantiles(&mut lum, &[0.5, 0.999]);
        let (bg, hi) = (q[0] as f64, q[1] as f64);
        let target = if cfg.hdr { 0.045 } else { 0.03 };
        gain = if bg > 0.0 { target / bg } else { 1.0 };
        if !cfg.hdr && hi > 0.0 {
            gain = gain.min(1.0 / hi);
        }
        gain = gain.clamp(0.05, 500.0);
    }
    let g = gain as f32;
    let lin = img.map(move |c| [c[0] * g, c[1] * g, c[2] * g]);
    let mut disp = lin.clone();
    if cfg.hdr {
        let mut l: Vec<f32> = lin.subsample(step).map(luminance).collect();
        let lw = quantiles(&mut l, &[0.9999])[0].max(1.0);
        disp = lin.map(move |c| {
            let l = luminance(c);
            if l > 0.0 {
                let k = l * (1.0 + l / (lw * lw)) / (1.0 + l) / l;
                [c[0] * k, c[1] * k, c[2] * k]
            } else {
                c
            }
        });
    }
    let (cs, tone) = (cfg.color_space, cfg.tone);
    if cs == ColorSpace::AdobeRgb {
        disp = disp.map(to_adobe);
    }
    let mut str_ = None;
    if tone == Tone::Stretch {
        let s = &cfg.stretch;
        let (b, m, w) = if s.auto { auto_stretch(&disp, 0.25, 2.8) } else { (s.black, s.mid, s.white) };
        str_ = Some((b, m, w));
        disp = hist_stretch(&disp, b, m, w);
    } else {
        let adobe = cs == ColorSpace::AdobeRgb;
        let linear = tone == Tone::Linear;
        let enc = move |v: f32| {
            let v = v.clamp(0.0, 1.0);
            if linear {
                v
            } else if adobe {
                v.powf(1.0 / 2.19921875)
            } else {
                linear_to_srgb(v)
            }
        };
        disp = disp.map(move |c| [enc(c[0]), enc(c[1]), enc(c[2])]);
    }
    Finished { linear: lin, image: disp, gain, stretch: str_ }
}

pub struct PostResult {
    pub linear: Img,
    pub image: Img,
    pub gain: f64,
    pub background: Option<Img>,
    pub stretch: Option<(f64, f64, f64)>,
}

/// Everything after stacking, on the linear stack `raw` (all optional, in this
/// order): background extraction, colour calibration, colour noise reduction,
/// light pollution, star enhancement, then brightness, HDR and tone.
pub fn postprocess(raw: &Img, wt: &Gray, cfg: &StackConfig, fwhm: f64, gain: Option<f64>) -> PostResult {
    let mut img = raw.clone();
    if cfg.bg_extraction.0 {
        img = extract_background(&img, wt, cfg.bg_extraction.1);
    }
    if cfg.color_calibration {
        img = calibrate_color(&img, wt);
    }
    if cfg.color_noise.0 {
        img = reduce_color_noise(&img, cfg.color_noise.1);
    }
    let (img, bgm) = remove_pollution(&img, wt, cfg);
    let img = if cfg.enhance_stars.0 { enhance_stars(&img, wt, cfg.enhance_stars.1, fwhm) } else { img };
    let f = finish(&img, wt, cfg, gain);
    let background = bgm.map(|b| finish(&b, wt, cfg, Some(f.gain)).image);
    PostResult { linear: f.linear, image: f.image, gain: f.gain, background, stretch: f.stretch }
}

/// The post-processing settings in words (for the log).
pub fn postprocess_summary(cfg: &StackConfig) -> String {
    let mut p = Vec::new();
    if cfg.bg_extraction.0 {
        p.push(format!("background extraction (degree {})", cfg.bg_extraction.1));
    }
    if cfg.color_calibration {
        p.push("colour calibration".to_string());
    }
    if cfg.color_noise.0 {
        p.push(format!("colour noise {}", cfg.color_noise.1));
    }
    let lp = &cfg.light_pollution;
    if lp.filter != Pollution::Off {
        p.push(format!("light pollution {} {}{}", lp.filter.name().to_lowercase(), lp.strength, if lp.iaa { " + I.A.A." } else { "" }));
    }
    if cfg.enhance_stars.0 {
        p.push(format!("star light {}", cfg.enhance_stars.1));
    }
    if cfg.auto_brightness {
        p.push("auto brightness".into());
    }
    if cfg.hdr {
        p.push("HDR".into());
    }
    p.push(format!("{}, {}", cfg.tone.name(), cfg.color_space.name()));
    p.join(", ")
}

/// Save a stack result again with (possibly changed) post-processing settings: FITS / XISF linear.
pub fn save_result(res: &super::StackResult, cfg: &StackConfig) -> Result<String, String> {
    let p = postprocess(&res.raw, &res.weight, cfg, res.fwhm, None);
    crate::io::save_panorama(&cfg.output, if crate::io::is_astro(&cfg.output) { &p.linear } else { &p.image })?;
    Ok(cfg.output.clone())
}

#[allow(dead_code)]
fn _name(p: &str) -> String {
    basename(p)
}

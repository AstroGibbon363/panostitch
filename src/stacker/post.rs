//! Everything after combining: the foreground image, light pollution,
//! background extraction, colour calibration, colour noise, star enhancement,
//! brightness, HDR and tone.

use super::{detect_stars, linear_to_srgb, BgModel, ColorCal, ColorSpace, Foreground, ManualCal, Pollution, StackConfig, Tone};
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

/// Thin-plate spline kernel r² ln r.
fn tps(r2: f64) -> f64 {
    if r2 <= 1e-18 { 0.0 } else { 0.5 * r2 * r2.ln() }
}

/// A smoothing thin-plate spline through points `p` (x, y) with values `z`
/// (one column per colour): weights of the kernels and of the plane (1, x, y).
fn tps_fit(p: &[(f64, f64)], z: &[[f64; 3]], lambda: f64) -> Option<(Vec<[f64; 3]>, [[f64; 3]; 3])> {
    let n = p.len();
    let m = n + 3;
    let mut a = nalgebra::DMatrix::<f64>::zeros(m, m);
    for i in 0..n {
        for j in 0..n {
            let (dx, dy) = (p[i].0 - p[j].0, p[i].1 - p[j].1);
            a[(i, j)] = tps(dx * dx + dy * dy);
        }
        a[(i, i)] += lambda;
        let q = [1.0, p[i].0, p[i].1];
        for k in 0..3 {
            a[(i, n + k)] = q[k];
            a[(n + k, i)] = q[k];
        }
    }
    let mut b = nalgebra::DMatrix::<f64>::zeros(m, 3);
    for i in 0..n {
        for c in 0..3 {
            b[(i, c)] = z[i][c];
        }
    }
    let s = a.lu().solve(&b)?;
    if s.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let w = (0..n).map(|i| [s[(i, 0)], s[(i, 1)], s[(i, 2)]]).collect();
    let plane = [[s[(n, 0)], s[(n, 1)], s[(n, 2)]], [s[(n + 1, 0)], s[(n + 1, 1)], s[(n + 1, 2)]], [s[(n + 2, 0)], s[(n + 2, 1)], s[(n + 2, 2)]]];
    Some((w, plane))
}

fn tps_at(p: &[(f64, f64)], w: &[[f64; 3]], plane: &[[f64; 3]; 3], x: f64, y: f64) -> [f64; 3] {
    let mut o = [0.0; 3];
    for c in 0..3 {
        o[c] = plane[0][c] + plane[1][c] * x + plane[2][c] * y;
    }
    for (q, wq) in p.iter().zip(w) {
        let (dx, dy) = (x - q.0, y - q.1);
        let k = tps(dx * dx + dy * dy);
        for c in 0..3 {
            o[c] += wq[c] * k;
        }
    }
    o
}

/// The sky background as a smooth surface through samples of the sky (a
/// smoothing thin-plate spline: radial basis functions, as Siril's RBF). It
/// follows gradients a polynomial cannot (several glows, a bright horizon in one
/// corner). `samples`: sample boxes along the long side; `smooth`: 0 = close to
/// the samples … 1 = very smooth. Boxes on stars count by their median; boxes
/// much brighter than their surroundings (Milky Way, nebulae) are left out.
/// The grid of sky samples for RBF background extraction: sample positions
/// (x, y in units of the long side), their colour, whether each is used, and
/// the box size (px). `samples`: boxes per line (along the long side);
/// `tolerance`: how far a box may stand out from its neighbours before it is
/// taken for an object and left out (1 = default, higher keeps more).
pub struct RbfGrid {
    pub p: Vec<(f64, f64)>,
    pub z: Vec<[f64; 3]>,
    pub keep: Vec<bool>,
    pub box_px: usize,
}

pub fn rbf_grid(img: &Img, wt: &Gray, samples: usize, tolerance: f64) -> Option<RbfGrid> {
    let (h, w) = (img.h, img.w);
    let long = h.max(w) as f64;
    let across = ((samples as f64 * w as f64 / long).round() as usize).max(3);
    let (l, ny, nx, t) = tile_levels(img, wt, across, 0.5);
    // Coordinates in units of the long side, so distances are the same both ways.
    let mut p: Vec<(f64, f64)> = Vec::new();
    let mut z: Vec<[f64; 3]> = Vec::new();
    for i in 0..ny {
        for j in 0..nx {
            let v = l[i * nx + j];
            if v[0].is_nan() {
                continue;
            }
            let (cy, cx) = (((i * t) as f64 + (((i + 1) * t).min(h) - i * t) as f64 / 2.0) / long, ((j * t) as f64 + (((j + 1) * t).min(w) - j * t) as f64 / 2.0) / long);
            p.push((cx, cy));
            z.push([v[0] as f64, v[1] as f64, v[2] as f64]);
        }
    }
    if p.len() < 8 {
        return None;
    }
    let lum = |c: &[f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    // Leave out samples that stand out from their neighbours (the Milky Way, a
    // nebula, a bright star's halo): above the median of the samples within two
    // boxes. Broad glows rise slowly from box to box, so they stay.
    let grid: Vec<(usize, usize)> = {
        let mut g = Vec::new();
        for i in 0..ny {
            for j in 0..nx {
                if !l[i * nx + j][0].is_nan() {
                    g.push((i, j));
                }
            }
        }
        g
    };
    let mut keep = vec![true; p.len()];
    for _ in 0..2 {
        let at: std::collections::HashMap<(usize, usize), usize> = grid.iter().enumerate().filter(|(k, _)| keep[*k]).map(|(k, ij)| (*ij, k)).collect();
        let d: Vec<f64> = grid
            .iter()
            .map(|&(i, j)| {
                // A quadratic surface through the neighbours, without this sample.
                let mut rows: Vec<[f64; 6]> = Vec::new();
                let mut vals: Vec<f64> = Vec::new();
                for di in -2i64..=2 {
                    for dj in -2i64..=2 {
                        if (di, dj) == (0, 0) {
                            continue;
                        }
                        if let Some(&k) = at.get(&((i as i64 + di) as usize, (j as i64 + dj) as usize)) {
                            let (u, v) = (dj as f64, di as f64);
                            rows.push([1.0, u, v, u * u, u * v, v * v]);
                            vals.push(lum(&z[k]));
                        }
                    }
                }
                let k = grid.iter().position(|q| *q == (i, j)).unwrap();
                if rows.len() < 10 {
                    return 0.0;
                }
                let a = nalgebra::DMatrix::from_fn(rows.len(), 6, |r, c| rows[r][c]);
                let b = nalgebra::DVector::from_vec(vals);
                // Relative to the level there: a few percent above a smooth surface is
                // still sky (a glow's curve), more is an object.
                match a.svd(true, true).solve(&b, 1e-10) {
                    Ok(c) => (lum(&z[k]) - c[0]) / c[0].abs().max(1e-6),
                    Err(_) => 0.0,
                }
            })
            .collect();
        let mut ad: Vec<f32> = d.iter().map(|v| v.abs() as f32).collect();
        let sigma = 1.4826 * median(&mut ad) as f64;
        let next: Vec<bool> = d.iter().map(|&v| v < (3.0 * sigma).max(0.02) * tolerance.max(0.05)).collect();
        if next.iter().filter(|k| **k).count() < 8 || next == keep {
            break;
        }
        keep = next;
    }
    // The faint outskirts around a left-out object (a galaxy's halo) are not
    // background either: leave out its neighbours too, `grow` boxes around.
    let grow = 1i64;
    let out: std::collections::HashSet<(usize, usize)> = grid.iter().enumerate().filter(|(k, _)| !keep[*k]).map(|(_, ij)| *ij).collect();
    let grown: Vec<bool> = grid
        .iter()
        .enumerate()
        .map(|(k, &(i, j))| keep[k] && !(-grow..=grow).any(|di| (-grow..=grow).any(|dj| out.contains(&((i as i64 + di) as usize, (j as i64 + dj) as usize)))))
        .collect();
    if grown.iter().filter(|k| **k).count() >= 8 {
        keep = grown;
    }
    Some(RbfGrid { p, z, keep, box_px: t })
}

/// The sky background as a smooth surface through the grid of sky samples (see
/// `rbf_grid`), a smoothing thin-plate spline: radial basis functions, as
/// Siril's RBF. `smooth`: 0 = close to the samples … 1 = very smooth.
pub fn rbf_background(img: &Img, wt: &Gray, samples: usize, smooth: f64, tolerance: f64) -> Option<Img> {
    let (h, w) = (img.h, img.w);
    let long = h.max(w) as f64;
    let RbfGrid { p, z, keep, box_px: t } = rbf_grid(img, wt, samples, tolerance)?;
    let lambda = |s: f64| 1e-8 * 10f64.powf(5.0 * s.clamp(0.0, 1.0)) * p.len() as f64;
    let kp: Vec<(f64, f64)> = (0..p.len()).filter(|&k| keep[k]).map(|k| p[k]).collect();
    let kz: Vec<[f64; 3]> = (0..p.len()).filter(|&k| keep[k]).map(|k| z[k]).collect();
    let (wk, pl) = tps_fit(&kp, &kz, lambda(smooth))?;
    // Evaluated on a grid a quarter of a sample box apart, then smoothly filled in.
    let s = (t / 4).max(4);
    let (gy, gx) = (h.div_ceil(s), w.div_ceil(s));
    let mut g = vec![[0f32; 3]; gy * gx];
    g.par_chunks_mut(gx).enumerate().for_each(|(i, row)| {
        for (j, cell) in row.iter_mut().enumerate() {
            let v = tps_at(&kp, &wk, &pl, ((j as f64 + 0.5) * s as f64) / long, ((i as f64 + 0.5) * s as f64) / long);
            *cell = [v[0] as f32, v[1] as f32, v[2] as f32];
        }
    });
    Some(upsample(&g, gy, gx, s, h, w))
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
    subtract_background(img, poly_background(img, wt, degree))
}

/// Background extraction with the model of the settings: RBF for frames that
/// are all sky (deep sky), the polynomial for landscapes (the sky region's
/// edge and the ground would bend a flexible surface).
pub fn extract_background_cfg(img: &Img, wt: &Gray, cfg: &StackConfig) -> Img {
    let model = if cfg.sky.mode == super::SkyMode::All { cfg.bg_model } else { BgModel::Polynomial };
    let b = match model {
        BgModel::Rbf => rbf_background(img, wt, cfg.bg_samples, cfg.bg_smooth, cfg.bg_tolerance),
        BgModel::Polynomial => poly_background(img, wt, cfg.bg_extraction.1),
    };
    subtract_background(img, b)
}

/// The image minus the background model, the sky keeping its average level.
fn subtract_background(img: &Img, b: Option<Img>) -> Img {
    let Some(b) = b else { return img.clone() };
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

/// Per-colour median (or mean) of the area `r` (fractions x0, y0, x1, y1) of the image.
fn area_level(img: &Img, r: [f64; 4], mean: bool) -> Option<[f32; 3]> {
    let (w, h) = (img.w as f64, img.h as f64);
    let (x0, x1) = ((r[0].min(r[2]) * w).floor().max(0.0) as usize, ((r[0].max(r[2]) * w).ceil() as usize).min(img.w));
    let (y0, y1) = ((r[1].min(r[3]) * h).floor().max(0.0) as usize, ((r[1].max(r[3]) * h).ceil() as usize).min(img.h));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let step = (((x1 - x0) * (y1 - y0)) as f64 / 200_000.0).sqrt().ceil().max(1.0) as usize;
    let px: Vec<Rgb> = (y0..y1).step_by(step).flat_map(|y| (x0..x1).step_by(step).map(move |x| (y, x))).map(|(y, x)| img.at(y, x)).collect();
    if px.is_empty() {
        return None;
    }
    Some(if mean {
        let n = px.len() as f32;
        let s = px.iter().fold([0f32; 3], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2]]);
        [s[0] / n, s[1] / n, s[2] / n]
    } else {
        [median_of(px.iter().map(|c| c[0])), median_of(px.iter().map(|c| c[1])), median_of(px.iter().map(|c| c[2]))]
    })
}

/// Colour calibration by hand: the background area becomes neutral grey, the
/// white reference area (its light above the background, averaged) white, then
/// temperature and tint. Without a background area the sky's median is used;
/// without a white reference only the background, temperature and tint count.
pub fn calibrate_manual(img: &Img, wt: &Gray, m: &ManualCal) -> Img {
    let bg = m.background.and_then(|r| area_level(img, r, false)).unwrap_or_else(|| {
        let step = img.step_for(200_000);
        let mut idx: Vec<usize> = (0..img.len()).step_by(step).filter(|&i| wt.px[i] >= 0.5).collect();
        if idx.is_empty() {
            idx = (0..img.len()).step_by(step).collect();
        }
        [median_of(idx.iter().map(|&i| img.px[i][0])), median_of(idx.iter().map(|&i| img.px[i][1])), median_of(idx.iter().map(|&i| img.px[i][2]))]
    });
    let (mut kr, mut kb) = (1.0f32, 1.0f32);
    if let Some(wv) = m.white.and_then(|r| area_level(img, r, true)) {
        let d = [wv[0] - bg[0], wv[1] - bg[1], wv[2] - bg[2]];
        if d.iter().all(|v| *v > 1e-6) {
            (kr, kb) = ((d[1] / d[0]).clamp(0.2, 5.0), (d[1] / d[2]).clamp(0.2, 5.0));
        }
    }
    let t = m.temperature as f32 * 0.25;
    let g = 1.0 - m.tint as f32 * 0.2;
    let k = [kr * (1.0 + t), g, kb * (1.0 - t)];
    let l = luminance(bg);
    img.map(move |c| [(c[0] - bg[0]) * k[0] + l, (c[1] - bg[1]) * k[1] + l, (c[2] - bg[2]) * k[2] + l])
}

/// Colour noise reduction (wavelets on the colour layers, the luminance kept; see `denoise`).
pub fn reduce_color_noise(img: &Img, strength: f64) -> Img {
    crate::denoise::reduce_color_noise(img, strength / 100.0)
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
    /// Arcsinh / GHS with the points from the background: (black point, symmetry point).
    pub points: Option<(f64, f64)>,
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
    let mut points = None;
    if tone == Tone::Stretch {
        let s = &cfg.stretch;
        let (b, m, w) = if s.auto { auto_stretch(&disp, 0.25, 2.8) } else { (s.black, s.mid, s.white) };
        str_ = Some((b, m, w));
        disp = hist_stretch(&disp, b, m, w);
    } else if matches!(tone, Tone::Asinh | Tone::Ghs) {
        // The curves of the Post-processing tab, keeping the star colours.
        let s = &cfg.stretch;
        let (black, sp) = if s.auto { crate::finish::background_points(&disp) } else { (s.black, s.ghs_sp) };
        points = Some((black, sp));
        let fp = crate::finish::FinishParams {
            stretch: if tone == Tone::Asinh { crate::finish::StretchMode::Asinh } else { crate::finish::StretchMode::Ghs },
            black,
            asinh: s.asinh,
            ghs_d: s.ghs_d,
            ghs_b: s.ghs_b,
            ghs_sp: sp,
            keep_colour: true,
            ..Default::default()
        };
        disp = crate::finish::stretch(&disp.map(|c| c.map(|v| v.clamp(0.0, 1.0))), &fp, &mut Default::default());
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
    Finished { linear: lin, image: disp, gain, stretch: str_, points }
}

pub struct PostResult {
    pub linear: Img,
    pub image: Img,
    pub gain: f64,
    pub background: Option<Img>,
    pub stretch: Option<(f64, f64, f64)>,
    pub points: Option<(f64, f64)>,
    /// RBF background extraction: its samples (x, y as fractions of the width / height, used).
    pub samples: Vec<(f32, f32, bool)>,
}

/// Everything after stacking, on the linear stack `raw` (all optional, in this
/// order): background extraction, colour calibration, colour noise reduction,
/// light pollution, star enhancement, then brightness, HDR and tone.
pub fn postprocess(raw: &Img, wt: &Gray, cfg: &StackConfig, fwhm: f64, gain: Option<f64>) -> PostResult {
    let mut img = raw.clone();
    let rbf = cfg.bg_extraction.0 && cfg.bg_model == BgModel::Rbf && cfg.sky.mode == super::SkyMode::All;
    let long = raw.w.max(raw.h) as f64;
    let samples: Vec<(f32, f32, bool)> = if rbf {
        rbf_grid(raw, wt, cfg.bg_samples, cfg.bg_tolerance)
            .map(|g| g.p.iter().zip(&g.keep).map(|(q, &k)| ((q.0 * long / raw.w as f64) as f32, (q.1 * long / raw.h as f64) as f32, k)).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    if cfg.bg_extraction.0 {
        img = extract_background_cfg(&img, wt, cfg);
    }
    if cfg.color_calibration {
        img = match cfg.color_cal {
            ColorCal::Auto => calibrate_color(&img, wt),
            ColorCal::Manual => calibrate_manual(&img, wt, &cfg.manual_cal),
        };
    }
    if cfg.color_noise.0 {
        img = reduce_color_noise(&img, cfg.color_noise.1);
    }
    let (img, bgm) = remove_pollution(&img, wt, cfg);
    let img = if cfg.enhance_stars.0 { enhance_stars(&img, wt, cfg.enhance_stars.1, fwhm) } else { img };
    let f = finish(&img, wt, cfg, gain);
    let background = bgm.map(|b| finish(&b, wt, cfg, Some(f.gain)).image);
    PostResult { linear: f.linear, image: f.image, gain: f.gain, background, stretch: f.stretch, points: f.points, samples }
}

/// The post-processing settings in words (for the log).
pub fn postprocess_summary(cfg: &StackConfig) -> String {
    let mut p = Vec::new();
    if cfg.bg_extraction.0 {
        p.push(match cfg.bg_model {
            BgModel::Rbf => format!("background extraction (RBF, {} samples, smoothing {:.2})", cfg.bg_samples, cfg.bg_smooth),
            BgModel::Polynomial => format!("background extraction (degree {})", cfg.bg_extraction.1),
        });
    }
    if cfg.color_calibration {
        p.push(format!("colour calibration ({})", cfg.color_cal.name().to_lowercase()));
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
    if cfg.save_linear == Some(true) {
        p.push("saved linear".into());
    } else if cfg.save_linear == Some(false) {
        p.push("saved stretched".into());
    }
    p.join(", ")
}

/// Save a stack result again with (possibly changed) post-processing settings,
/// linear or stretched (see `StackConfig::saves_linear`).
pub fn save_result(res: &super::StackResult, cfg: &StackConfig) -> Result<String, String> {
    let p = postprocess(&res.raw, &res.weight, cfg, res.fwhm, None);
    crate::io::save_panorama(&cfg.output, if cfg.saves_linear(&cfg.output) { &p.linear } else { &p.image })?;
    Ok(cfg.output.clone())
}

#[allow(dead_code)]
fn _name(p: &str) -> String {
    basename(p)
}

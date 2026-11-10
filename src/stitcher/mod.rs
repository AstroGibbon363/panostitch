//! Stitching core, split into steps so the window can run, inspect and adjust
//! each one:
//!
//!   load (RAW/JPEG/TIFF) → corner features → match neighbours → RANSAC motion
//!   model → chain to a reference image → project → blend (`render`).
//!
//! Assumes the images were shot by rotating the camera (standard panorama case).

pub mod render;

use crate::image::{blur, luminance, median_f64, quantiles, resize, to_full, Gray, Img};
use nalgebra::{DMatrix, DVector, Matrix3, Vector3};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::collections::HashMap;

pub use render::*;

pub type M3 = Matrix3<f64>;
pub type Pt = [f64; 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    Homography,
    Affine,
    Similarity,
    Translation,
}

impl Model {
    pub const ALL: [Model; 4] = [Model::Homography, Model::Affine, Model::Similarity, Model::Translation];
    pub fn name(self) -> &'static str {
        match self {
            Model::Homography => "homography",
            Model::Affine => "affine",
            Model::Similarity => "similarity",
            Model::Translation => "translation",
        }
    }
    pub fn from_name(s: &str) -> Option<Model> {
        Model::ALL.iter().copied().find(|m| m.name() == s)
    }
    /// Minimum number of correspondences that determine the model.
    pub fn min_samples(self) -> usize {
        match self {
            Model::Translation => 1,
            Model::Similarity => 2,
            Model::Affine => 3,
            Model::Homography => 4,
        }
    }
}

// --------------------------------------------------------------------------
// Histogram stretch (for linear data such as deep-sky images)
// --------------------------------------------------------------------------

/// Midtones transfer function: maps 0 → 0, `m` → 0.5, 1 → 1.
#[inline]
pub fn mtf(m: f32, x: f32) -> f32 {
    if x <= 0.0 {
        0.0
    } else if x >= 1.0 {
        1.0
    } else {
        (m - 1.0) * x / ((2.0 * m - 1.0) * x - m)
    }
}

/// Apply black point, midtones and white point to every colour channel.
pub fn hist_stretch(img: &Img, black: f64, mid: f64, white: f64) -> Img {
    let s = 1.0 / ((white - black) as f32).max(1e-6);
    let (b, m) = (black as f32, mid as f32);
    let f = |v: f32| mtf(m, ((v - b) * s).clamp(0.0, 1.0));
    img.map(move |c| [f(c[0]), f(c[1]), f(c[2])])
}

/// Automatic stretch (as PixInsight's screen transfer function does): black
/// point just below the sky background, midtones so that the background ends
/// up at `target` brightness. Black pixels are ignored. Returns (black, mid, white).
pub fn auto_stretch(img: &Img, target: f64, clip: f64) -> (f64, f64, f64) {
    let step = img.step_for(200_000);
    let mut v: Vec<f32> = img.subsample(step).map(luminance).filter(|l| *l > 0.0).collect();
    if v.is_empty() {
        return (0.0, 0.5, 1.0);
    }
    let (med, mad) = crate::image::median_mad(&v);
    v.clear();
    let (med, mad) = (med as f64, mad as f64);
    let black = (med - clip * mad).clamp(0.0, 0.99);
    let x = (med - black) / (1.0 - black);
    let mid = if x > 0.0 { (mtf(target as f32, x as f32) as f64).clamp(0.0001, 0.9999) } else { 0.5 };
    (black, mid, 1.0)
}

/// Histogram (counts in `n` bins over 0…1) of the luminance of non-black pixels.
pub fn lum_histogram(img: &Img, n: usize) -> Vec<usize> {
    let mut h = vec![0usize; n];
    for c in img.subsample(img.step_for(500_000)) {
        let l = luminance(c);
        if l > 0.0 {
            let k = ((l * n as f32).floor() as isize).clamp(0, n as isize - 1) as usize;
            h[k] += 1;
        }
    }
    h
}

// --------------------------------------------------------------------------
// Features: Harris corners spread over a grid + BRIEF binary descriptors
// --------------------------------------------------------------------------

const PATCH: i64 = 13; // descriptor sampling radius (px)
pub type Desc = [u64; 4]; // 256-bit descriptor

/// Fixed random point pairs inside the patch (BRIEF sampling pattern).
fn pairs() -> &'static Vec<[i64; 4]> {
    use std::sync::OnceLock;
    static PAIRS: OnceLock<Vec<[i64; 4]>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        let mut rng = ChaCha8Rng::seed_from_u64(20240607);
        let mut out = Vec::with_capacity(256);
        let mut randn = move || -> f64 {
            // Box–Muller
            use rand::Rng;
            let u1: f64 = rng.gen_range(1e-12..1.0);
            let u2: f64 = rng.gen();
            (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        };
        while out.len() < 256 {
            let v: Vec<i64> = (0..4).map(|_| (6.0 * randn()).round() as i64).collect();
            let (x1, y1, x2, y2) = (v[0], v[1], v[2], v[3]);
            if x1 * x1 + y1 * y1 <= PATCH * PATCH && x2 * x2 + y2 * y2 <= PATCH * PATCH && (x1, y1) != (x2, y2) {
                out.push([x1, y1, x2, y2]);
            }
        }
        out
    })
}

#[inline]
pub fn hamming(a: &Desc, b: &Desc) -> u32 {
    (a[0] ^ b[0]).count_ones() + (a[1] ^ b[1]).count_ones() + (a[2] ^ b[2]).count_ones() + (a[3] ^ b[3]).count_ones()
}

/// Stretch to the 1st–99th percentile range (from a subsample, for speed).
fn stretch(g: &Gray) -> Gray {
    let mut s: Vec<f32> = g.px.iter().step_by(7).copied().collect();
    let q = quantiles(&mut s, &[0.01, 0.99]);
    let (lo, hi) = (q[0], q[1]);
    if hi <= lo {
        return g.clone();
    }
    g.map(move |v| ((v - lo) / (hi - lo)).clamp(0.0, 1.0))
}

fn describe(s: &Gray, i: usize, j: usize, oriented: bool) -> Desc {
    let (mut c, mut sn) = (1.0f64, 0.0f64);
    let at = |ii: i64, jj: i64| s.px[(ii as usize) * s.w + jj as usize] as f64;
    if oriented {
        let (mut m10, mut m01) = (0.0, 0.0);
        for dy in -PATCH..=PATCH {
            for dx in -PATCH..=PATCH {
                if dx * dx + dy * dy > PATCH * PATCH {
                    continue;
                }
                let v = at(i as i64 + dy, j as i64 + dx);
                m10 += dx as f64 * v;
                m01 += dy as f64 * v;
            }
        }
        let th = m01.atan2(m10);
        c = th.cos();
        sn = th.sin();
    }
    let mut w = [0u64; 4];
    for (k, p) in pairs().iter().enumerate() {
        let (mut x1, mut y1, mut x2, mut y2) = (p[0], p[1], p[2], p[3]);
        if oriented {
            let r = |x: i64, y: i64| ((c * x as f64 - sn * y as f64).round() as i64, (sn * x as f64 + c * y as f64).round() as i64);
            (x1, y1) = r(x1, y1);
            (x2, y2) = r(x2, y2);
        }
        if at(i as i64 + y1, j as i64 + x1) < at(i as i64 + y2, j as i64 + x2) {
            w[k >> 6] |= 1u64 << (k & 63);
        }
    }
    w
}

/// Corner keypoints (full-resolution x, y) and descriptors.
#[derive(Clone, Debug, Default)]
pub struct Features {
    pub pts: Vec<Pt>,
    pub desc: Vec<Desc>,
}

/// Parameters of `detect`.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectParams {
    /// Detect on a copy resized by this factor (faster, less noise).
    pub scale: f64,
    /// At most this many keypoints, spread evenly over a grid of the image.
    pub nkeypoints: usize,
    /// Minimum corner strength as a fraction of the strongest corner.
    pub threshold: f64,
    /// Gaussian blur σ (px) against sensor noise.
    pub denoise: f64,
    /// Rotation-invariant descriptors (only needed for rolled frames).
    pub oriented: bool,
}

impl Default for DetectParams {
    fn default() -> Self {
        Self { scale: 0.5, nkeypoints: 5000, threshold: 0.001, denoise: 1.5, oriented: false }
    }
}

/// Corner keypoints and descriptors (see `DetectParams`).
pub fn detect(img: &Img, p: &DetectParams) -> Features {
    let mut g = img.luminance();
    if p.scale < 1.0 {
        g = resize(&g, p.scale);
    }
    let g = stretch(&blur(&g, p.denoise));
    let (h, w) = (g.h, g.w);
    let m = (PATCH + 3) as usize;
    if h <= 2 * m || w <= 2 * m {
        return Features::default();
    }
    let mut gx = Gray::new(h, w);
    let mut gy = Gray::new(h, w);
    gx.px.par_chunks_mut(w).zip(gy.px.par_chunks_mut(w)).enumerate().for_each(|(i, (rx, ry))| {
        if i == 0 || i == h - 1 {
            return;
        }
        for j in 1..w - 1 {
            rx[j] = (g.at(i, j + 1) - g.at(i, j - 1)) / 2.0;
            ry[j] = (g.at(i + 1, j) - g.at(i - 1, j)) / 2.0;
        }
    });
    let xx = blur(&gx.zip_map(&gx, |a, b| a * b), 1.5);
    let yy = blur(&gy.zip_map(&gy, |a, b| a * b), 1.5);
    let xy = blur(&gx.zip_map(&gy, |a, b| a * b), 1.5);
    let r_img = Gray::from_vec(
        h,
        w,
        (0..h * w).into_par_iter().map(|k| xx.px[k] * yy.px[k] - xy.px[k] * xy.px[k] - 0.04 * (xx.px[k] + yy.px[k]).powi(2)).collect(),
    );
    let rmax = r_img.px.iter().cloned().fold(f32::MIN, f32::max);
    let thr = p.threshold as f32 * rmax;
    // Local maxima in 5×5 neighbourhoods.
    let mut cands: Vec<(f32, usize, usize)> = (m..h - m)
        .into_par_iter()
        .flat_map_iter(|i| {
            let r_img = &r_img;
            (m..w - m).filter_map(move |j| {
                let r = r_img.at(i, j);
                if r <= thr {
                    return None;
                }
                for dj in -2i64..=2 {
                    for di in -2i64..=2 {
                        if di == 0 && dj == 0 {
                            continue;
                        }
                        let v = r_img.at((i as i64 + di) as usize, (j as i64 + dj) as usize);
                        if v > r || (v == r && (dj, di) < (0, 0)) {
                            return None;
                        }
                    }
                }
                Some((r, i, j))
            })
        })
        .collect();
    cands.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    // Even spread: each grid cell gets a quota first, then the strongest of the rest.
    let gxn = 16usize;
    let gyn = ((16.0 * h as f64 / w as f64).round() as usize).max(1);
    let quota = p.nkeypoints.div_ceil(gxn * gyn);
    let mut used = vec![0usize; gyn * gxn];
    let mut chosen: Vec<(usize, usize)> = Vec::new();
    let mut rest: Vec<(usize, usize)> = Vec::new();
    for &(_, i, j) in &cands {
        let ci = (i * gyn / h).min(gyn - 1);
        let cj = (j * gxn / w).min(gxn - 1);
        if used[ci * gxn + cj] < quota && chosen.len() < p.nkeypoints {
            used[ci * gxn + cj] += 1;
            chosen.push((i, j));
        } else {
            rest.push((i, j));
        }
    }
    let extra = p.nkeypoints.saturating_sub(chosen.len()).min(rest.len());
    chosen.extend_from_slice(&rest[..extra]);
    let s = blur(&g, 1.2); // smoothing for stable descriptor bits
    let sub = |a: f32, b: f32, c: f32| -> f64 {
        let d = (a - 2.0 * b + c) as f64;
        if d < 0.0 {
            (((a - c) as f64) / (2.0 * d)).clamp(-0.5, 0.5)
        } else {
            0.0
        }
    };
    let out: Vec<(Pt, Desc)> = chosen
        .par_iter()
        .map(|&(i, j)| {
            // Sub-pixel position from a parabola through the corner response (1-based coordinates).
            let x = to_full(j as f64 + 1.0 + sub(r_img.at(i, j - 1), r_img.at(i, j), r_img.at(i, j + 1)), p.scale);
            let y = to_full(i as f64 + 1.0 + sub(r_img.at(i - 1, j), r_img.at(i, j), r_img.at(i + 1, j)), p.scale);
            ([x, y], describe(&s, i, j, p.oriented))
        })
        .collect();
    Features { pts: out.iter().map(|o| o.0).collect(), desc: out.iter().map(|o| o.1).collect() }
}

/// Index of the nearest descriptor in `ys` to `x`, with the best and second-best distances.
fn nearest2(x: &Desc, ys: &[Desc]) -> (u32, u32, usize) {
    let (mut d1, mut d2, mut j1) = (u32::MAX, u32::MAX, usize::MAX);
    for (j, y) in ys.iter().enumerate() {
        let d = hamming(x, y);
        if d < d1 {
            d2 = d1;
            d1 = d;
            j1 = j;
        } else if d < d2 {
            d2 = d;
        }
    }
    (d1, d2, j1)
}

/// Match two feature sets from `detect`; returns the matched points (in 1, in 2).
/// A match must be mutual (each is the other's nearest neighbour), clearly better
/// than the runner-up (best < `ratio` × second best, Lowe's test), and closer than
/// `max_distance` (fraction of descriptor bits that differ).
pub fn match_pair(f1: &Features, f2: &Features, ratio: f64, max_distance: f64) -> (Vec<Pt>, Vec<Pt>) {
    if f1.desc.is_empty() || f2.desc.is_empty() {
        return (vec![], vec![]);
    }
    let back: Vec<usize> = f2.desc.par_iter().map(|d| nearest2(d, &f1.desc).2).collect();
    let m: Vec<(usize, usize)> = f1
        .desc
        .par_iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let (d1, d2, j) = nearest2(d, &f2.desc);
            if (d1 as f64) < ratio * d2 as f64 && d1 as f64 <= max_distance * 256.0 && back[j] == i {
                Some((i, j))
            } else {
                None
            }
        })
        .collect();
    (m.iter().map(|&(i, _)| f1.pts[i]).collect(), m.iter().map(|&(_, j)| f2.pts[j]).collect())
}

// --------------------------------------------------------------------------
// Motion models (normalised DLT for homographies, least squares otherwise) + RANSAC
// --------------------------------------------------------------------------

#[inline]
pub fn apply_h(h: &M3, x: f64, y: f64) -> (f64, f64) {
    let w = h[(2, 0)] * x + h[(2, 1)] * y + h[(2, 2)];
    ((h[(0, 0)] * x + h[(0, 1)] * y + h[(0, 2)]) / w, (h[(1, 0)] * x + h[(1, 1)] * y + h[(1, 2)]) / w)
}

pub fn translation(dx: f64, dy: f64) -> M3 {
    M3::new(1.0, 0.0, dx, 0.0, 1.0, dy, 0.0, 0.0, 1.0)
}

pub fn identity() -> M3 {
    M3::identity()
}

/// Inverse of a 3×3 matrix (the identity if singular).
pub fn inv(m: &M3) -> M3 {
    m.try_inverse().unwrap_or_else(M3::identity)
}

fn normalizer(p: &[Pt]) -> M3 {
    let n = p.len() as f64;
    let cx = p.iter().map(|q| q[0]).sum::<f64>() / n;
    let cy = p.iter().map(|q| q[1]).sum::<f64>() / n;
    let d = p.iter().map(|q| ((q[0] - cx).powi(2) + (q[1] - cy).powi(2)).sqrt()).sum::<f64>() / n;
    let s = if d > 0.0 { 2f64.sqrt() / d } else { 1.0 };
    M3::new(s, 0.0, -s * cx, 0.0, s, -s * cy, 0.0, 0.0, 1.0)
}

/// Homography H with H * src ≈ dst (N ≥ 4).
pub fn dlt(src: &[Pt], dst: &[Pt]) -> Result<M3, String> {
    let n = src.len();
    if n < 4 {
        return Err("DLT needs 4 points".into());
    }
    let (ts, td) = (normalizer(src), normalizer(dst));
    let mut a = DMatrix::<f64>::zeros(2 * n, 9);
    for i in 0..n {
        let (x, y) = apply_h(&ts, src[i][0], src[i][1]);
        let (u, v) = apply_h(&td, dst[i][0], dst[i][1]);
        let r1 = [-x, -y, -1.0, 0.0, 0.0, 0.0, u * x, u * y, u];
        let r2 = [0.0, 0.0, 0.0, -x, -y, -1.0, v * x, v * y, v];
        for k in 0..9 {
            a[(2 * i, k)] = r1[k];
            a[(2 * i + 1, k)] = r2[k];
        }
    }
    let svd = a.svd(false, true);
    let vt = svd.v_t.ok_or("SVD failed")?;
    let sv = &svd.singular_values;
    let mut kmin = 0;
    for k in 0..sv.len() {
        if sv[k] < sv[kmin] {
            kmin = k;
        }
    }
    let hrow = vt.row(kmin);
    let hm = M3::new(hrow[0], hrow[1], hrow[2], hrow[3], hrow[4], hrow[5], hrow[6], hrow[7], hrow[8]);
    let hh = inv(&td) * hm * ts;
    if hh[(2, 2)] == 0.0 || !hh[(2, 2)].is_finite() {
        return Err("degenerate homography".into());
    }
    Ok(hh / hh[(2, 2)])
}

/// Least-squares fit of `model` with H * src ≈ dst.
pub fn fit_model(model: Model, src: &[Pt], dst: &[Pt]) -> Result<M3, String> {
    if model == Model::Homography {
        return dlt(src, dst);
    }
    let n = src.len() as f64;
    if src.is_empty() {
        return Err("no points".into());
    }
    let cs = [src.iter().map(|p| p[0]).sum::<f64>() / n, src.iter().map(|p| p[1]).sum::<f64>() / n];
    let cd = [dst.iter().map(|p| p[0]).sum::<f64>() / n, dst.iter().map(|p| p[1]).sum::<f64>() / n];
    if model == Model::Translation {
        return Ok(translation(cd[0] - cs[0], cd[1] - cs[1]));
    }
    // Centred for conditioning.
    let s: Vec<Pt> = src.iter().map(|p| [p[0] - cs[0], p[1] - cs[1]]).collect();
    let d: Vec<Pt> = dst.iter().map(|p| [p[0] - cd[0], p[1] - cd[1]]).collect();
    let m: [[f64; 2]; 2] = if model == Model::Similarity {
        // [a -b; b a]: rotation + uniform scale. Normal equations of the 2n×2 system.
        let mut ata = 0.0;
        let (mut atb_a, mut atb_b) = (0.0, 0.0);
        for i in 0..s.len() {
            let (sx, sy) = (s[i][0], s[i][1]);
            let (dx, dy) = (d[i][0], d[i][1]);
            ata += sx * sx + sy * sy;
            atb_a += sx * dx + sy * dy;
            atb_b += -sy * dx + sx * dy;
        }
        if ata <= 0.0 {
            return Err("degenerate points".into());
        }
        let (a, b) = (atb_a / ata, atb_b / ata);
        [[a, -b], [b, a]]
    } else {
        // Affine: M = D Sᵀ (S Sᵀ)⁻¹
        let mut sst = [[0.0; 2]; 2];
        let mut dst_ = [[0.0; 2]; 2];
        for i in 0..s.len() {
            for r in 0..2 {
                for c in 0..2 {
                    sst[r][c] += s[i][r] * s[i][c];
                    dst_[r][c] += d[i][r] * s[i][c];
                }
            }
        }
        let det = sst[0][0] * sst[1][1] - sst[0][1] * sst[1][0];
        if det.abs() < 1e-12 {
            return Err("degenerate points".into());
        }
        let inv_ = [[sst[1][1] / det, -sst[0][1] / det], [-sst[1][0] / det, sst[0][0] / det]];
        let mut m = [[0.0; 2]; 2];
        for r in 0..2 {
            for c in 0..2 {
                m[r][c] = dst_[r][0] * inv_[0][c] + dst_[r][1] * inv_[1][c];
            }
        }
        m
    };
    let tx = cd[0] - (m[0][0] * cs[0] + m[0][1] * cs[1]);
    let ty = cd[1] - (m[1][0] * cs[0] + m[1][1] * cs[1]);
    Ok(M3::new(m[0][0], m[0][1], tx, m[1][0], m[1][1], ty, 0.0, 0.0, 1.0))
}

pub fn reproj_err(h: &M3, src: Pt, dst: Pt) -> f64 {
    let (x, y) = apply_h(h, src[0], src[1]);
    (x - dst[0]).hypot(y - dst[1])
}

pub fn inliers(h: &M3, src: &[Pt], dst: &[Pt], thresh: f64) -> Vec<usize> {
    (0..src.len()).filter(|&i| reproj_err(h, src[i], dst[i]) < thresh).collect()
}

fn finite(h: &M3) -> bool {
    h.iter().all(|v| v.is_finite())
}

/// RANSAC fit of `model` with H * src ≈ dst. `thresh` is the inlier distance in
/// full-resolution pixels. Returns (H, inlier indices).
pub fn ransac(src: &[Pt], dst: &[Pt], model: Model, iters: usize, thresh: f64) -> Result<(M3, Vec<usize>), String> {
    let (n, k) = (src.len(), model.min_samples());
    let need = (k + 2).max(4);
    if n < need {
        return Err(format!("Only {n} matches; need ≥ {need}. Images may not overlap enough."));
    }
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let mut best: Vec<usize> = Vec::new();
    let mut idx: Vec<usize> = (0..n).collect();
    let pick = |idx: &[usize], src: &[Pt]| -> Vec<Pt> { idx.iter().map(|&i| src[i]).collect() };
    for _ in 0..iters {
        idx.shuffle(&mut rng);
        let sel = &idx[..k];
        let h = match fit_model(model, &pick(sel, src), &pick(sel, dst)) {
            Ok(h) => h,
            Err(_) => continue,
        };
        if !finite(&h) {
            continue;
        }
        let inl = inliers(&h, src, dst, thresh);
        if inl.len() > best.len() {
            best = inl;
        }
    }
    if best.len() < need {
        return Err(format!("RANSAC found only {} inliers; images may not overlap.", best.len()));
    }
    let h = fit_model(model, &pick(&best, src), &pick(&best, dst))?;
    let best = inliers(&h, src, dst, thresh); // refit once on the refined inlier set
    if best.len() < need {
        return Err("RANSAC refit lost its inliers".into());
    }
    Ok((fit_model(model, &pick(&best, src), &pick(&best, dst))?, best))
}

// --------------------------------------------------------------------------
// Placement
// --------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    All,
    Sequential,
    Grid,
}

/// Pairs of images (i < j, 0-based) to try matching.
pub fn candidate_pairs(n: usize, mode: Layout, cols: usize, snake: bool) -> Vec<(usize, usize)> {
    match mode {
        Layout::All => (0..n).flat_map(|i| (i + 1..n).map(move |j| (i, j))).collect(),
        Layout::Sequential => (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect(),
        Layout::Grid => {
            let cols = cols.clamp(1, n.max(1));
            let pos = |k: usize| -> (i64, i64) {
                let (r, c) = ((k / cols) as i64, (k % cols) as i64);
                (r, if snake && r % 2 == 1 { cols as i64 - 1 - c } else { c })
            };
            let at: HashMap<(i64, i64), usize> = (0..n).map(|k| (pos(k), k)).collect();
            let mut out = Vec::new();
            for k in 0..n {
                let (r, c) = pos(k);
                for (dr, dc) in [(0, 1), (1, 0), (1, 1), (1, -1)] {
                    if let Some(&j) = at.get(&(r + dr, c + dc)) {
                        out.push((k.min(j), k.max(j)));
                    }
                }
            }
            out.sort();
            out.dedup();
            out
        }
    }
}

/// Is a RANSAC result a real overlap? Returns (accepted, reason).
pub fn accept_pair(nmatches: usize, ninliers: usize, h: &M3, hgt: usize, wid: usize, min_inliers: usize) -> (bool, String) {
    if ninliers < min_inliers {
        return (false, format!("only {ninliers} inliers (need {min_inliers})"));
    }
    if (ninliers as f64) < 8.0 + 0.2 * nmatches as f64 {
        return (false, format!("{ninliers} inliers of {nmatches} matches is too few to be a real overlap"));
    }
    if !plausible(h, hgt, wid) {
        return (false, "the transform folds or strongly distorts the image".into());
    }
    (true, String::new())
}

/// Could H come from rotating the camera? The mapped h×w image must stay a convex,
/// non-flipped quadrilateral whose area is between ¼ and 4× the original.
pub fn plausible(h: &M3, hgt: usize, wid: usize) -> bool {
    let (w, hh) = (wid as f64, hgt as f64);
    let q: Vec<Vector3<f64>> =
        [(1.0, 1.0), (w, 1.0), (w, hh), (1.0, hh)].iter().map(|&(x, y)| h * Vector3::new(x, y, 1.0)).collect();
    if !q.iter().all(|v| v[2] > 0.0) {
        return false;
    }
    let c: Vec<(f64, f64)> = q.iter().map(|v| (v[0] / v[2], v[1] / v[2])).collect();
    let cross = |k: usize| {
        let (a, b, d) = (c[k], c[(k + 1) % 4], c[(k + 2) % 4]);
        (b.0 - a.0) * (d.1 - b.1) - (b.1 - a.1) * (d.0 - b.0)
    };
    if !(0..4).all(|k| cross(k) > 0.0) {
        return false;
    }
    let area = 0.5 * (0..4).map(|k| c[k].0 * c[(k + 1) % 4].1 - c[(k + 1) % 4].0 * c[k].1).sum::<f64>().abs();
    let ratio = area / (w * hh);
    0.25 < ratio && ratio < 4.0
}

/// Maximum spanning tree over accepted pairs `edges` = (i, j, weight): adjacency lists.
pub fn spanning_tree(n: usize, edges: &[(usize, usize, f64)]) -> Vec<Vec<usize>> {
    let mut uf: Vec<usize> = (0..n).collect();
    fn find(uf: &mut [usize], mut x: usize) -> usize {
        while uf[x] != x {
            uf[x] = uf[uf[x]];
            x = uf[x];
        }
        x
    }
    let mut adj = vec![Vec::new(); n];
    let mut e: Vec<&(usize, usize, f64)> = edges.iter().collect();
    e.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    for &(i, j, _) in e {
        let (a, b) = (find(&mut uf, i), find(&mut uf, j));
        if a == b {
            continue;
        }
        uf[a] = b;
        adj[i].push(j);
        adj[j].push(i);
    }
    adj
}

/// Breadth-first walk of tree `adj` from `roots` (in order): (parent or None, order, component).
pub fn walk_tree(adj: &[Vec<usize>], roots: &[usize]) -> (Vec<Option<usize>>, Vec<usize>, Vec<usize>) {
    let n = adj.len();
    let mut parent = vec![None; n];
    let mut comp = vec![0usize; n];
    let mut order = Vec::with_capacity(n);
    let mut c = 0;
    for &r in roots {
        if comp[r] != 0 {
            continue;
        }
        c += 1;
        comp[r] = c;
        let mut queue = std::collections::VecDeque::from([r]);
        while let Some(v) = queue.pop_front() {
            order.push(v);
            for &u in &adj[v] {
                if comp[u] != 0 {
                    continue;
                }
                comp[u] = c;
                parent[u] = Some(v);
                queue.push_back(u);
            }
        }
    }
    (parent, order, comp)
}

/// Best reference image: the centre (fewest hops to the farthest image) of the largest group.
pub fn tree_center(adj: &[Vec<usize>]) -> usize {
    let n = adj.len();
    if n == 0 {
        return 0;
    }
    let all: Vec<usize> = (0..n).collect();
    let (_, _, comp) = walk_tree(adj, &all);
    let ncomp = *comp.iter().max().unwrap_or(&1);
    let big = (1..=ncomp).max_by_key(|c| comp.iter().filter(|x| **x == *c).count()).unwrap_or(1);
    let (mut best, mut beste) = (0usize, usize::MAX);
    for v in 0..n {
        if comp[v] != big {
            continue;
        }
        let (p, order, _) = walk_tree(adj, &[v]);
        let mut depth = vec![0usize; n];
        for &u in &order {
            if u != v {
                depth[u] = depth[p[u].unwrap()] + 1;
            }
        }
        let e = order.iter().map(|&u| depth[u]).max().unwrap_or(0);
        if e < beste {
            best = v;
            beste = e;
        }
    }
    best
}

/// Transforms of all images into the frame of `ref_`, following the tree.
/// `h[(i, j)]` maps image j into image i. Images not connected to `ref_` are
/// placed side by side to its right. Each result is scaled to determinant 1.
pub fn place(h: &HashMap<(usize, usize), M3>, parent: &[Option<usize>], order: &[usize], ref_: usize, widths: &[usize]) -> Vec<M3> {
    let n = parent.len();
    let mut g = vec![M3::identity(); n];
    let mut x = 1.05 * widths[ref_] as f64;
    for &v in order {
        match parent[v] {
            None => {
                if v != ref_ {
                    g[v] = translation(x, 0.0);
                    x += 1.05 * widths[v] as f64;
                }
            }
            Some(p) => {
                g[v] = match h.get(&(p, v)) {
                    Some(hpv) => g[p] * hpv,
                    None => g[p] * inv(&h[&(v, p)]),
                };
            }
        }
    }
    g.iter().map(|gi| gi / gi.determinant().cbrt()).collect()
}

/// Focal length (px) implied by homographies of a rotating camera (Szeliski's
/// method, as in OpenCV). `pairs` holds (H, c_i, c_j) with H mapping image j
/// into image i and c the image centres. None if no pair gives an estimate.
pub fn estimate_focal(pairs: &[(M3, Pt, Pt)]) -> Option<f64> {
    let mut est = Vec::new();
    for (hm, c1, c2) in pairs {
        let hh = translation(-c1[0], -c1[1]) * hm * translation(c2[0], c2[1]);
        // row-major
        let mut h = [hh[(0, 0)], hh[(0, 1)], hh[(0, 2)], hh[(1, 0)], hh[(1, 1)], hh[(1, 2)], hh[(2, 0)], hh[(2, 1)], hh[(2, 2)]];
        let h8 = h[8];
        h.iter_mut().for_each(|v| *v /= h8);
        let pick = |d1: f64, d2: f64, v1: f64, v2: f64| -> f64 {
            let (mut v1, mut v2) = (v1, v2);
            if v1 < v2 {
                std::mem::swap(&mut v1, &mut v2);
            }
            if v1 > 0.0 && v2 > 0.0 {
                (if d1.abs() > d2.abs() { v1 } else { v2 }).sqrt()
            } else if v1 > 0.0 {
                v1.sqrt()
            } else {
                f64::NAN
            }
        };
        let f1 = {
            let d1 = h[6] * h[7];
            let d2 = (h[7] - h[6]) * (h[7] + h[6]);
            let v1 = if d1 != 0.0 { -(h[0] * h[1] + h[3] * h[4]) / d1 } else { -1.0 };
            let v2 = if d2 != 0.0 { (h[0] * h[0] + h[3] * h[3] - h[1] * h[1] - h[4] * h[4]) / d2 } else { -1.0 };
            pick(d1, d2, v1, v2)
        };
        let f0 = {
            let d1 = h[0] * h[3] + h[1] * h[4];
            let d2 = h[0] * h[0] + h[1] * h[1] - h[3] * h[3] - h[4] * h[4];
            let v1 = if d1 != 0.0 { -h[2] * h[5] / d1 } else { -1.0 };
            let v2 = if d2 != 0.0 { (h[5] * h[5] - h[2] * h[2]) / d2 } else { -1.0 };
            pick(d1, d2, v1, v2)
        };
        let f = (f0 * f1).sqrt();
        if f.is_finite() && f > 0.0 {
            est.push(f);
        }
    }
    if est.is_empty() {
        None
    } else {
        Some(median_f64(est))
    }
}

// --------------------------------------------------------------------------
// Global refinement (bundle adjustment of a rotating camera)
// --------------------------------------------------------------------------

fn skew(w: &Vector3<f64>) -> M3 {
    M3::new(0.0, -w[2], w[1], w[2], 0.0, -w[0], -w[1], w[0], 0.0)
}

/// Rotation matrix for the axis-angle vector `w`.
pub fn rodrigues(w: &Vector3<f64>) -> M3 {
    let th = w.norm();
    if th < 1e-12 {
        return M3::identity() + skew(w);
    }
    let k = skew(&(w / th));
    M3::identity() + th.sin() * k + (1.0 - th.cos()) * k * k
}

/// Closest rotation matrix to M.
pub fn nearest_rotation(m: &M3) -> M3 {
    let svd = m.svd(true, true);
    let (u, vt) = (svd.u.unwrap(), svd.v_t.unwrap());
    let r = u * vt;
    if r.determinant() < 0.0 {
        u * M3::from_diagonal(&Vector3::new(1.0, 1.0, -1.0)) * vt
    } else {
        r
    }
}

pub fn kmat(f: f64, c: Pt) -> M3 {
    M3::new(f, 0.0, c[0], 0.0, f, c[1], 0.0, 0.0, 1.0)
}

/// Inlier points of an accepted pair: (i, j, points in i, points in j).
pub type MatchSet = (usize, usize, Vec<Pt>, Vec<Pt>);

pub struct BundleResult {
    pub g: Vec<M3>,
    pub f: f64,
    pub k1: f64,
    pub rms_before: f64,
    pub rms_after: f64,
    /// Median error (px) of each entry of `matches`, NaN for pairs that were not used.
    pub med: Vec<f64>,
}

/// Refine all images together, assuming the camera only rotated and kept its
/// focal length: every accepted pair at once, the error spread evenly.
/// `g`: initial transforms (image → reference plane); `members`: images to
/// refine; `huber`: residuals above this (px) count less. With `distortion`
/// the lens's radial k1 is estimated too.
#[allow(clippy::too_many_arguments)]
pub fn bundle_adjust(
    g: &[M3],
    centers: &[Pt],
    ref_: usize,
    f: f64,
    members: &[usize],
    matches: &[MatchSet],
    iters: usize,
    huber: f64,
    maxpts: usize,
    distortion: bool,
) -> BundleResult {
    let n = g.len();
    let mut inset = vec![false; n];
    for &m in members {
        inset[m] = true;
    }
    let sub = |p: &[Pt]| -> Vec<Pt> {
        let k = p.len().min(maxpts);
        if k == 0 {
            return vec![];
        }
        (0..k).map(|t| p[((t as f64) * (p.len() - 1) as f64 / (k.max(2) - 1) as f64).round() as usize]).collect()
    };
    let used: Vec<usize> =
        (0..matches.len()).filter(|&k| inset[matches[k].0] && inset[matches[k].1] && !matches[k].2.is_empty()).collect();
    let e: Vec<MatchSet> = used.iter().map(|&k| (matches[k].0, matches[k].1, sub(&matches[k].2), sub(&matches[k].3))).collect();
    if e.is_empty() {
        return BundleResult { g: g.to_vec(), f, k1: 0.0, rms_before: f64::NAN, rms_after: f64::NAN, med: vec![f64::NAN; matches.len()] };
    }
    // Camera rotations from the initial transforms: G_i = K_ref R_iᵀ K_i⁻¹.
    let r0: Vec<M3> = (0..n)
        .map(|i| if inset[i] { nearest_rotation(&(inv(&kmat(f, centers[ref_])) * g[i] * kmat(f, centers[i]))).transpose() } else { M3::identity() })
        .collect();
    let free: Vec<usize> = members.iter().copied().filter(|&i| i != ref_).collect();
    let col: HashMap<usize, usize> = free.iter().enumerate().map(|(k, &i)| (i, 3 * k)).collect();
    let np = 3 * free.len() + 2; // + log focal length, k1
    let rot = |w: &[f64], i: usize| -> M3 {
        match col.get(&i) {
            Some(&c) => rodrigues(&Vector3::new(w[c], w[c + 1], w[c + 2])) * r0[i],
            None => r0[i],
        }
    };
    let resid = |ri: &M3, rj: &M3, fl: f64, k1: f64, e: &MatchSet| -> Vec<Pt> {
        let (i, j, pi, pj) = e;
        let h = kmat(fl, centers[*j]) * rj * ri.transpose() * inv(&kmat(fl, centers[*i]));
        let lens = (k1, fl);
        (0..pi.len())
            .map(|k| {
                let (ux, uy) = undistort(pi[k][0], pi[k][1], centers[*i], lens);
                let (x, y) = apply_h(&h, ux, uy);
                let (u, v) = undistort(pj[k][0], pj[k][1], centers[*j], lens);
                [x - u, y - v]
            })
            .collect()
    };
    let rho = |d: f64| if d <= huber { d * d } else { 2.0 * huber * d - huber * huber };
    let cost = |w: &[f64], lf: f64, k1: f64| -> f64 {
        e.iter().map(|ed| resid(&rot(w, ed.0), &rot(w, ed.1), lf.exp(), k1, ed).iter().map(|r| rho(r[0].hypot(r[1]))).sum::<f64>()).sum()
    };
    let rms = |w: &[f64], lf: f64, k1: f64| -> f64 {
        let (mut s, mut c) = (0.0, 0usize);
        for ed in &e {
            for r in resid(&rot(w, ed.0), &rot(w, ed.1), lf.exp(), k1, ed) {
                s += r[0] * r[0] + r[1] * r[1];
                c += 1;
            }
        }
        (s / c.max(1) as f64).sqrt()
    };
    let mut w = vec![0.0; np - 2];
    let mut lf = f.ln();
    let mut k1 = 0.0;
    let rms0 = rms(&w, lf, k1);
    let mut c0 = cost(&w, lf, k1);
    let mut lambda = 1e-3;
    for _ in 0..iters {
        let mut jtj = DMatrix::<f64>::zeros(np, np);
        let mut jtr = DVector::<f64>::zeros(np);
        for ed in &e {
            let (i, j) = (ed.0, ed.1);
            let (ri, rj, fl) = (rot(&w, i), rot(&w, j), lf.exp());
            let r = resid(&ri, &rj, fl, k1, ed);
            // Robust (Huber) weight per point.
            let wt: Vec<f64> = r.iter().map(|p| { let d = p[0].hypot(p[1]); if d <= huber { 1.0 } else { huber / d } }).collect();
            let mut cols: Vec<usize> = Vec::new();
            let mut js: Vec<Vec<f64>> = Vec::new(); // each column: 2m residual derivatives
            let flat = |r2: &[Pt], r: &[Pt]| -> Vec<f64> {
                let mut v = Vec::with_capacity(2 * r.len());
                for k in 0..r.len() {
                    v.push((r2[k][0] - r[k][0]) / 1e-6);
                    v.push((r2[k][1] - r[k][1]) / 1e-6);
                }
                v
            };
            for img in [i, j] {
                let Some(&c) = col.get(&img) else { continue };
                for a in 0..3 {
                    let mut w2 = w.clone();
                    w2[c + a] += 1e-6;
                    let r2m = rot(&w2, img);
                    let r2 = if img == i { resid(&r2m, &rj, fl, k1, ed) } else { resid(&ri, &r2m, fl, k1, ed) };
                    cols.push(c + a);
                    js.push(flat(&r2, &r));
                }
            }
            let r2 = resid(&ri, &rj, (lf + 1e-6).exp(), k1, ed);
            cols.push(np - 2);
            js.push(flat(&r2, &r));
            if distortion {
                let r2 = resid(&ri, &rj, fl, k1 + 1e-6, ed);
                cols.push(np - 1);
                js.push(flat(&r2, &r));
            }
            let m2 = 2 * r.len();
            let wv: Vec<f64> = (0..m2).map(|k| wt[k / 2]).collect();
            let rv: Vec<f64> = r.iter().flat_map(|p| [p[0], p[1]]).collect();
            for (a, ca) in cols.iter().enumerate() {
                let mut s = 0.0;
                for k in 0..m2 {
                    s += js[a][k] * wv[k] * rv[k];
                }
                jtr[*ca] += s;
                for (b, cb) in cols.iter().enumerate() {
                    let mut s = 0.0;
                    for k in 0..m2 {
                        s += js[a][k] * wv[k] * js[b][k];
                    }
                    jtj[(*ca, *cb)] += s;
                }
            }
        }
        let mut improved = false;
        for _ in 0..10 {
            let mut a = jtj.clone();
            for k in 0..np {
                a[(k, k)] += lambda * (jtj[(k, k)] + 1e-9);
            }
            let Some(delta) = a.lu().solve(&(-&jtr)) else { lambda *= 4.0; continue };
            let wn: Vec<f64> = (0..np - 2).map(|k| w[k] + delta[k]).collect();
            let lfn = lf + delta[np - 2];
            let k1n = (k1 + delta[np - 1]).clamp(-0.5, 0.5);
            let cn = cost(&wn, lfn, k1n);
            if cn < c0 {
                let rel = (c0 - cn) / c0;
                w = wn;
                lf = lfn;
                k1 = k1n;
                c0 = cn;
                lambda = (lambda / 3.0).max(1e-9);
                improved = rel >= 1e-7;
                break;
            }
            lambda *= 4.0;
        }
        if !improved {
            break;
        }
    }
    let fn_ = lf.exp();
    let mut gn = g.to_vec();
    for &i in members {
        let m = kmat(fn_, centers[ref_]) * rot(&w, i).transpose() * inv(&kmat(fn_, centers[i]));
        gn[i] = m / m.determinant().cbrt();
    }
    let mut med = vec![f64::NAN; matches.len()];
    for (k, ed) in used.iter().zip(e.iter()) {
        let r = resid(&rot(&w, ed.0), &rot(&w, ed.1), fn_, k1, ed);
        med[*k] = median_f64(r.iter().map(|p| p[0].hypot(p[1])));
    }
    BundleResult { g: gn, f: fn_, k1, rms_before: rms0, rms_after: rms(&w, lf, k1), med }
}

/// Camera rotation (yaw right, pitch down, roll clockwise; radians) as a
/// homography of the reference image plane.
pub fn rotation_h(yaw: f64, pitch: f64, f: f64, c: Pt, roll: f64) -> M3 {
    let k = kmat(f, c);
    let ry = M3::new(yaw.cos(), 0.0, yaw.sin(), 0.0, 1.0, 0.0, -yaw.sin(), 0.0, yaw.cos());
    let rx = M3::new(1.0, 0.0, 0.0, 0.0, pitch.cos(), pitch.sin(), 0.0, -pitch.sin(), pitch.cos());
    let rz = M3::new(roll.cos(), -roll.sin(), 0.0, roll.sin(), roll.cos(), 0.0, 0.0, 0.0, 1.0);
    k * rz * ry * rx * inv(&k)
}

/// Per-image brightness gains that make overlaps agree, propagated along the
/// tree. `points[(p, v)]` holds the inlier points (in image p, in image v) of
/// the pair linking image v to its parent p.
pub fn estimate_gains(imgs: &[&Img], parent: &[Option<usize>], order: &[usize], points: &HashMap<(usize, usize), (Vec<Pt>, Vec<Pt>)>) -> Vec<f64> {
    // Mean luminance of 7×7 patches around the points (tolerates small position errors).
    let lum = |img: &Img, p: &[Pt]| -> f64 {
        let (h, w) = (img.h as i64, img.w as i64);
        let mut s = 0.0;
        let mut n = 0usize;
        for q in p {
            for dy in -3..=3 {
                for dx in -3..=3 {
                    let y = (q[1].round() as i64 + dy).clamp(1, h) - 1;
                    let x = (q[0].round() as i64 + dx).clamp(1, w) - 1;
                    s += luminance(img.at(y as usize, x as usize)) as f64;
                    n += 1;
                }
            }
        }
        s / n.max(1) as f64
    };
    let mut g = vec![1.0; imgs.len()];
    for &v in order {
        let Some(p) = parent[v] else { continue };
        let Some((pp, pv)) = points.get(&(p, v)) else { continue };
        if pp.is_empty() {
            g[v] = g[p];
            continue;
        }
        g[v] = g[p] * lum(imgs[p], pp) / lum(imgs[v], pv).max(1e-6);
    }
    g
}

// --------------------------------------------------------------------------
// Scripted entry point (the window runs these same steps one at a time)
// --------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct StitchParams {
    pub layout: Layout,
    pub cols: usize,
    pub snake: bool,
    pub refine: bool,
    pub feature_scale: f64,
    pub nkeypoints: usize,
    pub denoise: f64,
    pub ratio: f64,
    pub max_distance: f64,
    pub model: Model,
    pub ransac_thresh: f64,
    pub min_inliers: usize,
    pub projection: Projection,
    pub blend: Blend,
    pub sharpness: f64,
}

impl Default for StitchParams {
    fn default() -> Self {
        Self {
            layout: Layout::All,
            cols: 0,
            snake: false,
            refine: true,
            feature_scale: 0.5,
            nkeypoints: 5000,
            denoise: 1.5,
            ratio: 0.8,
            max_distance: 0.35,
            model: Model::Homography,
            ransac_thresh: 5.0,
            min_inliers: 20,
            projection: Projection::Cylindrical,
            blend: Blend::Smart,
            sharpness: 3.0,
        }
    }
}

/// Stitch images (in memory). `layout` chooses which pairs are matched; `refine`
/// runs the global refinement (homography model only).
pub fn stitch(imgs: &[Img], p: &StitchParams, log: &mut dyn FnMut(String)) -> Result<Img, String> {
    let n = imgs.len();
    if n < 2 {
        return Err("Need at least two images.".into());
    }
    let dp = DetectParams { scale: p.feature_scale, nkeypoints: p.nkeypoints, denoise: p.denoise, ..Default::default() };
    let feats: Vec<Features> = imgs.par_iter().map(|im| detect(im, &dp)).collect();
    let mut hs: HashMap<(usize, usize), M3> = HashMap::new();
    let mut matches: Vec<MatchSet> = Vec::new();
    let cols = if p.cols == 0 { n } else { p.cols };
    for (i, j) in candidate_pairs(n, p.layout, cols, p.snake) {
        let (pi, pj) = match_pair(&feats[i], &feats[j], p.ratio, p.max_distance);
        let Ok((hij, inl)) = ransac(&pj, &pi, p.model, 2000, p.ransac_thresh) else { continue };
        if !accept_pair(pi.len(), inl.len(), &hij, imgs[j].h, imgs[j].w, p.min_inliers).0 {
            continue;
        }
        log(format!("Pair {}-{}: {} matches, {} inliers", i + 1, j + 1, pi.len(), inl.len()));
        hs.insert((i, j), hij);
        matches.push((i, j, inl.iter().map(|&k| pi[k]).collect(), inl.iter().map(|&k| pj[k]).collect()));
    }
    let centers: Vec<Pt> = imgs.iter().map(|im| [(im.w as f64 + 1.0) / 2.0, (im.h as f64 + 1.0) / 2.0]).collect();
    let adj = spanning_tree(n, &matches.iter().map(|m| (m.0, m.1, m.2.len() as f64)).collect::<Vec<_>>());
    let ref_ = tree_center(&adj);
    let roots: Vec<usize> = std::iter::once(ref_).chain(0..n).collect();
    let (parent, order, comp) = walk_tree(&adj, &roots);
    let members: Vec<usize> = (0..n).filter(|&i| comp[i] == comp[ref_]).collect();
    if members.len() < n {
        log(format!("Images {:?} are not connected.", (0..n).filter(|&i| comp[i] != comp[ref_]).map(|i| i + 1).collect::<Vec<_>>()));
    }
    let f = estimate_focal(&hs.iter().map(|(&(i, j), h)| (*h, centers[i], centers[j])).collect::<Vec<_>>())
        .unwrap_or(imgs[ref_].w.max(imgs[ref_].h) as f64);
    let g = place(&hs, &parent, &order, ref_, &imgs.iter().map(|im| im.w).collect::<Vec<_>>());
    let (g, f, lens) = if p.refine && p.model == Model::Homography {
        let b = bundle_adjust(&g, &centers, ref_, f, &members, &matches, 40, p.ransac_thresh, 60, true);
        log(format!("Global refinement: rms error {:.2} → {:.2} px, lens distortion k1 = {:.3}", b.rms_before, b.rms_after, b.k1));
        (b.g, b.f, (b.k1, b.f))
    } else {
        (g, f, (0.0, f))
    };
    let o: Vec<Vec<Pt>> = g.iter().zip(imgs).map(|(gi, im)| outline(gi, im.h, im.w, p.projection, f, centers[ref_], lens, 24)).collect();
    let refs: Vec<&Img> = imgs.iter().collect();
    let cp = ComposeParams { projection: p.projection, f, c: centers[ref_], blend: p.blend, lens, sharpness: p.sharpness, ..Default::default() };
    Ok(compose(&refs, &g, bounds(&o), &cp, &vec![1.0; n], &vec![1.0; n], &imgs.iter().map(|im| (im.h, im.w)).collect::<Vec<_>>(), false).0)
}

/// Stitch image files.
pub fn stitch_files(paths: &[String], p: &StitchParams, raw_half: bool, log: &mut dyn FnMut(String)) -> Result<Img, String> {
    let opt = crate::io::LoadOptions { half: raw_half, ..Default::default() };
    let imgs: Result<Vec<Img>, String> = paths.par_iter().map(|pth| crate::io::load_any(pth, &opt)).collect();
    stitch(&imgs?, p, log)
}

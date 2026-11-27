//! Wide-field plate solving: which stars of the sky a night photo shows.
//!
//! The stars of the photo become directions (rays) through the camera's focal
//! length. Triangles of its brightest stars are compared by their shape with
//! triangles of the brightest catalogue stars; every match gives a rotation
//! from the camera to the sky, and the one under which most stars of the photo
//! land on catalogue stars wins. All the stars then refine the rotation, the
//! focal length and the lens distortion. Made for wide fields (about 10° to
//! 120° across) and needs no hint of where the camera pointed.
//!
//! The catalogue (`assets/stars/stars.bin`, see `tools/make_star_catalog.py`)
//! holds the Hipparcos and Tycho-2 stars to magnitude 8.

use super::render::{distort, undistort, Lens};
use super::{rodrigues, Pt, M3};
use crate::image::{Bits, Gray, Img};
use nalgebra::{SMatrix, SVector, Vector3};
use rayon::prelude::*;
use std::sync::OnceLock;

type V3 = Vector3<f64>;

/// Photos with at least this share of sky are solved in a panorama (the
/// landscape of the others would make "stars" of lights and grass). Measured
/// from the stars by `sky_mask`: a photo of pure sky comes out at about 1.
pub const SKY_SHARE: f64 = 0.95;
/// Fewest identified stars for a solution.
pub const MIN_SOLVED: usize = 20;

/// The star catalogue, brightest first.
pub struct Catalog {
    /// Unit vectors (equatorial, J2000 axes, epoch 2025).
    pub dir: Vec<V3>,
    pub mag: Vec<f32>,
}

static STARS: &[u8] = include_bytes!("../../assets/stars/stars.bin");

pub fn catalog() -> &'static Catalog {
    static C: OnceLock<Catalog> = OnceLock::new();
    C.get_or_init(|| {
        let b = STARS;
        assert!(b.len() >= 12 && &b[..8] == b"PSSTARS1", "bad star catalogue");
        let n = u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize;
        let (mut dir, mut mag) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for k in 0..n {
            let o = 12 + 10 * k;
            let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f64;
            let (ra, de) = (f(o).to_radians(), f(o + 4).to_radians());
            dir.push(V3::new(de.cos() * ra.cos(), de.cos() * ra.sin(), de.sin()));
            mag.push(i16::from_le_bytes([b[o + 8], b[o + 9]]) as f32 / 100.0);
        }
        Catalog { dir, mag }
    })
}

/// Right ascension and declination (degrees) of a direction.
pub fn radec(v: &V3) -> (f64, f64) {
    (v[1].atan2(v[0]).to_degrees().rem_euclid(360.0), v[2].clamp(-1.0, 1.0).asin().to_degrees())
}

/// Direction of a right ascension and declination (degrees).
pub fn direction(ra: f64, dec: f64) -> V3 {
    let (a, d) = (ra.to_radians(), dec.to_radians());
    V3::new(d.cos() * a.cos(), d.cos() * a.sin(), d.sin())
}

/// A solved photo: camera ray `v` points at the sky direction `r · v`.
#[derive(Clone, Debug)]
pub struct Solution {
    pub r: M3,
    /// Focal length (px) and radial distortion (as `render::distort` with fc = f).
    pub f: f64,
    pub k1: f64,
    pub size: (usize, usize),
    /// Identified stars: catalogue index, position in the photo (1-based px).
    pub stars: Vec<(u32, Pt)>,
    /// Mean distance (px) between the stars and their catalogue positions.
    pub rms: f64,
}

impl Solution {
    fn c(&self) -> Pt {
        [(self.size.1 as f64 + 1.0) / 2.0, (self.size.0 as f64 + 1.0) / 2.0]
    }

    /// Where a sky direction lands in the photo (it may be outside the frame).
    pub fn project(&self, s: &V3) -> Option<Pt> {
        project(&self.r, self.f, self.k1, self.c(), s)
    }

    /// The sky direction of a pixel.
    pub fn sky(&self, p: Pt) -> V3 {
        self.r * ray(p, self.c(), self.f, self.k1)
    }

    /// Right ascension and declination of the frame's centre (degrees).
    pub fn centre(&self) -> (f64, f64) {
        radec(&(self.r * V3::new(0.0, 0.0, 1.0)))
    }

    /// Field of view (degrees): across the width, across the height.
    pub fn fov(&self) -> (f64, f64) {
        let (h, w) = (self.size.0 as f64, self.size.1 as f64);
        (2.0 * (w / 2.0 / self.f).atan().to_degrees(), 2.0 * (h / 2.0 / self.f).atan().to_degrees())
    }
}

fn ray(p: Pt, c: Pt, f: f64, k1: f64) -> V3 {
    let (x, y) = undistort(p[0], p[1], c, Lens::new(k1, 0.0, f));
    V3::new(x - c[0], y - c[1], f).normalize()
}

fn project(r: &M3, f: f64, k1: f64, c: Pt, s: &V3) -> Option<Pt> {
    let v = r.transpose() * s;
    if v[2] < 0.05 {
        return None;
    }
    let (x, y) = distort(c[0] + f * v[0] / v[2], c[1] + f * v[1] / v[2], c, Lens::new(k1, 0.0, f));
    Some([x, y])
}

/// Rotation R with R·b ≈ a (least squares).
pub fn kabsch(a: &[V3], b: &[V3]) -> M3 {
    let mut h = M3::zeros();
    for (x, y) in a.iter().zip(b) {
        h += x * y.transpose();
    }
    let svd = h.svd(true, true);
    let (u, vt) = (svd.u.unwrap(), svd.v_t.unwrap());
    let d = (u * vt).determinant().signum();
    u * M3::from_diagonal(&V3::new(1.0, 1.0, d)) * vt
}

fn ang(a: &V3, b: &V3) -> f64 {
    a.dot(b).clamp(-1.0, 1.0).acos()
}

/// Directions on a cube map, for "which star is near this direction".
struct SkyGrid {
    n: usize,
    cells: Vec<Vec<u32>>,
}

impl SkyGrid {
    fn face(v: &V3) -> (usize, f64, f64) {
        let a = v.abs();
        let (ax, b, c) = if a[0] >= a[1] && a[0] >= a[2] { (0, 1, 2) } else if a[1] >= a[2] { (1, 2, 0) } else { (2, 0, 1) };
        let s = v[ax];
        (2 * ax + (s < 0.0) as usize, v[b] / s.abs(), v[c] / s.abs())
    }

    fn cell(&self, u: f64, w: f64) -> (i64, i64) {
        let n = self.n as f64;
        (((u + 1.0) / 2.0 * n).floor() as i64, ((w + 1.0) / 2.0 * n).floor() as i64)
    }

    /// `cell`: smallest search radius (radians).
    fn new(dir: &[V3], idx: impl Iterator<Item = usize>, cell: f64) -> Self {
        let n = ((2.0 / cell.max(1e-4)).floor() as usize).clamp(1, 1024);
        let mut g = SkyGrid { n, cells: vec![Vec::new(); 6 * n * n] };
        let margin = 2.0 / n as f64;
        for k in idx {
            let v = dir[k];
            // Into every face it falls on (with a margin), so that a search near
            // the edge of a face finds the stars just across it.
            for ax in 0..3 {
                for neg in [false, true] {
                    let s = if neg { -v[ax] } else { v[ax] };
                    if s < 0.4 {
                        continue;
                    }
                    let (b, c) = ((ax + 1) % 3, (ax + 2) % 3);
                    let (u, w) = (v[b] / s, v[c] / s);
                    if u.abs() > 1.0 + margin || w.abs() > 1.0 + margin {
                        continue;
                    }
                    let (i, j) = g.cell(u.clamp(-1.0, 1.0 - 1e-12), w.clamp(-1.0, 1.0 - 1e-12));
                    let f = 2 * ax + neg as usize;
                    g.cells[(f * n + i as usize) * n + j as usize].push(k as u32);
                }
            }
        }
        g
    }

    /// The nearest listed direction to `q` within the angle whose cosine is `cos_tol`.
    fn nearest(&self, dir: &[V3], q: &V3, cos_tol: f64) -> Option<(u32, f64)> {
        let (f, u, w) = Self::face(q);
        let (i, j) = self.cell(u.clamp(-1.0, 1.0 - 1e-12), w.clamp(-1.0, 1.0 - 1e-12));
        let n = self.n as i64;
        let mut best: Option<(u32, f64)> = None;
        for di in -1..=1 {
            for dj in -1..=1 {
                let (a, b) = (i + di, j + dj);
                if a < 0 || b < 0 || a >= n || b >= n {
                    continue;
                }
                for &k in &self.cells[((f as i64 * n + a) * n + b) as usize] {
                    let d = dir[k as usize].dot(q);
                    if d > cos_tol && best.is_none_or(|(_, bd)| d > bd) {
                        best = Some((k, d));
                    }
                }
            }
        }
        best
    }
}

/// Triangles of a star set, by their shape: (shortest / longest side, middle /
/// longest, longest side (rad), vertices opposite the sides shortest first,
/// handedness).
type Tri = (f64, f64, f64, [u32; 3], bool);

fn triangle(v: &[V3], i: usize, j: usize, k: usize, smin: f64, smax: f64) -> Option<Tri> {
    let (a, b, c) = (ang(&v[j], &v[k]), ang(&v[i], &v[k]), ang(&v[i], &v[j]));
    let mut s = [(a, i), (b, j), (c, k)];
    s.sort_by(|x, y| x.0.total_cmp(&y.0));
    if s[0].0 < smin || s[2].0 > smax {
        return None;
    }
    let (p, q, r) = (s[0].1, s[1].1, s[2].1);
    let hand = v[p].dot(&v[q].cross(&v[r])) > 0.0;
    Some((s[0].0 / s[2].0, s[1].0 / s[2].0, s[2].0, [p as u32, q as u32, r as u32], hand))
}

/// Solve a photo from its stars: positions (1-based px) brightest first, the
/// photo's size (h, w) and a guess of its focal length (px; within about 15%).
pub fn solve(stars: &[Pt], size: (usize, usize), f_guess: f64) -> Result<Solution, String> {
    let cat = catalog();
    let (h, w) = (size.0 as f64, size.1 as f64);
    let c = [(w + 1.0) / 2.0, (h + 1.0) / 2.0];
    let diag = w.hypot(h);
    if stars.len() < MIN_SOLVED {
        return Err(format!("only {} stars found", stars.len()));
    }
    // Solid angle of the frame, and how many catalogue stars it holds to a given count over the sky.
    let half = |s: f64| (s / 2.0 / f_guess).atan();
    let omega = 4.0 * (half(w).sin() * half(h).sin()).asin();
    let share = omega / (4.0 * std::f64::consts::PI);
    let over_sky = |in_frame: f64| ((in_frame / share) as usize).clamp(30, cat.dir.len());

    // Hypotheses: triangles of the brightest stars.
    let nb = stars.len().min(22);
    let ncat = over_sky(2.0 * nb as f64).min(4000);
    let smax = 2.0 * (0.33 * diag / f_guess).atan();
    let smin = smax * 0.06;
    let cv = &cat.dir;
    let bright: Vec<usize> = (0..ncat).collect();
    let mut ctris: Vec<Tri> = Vec::new();
    {
        let cmax = smax.cos();
        let nbr: Vec<Vec<usize>> = bright.par_iter().map(|&i| bright.iter().copied().filter(|&j| j > i && cv[i].dot(&cv[j]) >= cmax).collect()).collect();
        for &i in &bright {
            for (a, &j) in nbr[i].iter().enumerate() {
                for &k in &nbr[i][a + 1..] {
                    if let Some(t) = triangle(cv, i, j, k, smin, smax) {
                        ctris.push(t);
                    }
                }
            }
        }
    }
    // Shape grid over (a/c, b/c).
    const NG: usize = 64;
    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); NG * NG];
    let gi = |x: f64| ((x * NG as f64) as usize).min(NG - 1);
    for (k, t) in ctris.iter().enumerate() {
        grid[gi(t.0) * NG + gi(t.1)].push(k as u32);
    }
    // Stars checked per hypothesis, and the catalogue they are checked against.
    let kv = stars.len().min(60);
    let nver = over_sky(2.5 * kv as f64).max(ncat);
    let tol0 = (0.008 * diag).max(4.0) / f_guess;
    let vgrid = SkyGrid::new(cv, 0..nver, tol0);
    let rays0: Vec<V3> = stars.iter().map(|&p| ray(p, c, f_guess, 0.0)).collect();
    let mut itris: Vec<Tri> = Vec::new();
    for i in 0..nb {
        for j in i + 1..nb {
            for k in j + 1..nb {
                if let Some(t) = triangle(&rays0, i, j, k, smin, smax) {
                    itris.push(t);
                }
            }
        }
    }
    let tol_shape = 0.012;
    let best = itris
        .par_iter()
        .map(|it| {
            let mut best = (0usize, M3::identity(), f_guess);
            let (x0, x1) = (gi((it.0 - tol_shape).max(0.0)), gi(it.0 + tol_shape));
            let (y0, y1) = (gi((it.1 - tol_shape).max(0.0)), gi(it.1 + tol_shape));
            for gx in x0..=x1 {
                for gy in y0..=y1 {
                    for &ck in &grid[gx * NG + gy] {
                        let ct = &ctris[ck as usize];
                        if ct.4 != it.4 || (ct.0 - it.0).abs() > tol_shape || (ct.1 - it.1).abs() > tol_shape {
                            continue;
                        }
                        let scale = it.2 / ct.2;
                        if !(0.8..1.25).contains(&scale) {
                            continue;
                        }
                        // The size of the triangle corrects the focal length.
                        let f = f_guess * scale;
                        let iv: Vec<V3> = it.3.iter().map(|&k| ray(stars[k as usize], c, f, 0.0)).collect();
                        let sv: Vec<V3> = ct.3.iter().map(|&k| cv[k as usize]).collect();
                        let r = kabsch(&sv, &iv);
                        if (0..3).any(|q| sv[q].dot(&(r * iv[q])) < (2.0 * tol0).cos()) {
                            continue;
                        }
                        let cos_tol = tol0.cos();
                        let hit = |k: usize| vgrid.nearest(cv, &(r * ray(stars[k], c, f, 0.0)), cos_tol).is_some();
                        // A quick look at a few more stars first.
                        let extra = (0..kv.min(nb + 8)).filter(|k| !it.3.contains(&(*k as u32))).take(8).filter(|&k| hit(k)).count();
                        if extra < 3 {
                            continue;
                        }
                        let n = (0..kv).filter(|&k| hit(k)).count();
                        if n > best.0 {
                            best = (n, r, f);
                        }
                    }
                }
            }
            best
        })
        .reduce(|| (0, M3::identity(), f_guess), |a, b| if b.0 > a.0 { b } else { a });
    if best.0 < (kv / 4).max(10) {
        return Err(format!("no match with the star catalogue (best {} of {} stars)", best.0, kv));
    }
    refine(stars, size, best.1, best.2)
}

/// All the stars refine a rough solution: catalogue stars are projected into
/// the photo and paired with the nearest stars found, and the rotation, focal
/// length and distortion are fitted to them (Levenberg–Marquardt).
fn refine(stars: &[Pt], size: (usize, usize), r0: M3, f0: f64) -> Result<Solution, String> {
    let cat = catalog();
    let (h, w) = (size.0 as f64, size.1 as f64);
    let c = [(w + 1.0) / 2.0, (h + 1.0) / 2.0];
    let diag = w.hypot(h);
    let (mut r, mut f, mut k1) = (r0, f0, 0.0f64);
    // Stars found, on a pixel grid.
    let cellpx = 8.0;
    let (gw, gh) = ((w / cellpx) as usize + 2, (h / cellpx) as usize + 2);
    let mut pgrid: Vec<Vec<u32>> = vec![Vec::new(); gw * gh];
    for (k, p) in stars.iter().enumerate() {
        let (x, y) = ((p[0] / cellpx) as usize, (p[1] / cellpx) as usize);
        if x < gw && y < gh {
            pgrid[y * gw + x].push(k as u32);
        }
    }
    let nearest = |q: Pt, tol: f64| -> Option<(u32, f64)> {
        let rad = (tol / cellpx).ceil() as i64;
        let (x, y) = ((q[0] / cellpx) as i64, (q[1] / cellpx) as i64);
        let mut best: Option<(u32, f64)> = None;
        for yy in (y - rad).max(0)..=(y + rad).min(gh as i64 - 1) {
            for xx in (x - rad).max(0)..=(x + rad).min(gw as i64 - 1) {
                for &k in &pgrid[yy as usize * gw + xx as usize] {
                    let p = stars[k as usize];
                    let d = (p[0] - q[0]).hypot(p[1] - q[1]);
                    if d < tol && best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((k, d));
                    }
                }
            }
        }
        best
    };
    // Catalogue stars that can be in the frame: deep enough for about three times the stars found.
    let in_frame = |r: &M3, f: f64, k1: f64| -> Vec<(u32, Pt)> {
        let z = r * V3::new(0.0, 0.0, 1.0);
        let cmin = ((0.5 * diag / f).atan() + 0.05).cos();
        let mut out = Vec::new();
        for (k, s) in cat.dir.iter().enumerate() {
            if s.dot(&z) < cmin {
                continue;
            }
            if let Some(p) = project(r, f, k1, c, s) {
                if p[0] >= 1.0 && p[0] <= w && p[1] >= 1.0 && p[1] <= h {
                    out.push((k as u32, p));
                    if out.len() >= 3 * stars.len() {
                        break;
                    }
                }
            }
        }
        out
    };
    // Nearest star for each catalogue star; each star taken once (by the nearest).
    let pair_up = |r: &M3, f: f64, k1: f64, tol: f64| -> Vec<(u32, u32, f64)> {
        let mut cand: Vec<(u32, u32, f64)> = in_frame(r, f, k1).iter().filter_map(|&(k, p)| nearest(p, tol).map(|(s, d)| (k, s, d))).collect();
        cand.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut used = vec![false; stars.len()];
        cand.retain(|&(_, s, _)| !std::mem::replace(&mut used[s as usize], true));
        cand
    };
    let tol0 = (0.008 * diag).max(4.0);
    let mut tol = tol0;
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    let mut rms = f64::INFINITY;
    for it in 0..8 {
        pairs = pair_up(&r, f, k1, tol).iter().map(|&(k, s, _)| (k, s)).collect();
        if pairs.len() < 8 {
            return Err(format!("only {} stars identified", pairs.len()));
        }
        // Distortion only with enough stars spread over the frame.
        let fit_k1 = it >= 2 && pairs.len() >= 30;
        (r, f, k1) = fit(&pairs, stars, c, r, f, k1, fit_k1);
        let res: Vec<f64> = pairs
            .iter()
            .filter_map(|&(k, s)| project(&r, f, k1, c, &cat.dir[k as usize]).map(|p| (p[0] - stars[s as usize][0]).hypot(p[1] - stars[s as usize][1])))
            .collect();
        rms = (res.iter().map(|d| d * d).sum::<f64>() / res.len().max(1) as f64).sqrt();
        tol = (3.0 * rms).clamp(2.5, tol);
    }
    // A real solution puts the stars right on the catalogue: nearly all the pairs
    // within the first, wide tolerance are much closer. Chance pairs of random
    // dots fill that circle evenly (and grow with its area).
    let wide = pair_up(&r, f, k1, tol0);
    let close = wide.iter().filter(|p| p.2 < tol0 / 3.0).count();
    if (close as f64) < 0.6 * wide.len() as f64 || rms > 0.004 * diag + 1.0 {
        return Err(format!("no match with the star catalogue ({close} of {} stars close, {rms:.1} px)", wide.len()));
    }
    if pairs.len() < MIN_SOLVED {
        return Err(format!("only {} stars identified", pairs.len()));
    }
    if !(k1.abs() < 0.5 && f.is_finite() && f > 0.0) {
        return Err("the fit did not converge".into());
    }
    let stars_out = pairs.iter().map(|&(k, s)| (k, stars[s as usize])).collect();
    Ok(Solution { r, f, k1, size, stars: stars_out, rms })
}

/// Least squares over rotation (small turn ω), focal length and k1.
fn fit(pairs: &[(u32, u32)], stars: &[Pt], c: Pt, r: M3, f: f64, k1: f64, with_k1: bool) -> (M3, f64, f64) {
    let cat = catalog();
    type P = SVector<f64, 5>;
    let model = |p: &P| -> (M3, f64, f64) { (r * rodrigues(&V3::new(p[0], p[1], p[2])), f * (1.0 + p[3]), k1 + p[4]) };
    let resid = |p: &P| -> Vec<f64> {
        let (rr, ff, kk) = model(p);
        let mut out = Vec::with_capacity(2 * pairs.len());
        for &(k, s) in pairs {
            let q = project(&rr, ff, kk, c, &cat.dir[k as usize]).unwrap_or([1e6, 1e6]);
            // Robust: far-off pairs count less (Cauchy-like weight).
            let (dx, dy) = (q[0] - stars[s as usize][0], q[1] - stars[s as usize][1]);
            let wgt = 1.0 / (1.0 + (dx * dx + dy * dy) / 16.0).sqrt();
            out.push(dx * wgt.sqrt());
            out.push(dy * wgt.sqrt());
        }
        out
    };
    let np = if with_k1 { 5 } else { 4 };
    let mut p = P::zeros();
    let mut lambda = 1e-3;
    let cost = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>();
    let mut e = resid(&p);
    let mut ce = cost(&e);
    for _ in 0..12 {
        let mut jtj = SMatrix::<f64, 5, 5>::zeros();
        let mut jte = P::zeros();
        let eps = [1e-6, 1e-6, 1e-6, 1e-6, 1e-5];
        let cols: Vec<Vec<f64>> = (0..np)
            .map(|q| {
                let mut d = p;
                d[q] += eps[q];
                resid(&d).iter().zip(&e).map(|(a, b)| (a - b) / eps[q]).collect()
            })
            .collect();
        for a in 0..np {
            jte[a] = cols[a].iter().zip(&e).map(|(x, y)| x * y).sum();
            for b in 0..np {
                jtj[(a, b)] = cols[a].iter().zip(&cols[b]).map(|(x, y)| x * y).sum();
            }
        }
        if np == 4 {
            jtj[(4, 4)] = 1.0;
        }
        let mut done = false;
        for _ in 0..8 {
            let mut m = jtj;
            for a in 0..5 {
                m[(a, a)] *= 1.0 + lambda;
            }
            let Some(dp) = m.lu().solve(&(-jte)) else { break };
            let pn = p + dp;
            let en = resid(&pn);
            let cn = cost(&en);
            if cn < ce {
                done = cn > ce * (1.0 - 1e-9);
                p = pn;
                e = en;
                ce = cn;
                lambda = (lambda * 0.3).max(1e-9);
                break;
            }
            lambda *= 10.0;
        }
        if done {
            break;
        }
    }
    model(&p)
}

/// Where a photo shows sky (cells of about 1/24 of its width, true = sky),
/// from its stars: the photo is cut into cells,
/// and a cell is ground when it has almost no stars (under 15 % of the typical
/// cell's) and joins the photo's border through such cells (the ground comes
/// in from an edge; a starless patch inside the sky, a dark lane of the Milky
/// Way or a cloud, does not make ground). A photo of pure sky comes out at
/// 1.0, unlike `detect_sky`'s horizon search (made for landscapes with the
/// sky at the top), which finds a few per cent of "ground" in it and much
/// "sky" in grass. Averaged over 3 × 3 cells against chance gaps; all ground
/// when there are hardly any stars.
pub fn sky_mask(img: &Img) -> Bits {
    let none = || Bits::from_fn(1, 1, |_, _| false);
    let l = img.luminance();
    let (h, w) = (l.h, l.w);
    if h < 64 || w < 64 {
        return none();
    }
    let hp = crate::image::sub(&l, &crate::image::blur(&l, 2.0));
    let hv: Vec<f32> = hp.subsample(hp.step_for(200_000)).collect();
    let sigma = crate::image::median_mad(&hv).1 + 1e-7;
    let nx = 24usize;
    let c = w.div_ceil(nx);
    let ny = h.div_ceil(c);
    let mut d = vec![0usize; nx * ny];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let p = hp.at(y, x);
            if p > 6.0 * sigma && p >= hp.at(y - 1, x) && p >= hp.at(y + 1, x) && p >= hp.at(y, x - 1) && p >= hp.at(y, x + 1) {
                d[(y / c) * nx + x / c] += 1;
            }
        }
    }
    // Stars per full cell (the cells at the right and bottom edges are smaller).
    let area = |k: usize| {
        let (i, j) = (k / nx, k % nx);
        let (ch, cw) = (((i + 1) * c).min(h) - i * c, ((j + 1) * c).min(w) - j * c);
        (ch * cw) as f64 / (c * c) as f64
    };
    let raw: Vec<f64> = (0..nx * ny).map(|k| d[k] as f64 / area(k).max(0.05)).collect();
    // Averaged over the 3 × 3 neighbourhood: a cell holds only a few stars, so
    // single cells of thin sky (haze, a dark lane) would look starless by chance.
    let dens: Vec<f64> = (0..nx * ny)
        .map(|k| {
            let (i, j) = ((k / nx) as i64, (k % nx) as i64);
            let (mut s, mut n) = (0.0, 0.0);
            for di in -1..=1 {
                for dj in -1..=1 {
                    let (ii, jj) = (i + di, j + dj);
                    if ii >= 0 && jj >= 0 && (ii as usize) < ny && (jj as usize) < nx {
                        s += raw[ii as usize * nx + jj as usize];
                        n += 1.0;
                    }
                }
            }
            s / n
        })
        .collect();
    let mut sorted = dens.clone();
    sorted.sort_by(f64::total_cmp);
    let typical = sorted[sorted.len() * 4 / 5];
    if typical < 3.0 {
        return none(); // hardly any stars anywhere
    }
    let starless: Vec<bool> = dens.iter().map(|&n| n < 0.15 * typical).collect();
    // Ground: starless cells reachable from the border through starless cells.
    let mut ground = vec![false; nx * ny];
    let mut stack: Vec<usize> = (0..nx * ny).filter(|&k| (k / nx == 0 || k / nx == ny - 1 || k % nx == 0 || k % nx == nx - 1) && starless[k]).collect();
    while let Some(k) = stack.pop() {
        if ground[k] {
            continue;
        }
        ground[k] = true;
        let (i, j) = (k / nx, k % nx);
        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let (ii, jj) = (i as i64 + di, j as i64 + dj);
            if ii >= 0 && jj >= 0 && (ii as usize) < ny && (jj as usize) < nx {
                let q = ii as usize * nx + jj as usize;
                if starless[q] && !ground[q] {
                    stack.push(q);
                }
            }
        }
    }
    Bits::from_fn(ny, nx, |i, j| !ground[i * nx + j])
}

/// Share of a photo that is sky (see `sky_mask`).
pub fn sky_share(img: &Img) -> f64 {
    let m = sky_mask(img);
    m.v.iter().filter(|&&s| s).count() as f64 / m.v.len().max(1) as f64
}

/// The stars of a photo's sky (1-based px), brightest first.
pub fn photo_stars(img: &Img) -> Vec<Pt> {
    let (h, w) = (img.h, img.w);
    if h < 64 || w < 64 {
        return vec![];
    }
    let mask = sky_mask(img);
    let (mh, mw) = (mask.h, mask.w);
    let wt = Gray::from_fn(h, w, |y, x| if mask.v[(y * mh / h).min(mh - 1) * mw + (x * mw / w).min(mw - 1)] { 1.0 } else { 0.0 });
    crate::stacker::detect_stars(img, &wt).iter().map(|s| [s.x, s.y]).collect()
}

/// Solve a photo: its stars and a guess of its focal length (px). A first try
/// at the guess; then shorter and longer (a zoom lens may have crept, and a
/// guess from the EXIF focal length is off by the crop factor of a DX lens).
pub fn solve_photo(img: &Img, f_guess: f64) -> Result<Solution, String> {
    let stars = photo_stars(img);
    let size = (img.h, img.w);
    let mut err = String::new();
    for s in [1.0, 0.8, 1.25, 0.64, 1.56] {
        match solve(&stars, size, f_guess * s) {
            Ok(sol) => return Ok(sol),
            Err(e) => {
                if err.is_empty() {
                    err = e;
                }
            }
        }
    }
    Err(err)
}

/// Stars two solved photos share: (in the first, in the second), by catalogue star.
pub fn common_stars(a: &Solution, b: &Solution) -> (Vec<Pt>, Vec<Pt>) {
    let map: std::collections::HashMap<u32, Pt> = b.stars.iter().copied().collect();
    a.stars.iter().filter_map(|(k, p)| map.get(k).map(|q| (*p, *q))).unzip()
}

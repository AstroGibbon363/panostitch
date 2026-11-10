//! Projections, lens distortion, outlines, rendering (`compose`) and
//! multi-band blending.
//!
//! A viewing direction (X, Y, Z) of the reference camera (x right, y down, z
//! forward) maps to unit-free coordinates (a, b); the output pixel grid is
//! (u, v) = f·(a, b), so one unit ≈ one reference pixel near the centre.

use super::{inv, kmat, Pt, M3};
use crate::image::{bilinear, blur, luminance, Bits, Gray, Image, Img, Px, Rgb};
use rayon::prelude::*;
use std::collections::HashMap;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Projection {
    Rectilinear,
    Cylindrical,
    Spherical,
    Mercator,
    Stereographic,
    TransverseCylindrical,
    TransverseMercator,
    Fisheye,
    FisheyeEquisolid,
    LittlePlanet,
    Tunnel,
    Orthographic,
    Hammer,
    Mollweide,
}

impl Projection {
    pub const ALL: [Projection; 14] = [
        Projection::Rectilinear,
        Projection::Cylindrical,
        Projection::Spherical,
        Projection::Mercator,
        Projection::Stereographic,
        Projection::TransverseCylindrical,
        Projection::TransverseMercator,
        Projection::Fisheye,
        Projection::FisheyeEquisolid,
        Projection::LittlePlanet,
        Projection::Tunnel,
        Projection::Orthographic,
        Projection::Hammer,
        Projection::Mollweide,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Projection::Rectilinear => "rectilinear",
            Projection::Cylindrical => "cylindrical",
            Projection::Spherical => "spherical",
            Projection::Mercator => "mercator",
            Projection::Stereographic => "stereographic",
            Projection::TransverseCylindrical => "transverse_cylindrical",
            Projection::TransverseMercator => "transverse_mercator",
            Projection::Fisheye => "fisheye",
            Projection::FisheyeEquisolid => "fisheye_equisolid",
            Projection::LittlePlanet => "little_planet",
            Projection::Tunnel => "tunnel",
            Projection::Orthographic => "orthographic",
            Projection::Hammer => "hammer",
            Projection::Mollweide => "mollweide",
        }
    }
    pub fn from_name(s: &str) -> Option<Projection> {
        Projection::ALL.iter().copied().find(|p| p.name() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    Smart,
    Feather,
    Seam,
    Average,
}

impl Blend {
    pub const ALL: [Blend; 4] = [Blend::Smart, Blend::Feather, Blend::Seam, Blend::Average];
    pub fn name(self) -> &'static str {
        match self {
            Blend::Smart => "smart",
            Blend::Feather => "feather",
            Blend::Seam => "seam",
            Blend::Average => "average",
        }
    }
    pub fn from_name(s: &str) -> Option<Blend> {
        Blend::ALL.iter().copied().find(|b| b.name() == s)
    }
}

/// Largest |a|, |b| each projection can show (keeps canvases finite).
pub fn proj_limits(p: Projection) -> (f64, f64) {
    use Projection::*;
    match p {
        Rectilinear => (4.0, 4.0),
        Cylindrical => (PI, 4.0),
        Spherical => (PI, PI / 2.0),
        Mercator => (PI, 3.0),
        TransverseCylindrical => (4.0, PI),
        TransverseMercator => (3.0, PI),
        Fisheye => (PI, PI),
        FisheyeEquisolid => (2.0, 2.0),
        Orthographic => (1.0, 1.0),
        Hammer | Mollweide => (2.0 * 2f64.sqrt(), 2f64.sqrt()),
        LittlePlanet | Tunnel => (8.0, 8.0),
        Stereographic => (4.0, 4.0),
    }
}

/// Stereographic projection of a direction around the +Z axis.
#[inline]
fn stereo(x: f64, y: f64, z: f64, zmin: f64) -> (f64, f64) {
    let n = (x * x + y * y + z * z).sqrt();
    if z / n <= zmin {
        return (f64::NAN, f64::NAN);
    }
    let k = 2.0 / (1.0 + z / n);
    (k * x / n, k * y / n)
}

/// Mollweide auxiliary angle θ with 2θ + sin 2θ = π sin φ (Newton's method).
#[inline]
fn mollweide_theta(phi: f64) -> f64 {
    if phi.abs() >= PI / 2.0 - 1e-9 {
        return (PI / 2.0).copysign(phi);
    }
    let mut th = phi;
    for _ in 0..30 {
        let d = (2.0 * th + (2.0 * th).sin() - PI * phi.sin()) / (2.0 + 2.0 * (2.0 * th).cos());
        th -= d;
        if d.abs() < 1e-10 {
            break;
        }
    }
    th
}

/// Direction → (a, b); NaN if the projection cannot show it.
#[inline]
pub fn proj_forward(p: Projection, x: f64, y: f64, z: f64) -> (f64, f64) {
    use Projection::*;
    let (a, b) = match p {
        Rectilinear => {
            if z <= 1e-9 {
                return (f64::NAN, f64::NAN);
            }
            (x / z, y / z)
        }
        Stereographic => stereo(x, y, z, -0.9),
        LittlePlanet => stereo(x, -z, y, -0.97), // nadir in the centre
        Tunnel => stereo(x, z, -y, -0.97),       // zenith in the centre
        Orthographic => {
            let n = (x * x + y * y + z * z).sqrt();
            if z <= 0.0 {
                return (f64::NAN, f64::NAN);
            }
            (x / n, y / n)
        }
        Fisheye | FisheyeEquisolid => {
            let n = (x * x + y * y + z * z).sqrt();
            let th = (z / n).clamp(-1.0, 1.0).acos();
            let r = if p == Fisheye { th } else { 2.0 * (th / 2.0).sin() };
            let q = x.hypot(y);
            if q < 1e-12 {
                (0.0, 0.0)
            } else {
                (r * x / q, r * y / q)
            }
        }
        TransverseCylindrical | TransverseMercator => {
            let b = y.atan2(z);
            let rho = y.hypot(z);
            (if p == TransverseCylindrical { x / rho } else { (x / rho).asinh() }, b)
        }
        Hammer | Mollweide => {
            let lam = x.atan2(z);
            let phi = y.atan2(x.hypot(z));
            if p == Hammer {
                let zz = (1.0 + phi.cos() * (lam / 2.0).cos()).sqrt();
                (2.0 * 2f64.sqrt() * phi.cos() * (lam / 2.0).sin() / zz, 2f64.sqrt() * phi.sin() / zz)
            } else {
                let th = mollweide_theta(phi);
                (2.0 * 2f64.sqrt() / PI * lam * th.cos(), 2f64.sqrt() * th.sin())
            }
        }
        _ => {
            let a = x.atan2(z);
            let rho = x.hypot(z);
            let b = match p {
                Cylindrical => y / rho,
                Spherical => y.atan2(rho),
                _ => (y / rho).asinh(),
            };
            (a, b)
        }
    };
    let (la, lb) = proj_limits(p);
    (a.clamp(-la, la), b.clamp(-lb, lb))
}

/// (a, b) → direction (X, Y, Z); None outside the projection's range.
#[inline]
pub fn proj_inverse(p: Projection, a: f64, b: f64) -> Option<(f64, f64, f64)> {
    use Projection::*;
    let (la, lb) = proj_limits(p);
    if a.abs() > la || b.abs() > lb {
        return None;
    }
    Some(match p {
        Rectilinear => (a, b, 1.0),
        Stereographic | LittlePlanet | Tunnel => {
            let r2 = a * a + b * b;
            let (x, y, z) = (4.0 * a, 4.0 * b, 4.0 - r2);
            match p {
                Stereographic => (x, y, z),
                LittlePlanet => (x, z, -y),
                _ => (x, -z, y),
            }
        }
        Orthographic => {
            let r2 = a * a + b * b;
            if r2 > 1.0 {
                return None;
            }
            (a, b, (1.0 - r2).sqrt())
        }
        Fisheye | FisheyeEquisolid => {
            let r = a.hypot(b);
            if p == FisheyeEquisolid && r > 2.0 {
                return None;
            }
            let th = if p == Fisheye { r } else { 2.0 * (r / 2.0).asin() };
            if th > PI {
                return None;
            }
            if r < 1e-12 {
                return Some((0.0, 0.0, 1.0));
            }
            (th.sin() * a / r, th.sin() * b / r, th.cos())
        }
        TransverseCylindrical => (a, b.sin(), b.cos()),
        TransverseMercator => (a.sinh(), b.sin(), b.cos()),
        Hammer => {
            let z2 = 1.0 - (a / 4.0).powi(2) - (b / 2.0).powi(2);
            if z2 < 0.5 || a * a / 8.0 + b * b / 2.0 > 1.0 {
                return None;
            }
            let z = z2.sqrt();
            let lam = 2.0 * (z * a).atan2(2.0 * (2.0 * z2 - 1.0));
            let phi = (z * b).clamp(-1.0, 1.0).asin();
            (lam.sin() * phi.cos(), phi.sin(), lam.cos() * phi.cos())
        }
        Mollweide => {
            if b.abs() > 2f64.sqrt() {
                return None;
            }
            let th = (b / 2f64.sqrt()).asin();
            if th.cos().abs() < 1e-12 {
                return None;
            }
            let lam = PI * a / (2.0 * 2f64.sqrt() * th.cos());
            if lam.abs() > PI {
                return None;
            }
            let phi = ((2.0 * th + (2.0 * th).sin()) / PI).clamp(-1.0, 1.0).asin();
            (lam.sin() * phi.cos(), phi.sin(), lam.cos() * phi.cos())
        }
        Cylindrical => (a.sin(), b, a.cos()),
        _ => {
            let phi = if p == Spherical { b } else { b.sinh().atan() };
            (a.sin() * phi.cos(), phi.sin(), a.cos() * phi.cos())
        }
    })
}

// --------------------------------------------------------------------------
// Lens distortion: observed = c + (ideal - c)·(1 + k1·r²), r in units of the
// camera focal length fc. `lens = (k1, fc)`.
// --------------------------------------------------------------------------

pub type Lens = (f64, f64);

/// Ideal (distortion-free) pixel → observed pixel.
#[inline]
pub fn distort(x: f64, y: f64, c: Pt, lens: Lens) -> (f64, f64) {
    let (k1, fc) = lens;
    if k1 == 0.0 {
        return (x, y);
    }
    let (dx, dy) = ((x - c[0]) / fc, (y - c[1]) / fc);
    let s = 1.0 + k1 * (dx * dx + dy * dy);
    (c[0] + dx * s * fc, c[1] + dy * s * fc)
}

/// Observed pixel → ideal pixel (inverse of `distort`).
#[inline]
pub fn undistort(x: f64, y: f64, c: Pt, lens: Lens) -> (f64, f64) {
    let (k1, fc) = lens;
    if k1 == 0.0 {
        return (x, y);
    }
    let (ox, oy) = ((x - c[0]) / fc, (y - c[1]) / fc);
    let (mut ux, mut uy) = (ox, oy);
    for _ in 0..6 {
        let s = 1.0 + k1 * (ux * ux + uy * uy);
        ux = ox / s;
        uy = oy / s;
    }
    (c[0] + ux * fc, c[1] + uy * fc)
}

/// Outline of an h×w image placed by G (image → reference plane), in output
/// (u, v) coordinates: `n` points per edge, NaN where the projection cannot show it.
pub fn outline(g: &M3, h: usize, w: usize, p: Projection, f: f64, c: Pt, lens: Lens, n: usize) -> Vec<Pt> {
    let (wf, hf) = (w as f64, h as f64);
    let ci = [(wf + 1.0) / 2.0, (hf + 1.0) / 2.0];
    let lin = |a: f64, b: f64| (0..n).map(move |k| a + (b - a) * k as f64 / (n - 1).max(1) as f64);
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(4 * n);
    pts.extend(lin(1.0, wf).map(|x| (x, 1.0)));
    pts.extend(lin(1.0, hf).map(|y| (wf, y)));
    pts.extend(lin(wf, 1.0).map(|x| (x, hf)));
    pts.extend(lin(hf, 1.0).map(|y| (1.0, y)));
    pts.into_iter()
        .map(|(x, y)| {
            let (ux, uy) = undistort(x, y, ci, lens);
            let q = g * nalgebra::Vector3::new(ux, uy, 1.0);
            let (a, b) = proj_forward(p, (q[0] - c[0] * q[2]) / f, (q[1] - c[1] * q[2]) / f, q[2]);
            [f * a, f * b]
        })
        .collect()
}

/// Bounding box (umin, umax, vmin, vmax) of outlines.
pub fn bounds(outlines: &[Vec<Pt>]) -> (f64, f64, f64, f64) {
    let mut any = false;
    let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for o in outlines {
        for p in o {
            if p[0].is_finite() && p[1].is_finite() {
                any = true;
                u0 = u0.min(p[0]);
                u1 = u1.max(p[0]);
                v0 = v0.min(p[1]);
                v1 = v1.max(p[1]);
            }
        }
    }
    if !any {
        return (-1.0, 1.0, -1.0, 1.0);
    }
    (u0.floor(), u1.ceil(), v0.floor(), v1.ceil())
}

// --------------------------------------------------------------------------
// Rendering
// --------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ComposeParams {
    pub scale: f64,
    pub projection: Projection,
    pub f: f64,
    pub c: Pt,
    pub blend: Blend,
    pub lens: Lens,
    /// Feathering weight = (distance to the border)^sharpness.
    pub sharpness: f64,
    /// Relative brightness difference above which `Smart` treats photos as disagreeing.
    pub deghost: f64,
}

impl Default for ComposeParams {
    fn default() -> Self {
        Self { scale: 1.0, projection: Projection::Rectilinear, f: 1000.0, c: [0.0, 0.0], blend: Blend::Feather, lens: (0.0, 1.0), sharpness: 1.0, deghost: 0.12 }
    }
}

/// Output pixel centre px (1-based) ↔ u = (px-0.5)/scale + umin - 0.5.
struct Sampler<'a> {
    imgs: &'a [&'a Img],
    ms: Vec<M3>,
    sizes: &'a [(usize, usize)],
    scales: &'a [f64],
    gains: &'a [f64],
    rlim2: Vec<f64>,
    p: &'a ComposeParams,
    umin: f64,
    vmin: f64,
}

impl Sampler<'_> {
    /// Colour, blending weight and distance from the border (full-resolution px)
    /// of image i at output pixel (x, y) (1-based), or None.
    #[inline]
    fn sample(&self, i: usize, x: f64, y: f64) -> Option<(Rgb, f32, f64)> {
        let p = self.p;
        let (xx, yy, zz) = proj_inverse(p.projection, ((x - 0.5) / p.scale + self.umin - 0.5) / p.f, ((y - 0.5) / p.scale + self.vmin - 0.5) / p.f)?;
        let m = &self.ms[i];
        let sw = m[(2, 0)] * xx + m[(2, 1)] * yy + m[(2, 2)] * zz;
        if sw <= 0.0 {
            return None; // behind this camera
        }
        let (h, w) = self.sizes[i];
        let ci = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
        let ix = (m[(0, 0)] * xx + m[(0, 1)] * yy + m[(0, 2)] * zz) / sw;
        let iy = (m[(1, 0)] * xx + m[(1, 1)] * yy + m[(1, 2)] * zz) / sw;
        if ((ix - ci[0]).powi(2) + (iy - ci[1]).powi(2)) / p.lens.1.powi(2) > self.rlim2[i] {
            return None;
        }
        let (ox, oy) = distort(ix, iy, ci, p.lens);
        if !(1.0 <= ox && ox <= w as f64 && 1.0 <= oy && oy <= h as f64) {
            return None;
        }
        // Feathering: weight grows with distance from the image border.
        let dist = ox.min(w as f64 - ox + 1.0).min(oy).min(h as f64 - oy + 1.0);
        let wt = if p.blend == Blend::Average { 1.0f32 } else { (dist as f32 / 100.0).powf(p.sharpness as f32) };
        let s = self.scales[i];
        let col = bilinear(self.imgs[i], (oy - 0.5) * s + 0.5, (ox - 0.5) * s + 0.5).scale(self.gains[i] as f32);
        Some((col, wt, dist))
    }
}

fn tocanvas(u: f64, v: f64, umin: f64, vmin: f64, scale: f64) -> (f64, f64) {
    ((u - umin + 0.5) * scale + 0.5, (v - vmin + 0.5) * scale + 0.5)
}

/// Pixel box (x0, x1, y0, y1; 1-based inclusive) of an outline on the canvas, or None.
fn outline_box(o: &[Pt], umin: f64, vmin: f64, scale: f64, wc: usize, hc: usize) -> Option<(usize, usize, usize, usize)> {
    let cs: Vec<(f64, f64)> = o.iter().filter(|p| p[0].is_finite() && p[1].is_finite()).map(|p| tocanvas(p[0], p[1], umin, vmin, scale)).collect();
    if cs.is_empty() {
        return None;
    }
    let fmin = |f: &dyn Fn(&(f64, f64)) -> f64| cs.iter().map(f).fold(f64::MAX, f64::min);
    let fmax = |f: &dyn Fn(&(f64, f64)) -> f64| cs.iter().map(f).fold(f64::MIN, f64::max);
    let x0 = (fmin(&|p| p.0).floor() as i64).clamp(1, wc as i64) as usize;
    let x1 = (fmax(&|p| p.0).ceil() as i64).clamp(1, wc as i64) as usize;
    let y0 = (fmin(&|p| p.1).floor() as i64).clamp(1, hc as i64) as usize;
    let y1 = (fmax(&|p| p.1).ceil() as i64).clamp(1, hc as i64) as usize;
    Some((x0, x1, y0, y1))
}

/// Render images into the output box (umin, umax, vmin, vmax) at `p.scale`.
/// - `g[i]`: maps (distortion-free) full-resolution pixels of image i into the reference plane
/// - `imgs[i]`: the pixels to sample, which may be image i resized by `scales[i]`; `sizes[i]` the full size (h, w)
/// - `coverage`: also return the mask of the output pixels any photo covers
#[allow(clippy::too_many_arguments)]
pub fn compose(imgs: &[&Img], g: &[M3], box_: (f64, f64, f64, f64), p: &ComposeParams, gains: &[f64], scales: &[f64], sizes: &[(usize, usize)], coverage: bool) -> (Img, Option<Bits>) {
    let (umin, umax, vmin, vmax) = box_;
    let wc = (((umax - umin + 1.0) * p.scale).round() as usize).max(1);
    let hc = (((vmax - vmin + 1.0) * p.scale).round() as usize).max(1);
    let k = kmat(p.f, p.c);
    let ms: Vec<M3> = g.iter().map(|gi| inv(gi) * k).collect(); // direction → ideal full-res pixel
    // Lens distortion is only meaningful within a photo: only directions within
    // each photo's own extent (its undistorted corners, plus a margin) may sample it.
    let rlim2: Vec<f64> = sizes
        .iter()
        .map(|&(h, w)| {
            let ci = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
            let corners = [(1.0, 1.0), (w as f64, 1.0), (1.0, h as f64), (w as f64, h as f64)];
            1.05f64.powi(2)
                * corners
                    .iter()
                    .map(|&(x, y)| {
                        let (ux, uy) = undistort(x, y, ci, p.lens);
                        ((ux - ci[0]).powi(2) + (uy - ci[1]).powi(2)) / p.lens.1.powi(2)
                    })
                    .fold(f64::MIN, f64::max)
        })
        .collect();
    let s = Sampler { imgs, ms, sizes, scales, gains, rlim2, p, umin, vmin };
    let n = imgs.len();
    let smart = p.blend == Blend::Smart;
    let mut acc = Img::new(hc, wc);
    let mut wsum = Gray::new(hc, wc);
    let mut best = if smart { Img::new(hc, wc) } else { Img::new(0, 0) };
    let mut bestw = if smart { Gray::new(hc, wc) } else { Gray::new(0, 0) };
    let mut besti: Vec<i32> = vec![0; if smart { hc * wc } else { 0 }];
    let outlines: Vec<Vec<Pt>> = (0..n).map(|i| outline(&g[i], sizes[i].0, sizes[i].1, p.projection, p.f, p.c, p.lens, 24)).collect();
    for i in 0..n {
        let Some((x0, x1, y0, y1)) = outline_box(&outlines[i], umin, vmin, p.scale, wc, hc) else { continue };
        let rows = acc.px.par_chunks_mut(wc).zip(wsum.px.par_chunks_mut(wc)).enumerate().skip(y0 - 1).take(y1 - y0 + 1);
        if smart {
            rows.zip(best.px.par_chunks_mut(wc).zip(bestw.px.par_chunks_mut(wc)).zip(besti.par_chunks_mut(wc)).skip(y0 - 1).take(y1 - y0 + 1)).for_each(
                |((y, (arow, wrow)), ((brow, bwrow), birow))| {
                    for x in x0..=x1 {
                        let Some((col, wt, _)) = s.sample(i, x as f64, (y + 1) as f64) else { continue };
                        arow[x - 1] = arow[x - 1].add(col.scale(wt));
                        wrow[x - 1] += wt;
                        if wt > bwrow[x - 1] {
                            brow[x - 1] = col;
                            bwrow[x - 1] = wt;
                            birow[x - 1] = (i + 1) as i32;
                        }
                    }
                },
            );
        } else {
            let seam = p.blend == Blend::Seam;
            rows.for_each(|(y, (arow, wrow))| {
                for x in x0..=x1 {
                    let Some((col, wt, _)) = s.sample(i, x as f64, (y + 1) as f64) else { continue };
                    if seam {
                        if wt > wrow[x - 1] {
                            arow[x - 1] = col;
                            wrow[x - 1] = wt;
                        }
                    } else {
                        arow[x - 1] = arow[x - 1].add(col.scale(wt));
                        wrow[x - 1] += wt;
                    }
                }
            });
        }
    }
    let cover = if coverage { Some(Bits { w: wc, h: hc, v: wsum.px.iter().map(|&w| w > 0.0).collect() }) } else { None };
    if p.blend == Blend::Seam {
        return (acc, cover);
    }
    let out = acc.zip_map(&wsum, |a, ws| if ws > 0.0 { a.scale(1.0 / ws) } else { Rgb::zero() });
    if !smart {
        return (out, cover);
    }
    // Deghosting. Where the blend differs clearly from the most central photo,
    // the overlapping photos disagree (something moved, or parallax).
    let dg = p.deghost as f32;
    let mask = out.zip_map(&best, |o, b| {
        let (lo, lb) = (luminance(o), luminance(b));
        let d = (lo - lb).abs() / (lo.max(lb) + 0.02);
        ((d - dg) / dg).clamp(0.0, 1.0)
    });
    let region: Vec<bool> = blur(&mask, (3.0 * p.scale).max(1.0)).px.iter().map(|&v| v > 0.02).collect();
    // Which photo each output pixel takes its detail from: the most central one,
    // except that each connected area of disagreement comes from ONE photo (the
    // one dominating it), so a moved person or light stays whole.
    let mut sel = besti.clone();
    let mut seen = vec![false; hc * wc];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for x0 in 0..wc {
        for y0 in 0..hc {
            if !region[y0 * wc + x0] || seen[y0 * wc + x0] {
                continue;
            }
            let mut votes: HashMap<i32, f64> = HashMap::new();
            let mut pix: Vec<(usize, usize)> = Vec::new();
            seen[y0 * wc + x0] = true;
            stack.push((y0, x0));
            while let Some((y, x)) = stack.pop() {
                pix.push((y, x));
                let bi = besti[y * wc + x];
                if bi > 0 {
                    *votes.entry(bi).or_insert(0.0) += bestw.px[y * wc + x] as f64;
                }
                for (dy, dx) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                    let (yy, xx) = (y as i64 + dy, x as i64 + dx);
                    if yy < 0 || xx < 0 || yy >= hc as i64 || xx >= wc as i64 {
                        continue;
                    }
                    let k = yy as usize * wc + xx as usize;
                    if region[k] && !seen[k] {
                        seen[k] = true;
                        stack.push((yy as usize, xx as usize));
                    }
                }
            }
            if votes.is_empty() {
                continue;
            }
            let kbest = votes.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(k, _)| *k).unwrap();
            let ki = (kbest - 1) as usize;
            // Not right up to the chosen photo's edge: the switch happens inside
            // the overlap, where the multi-band blend makes it gradual.
            let margin = 0.06 * sizes[ki].0.min(sizes[ki].1) as f64;
            for (y, x) in pix {
                if let Some((_, _, dist)) = s.sample(ki, (x + 1) as f64, (y + 1) as f64) {
                    if dist >= margin {
                        sel[y * wc + x] = kbest;
                    }
                }
            }
        }
    }
    (multiband(&s, n, &sel, &wsum, hc, wc, &outlines), cover)
}

/// Largest axis-aligned rectangle of `true` cells in `m`, as (row range, column
/// range; 0-based, inclusive), or None. Used for auto-crop.
pub fn largest_rectangle(m: &Bits) -> Option<((usize, usize), (usize, usize))> {
    let (hh, ww) = (m.h, m.w);
    let mut heights = vec![0usize; ww];
    let mut best = None;
    let mut bestarea = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    for r in 0..hh {
        for c in 0..ww {
            heights[c] = if m.at(r, c) { heights[c] + 1 } else { 0 };
        }
        stack.clear();
        for c in 0..=ww {
            let h = if c < ww { heights[c] } else { 0 };
            while let Some(&top) = stack.last() {
                if heights[top] < h {
                    break;
                }
                stack.pop();
                let left = stack.last().map(|&l| l + 1).unwrap_or(0);
                let area = heights[top] * (c - left);
                if area > bestarea {
                    bestarea = area;
                    best = Some(((r + 1 - heights[top], r), (left, c - 1)));
                }
            }
            if c < ww {
                stack.push(c);
            }
        }
    }
    best
}

// One step down / up a Gaussian pyramid: level-l pixel i sits at level-(l+1) position (i+1)/2.
fn pyr_down<T: Px>(a: &Image<T>) -> Image<T> {
    let b = blur(a, 1.0);
    let (h, w) = (a.h.div_ceil(2), a.w.div_ceil(2));
    Image::from_fn(h, w, |i, j| b.at(2 * i, 2 * j))
}
fn pyr_up<T: Px>(a: &Image<T>, h: usize, w: usize) -> Image<T> {
    Image::from_fn(h, w, |i, j| bilinear(a, (i as f64 + 2.0) / 2.0, (j as f64 + 2.0) / 2.0))
}

/// Multi-band blending (Burt & Adelson): fine detail comes from the photo chosen
/// in `sel` for each output pixel, coarser brightness variations are blended
/// over wider and wider zones.
fn multiband(s: &Sampler, n: usize, sel: &[i32], covered: &Gray, hc: usize, wc: usize, outlines: &[Vec<Pt>]) -> Img {
    let lmax = ((hc.min(wc) as f64 / 16.0).log2().floor() as i64).clamp(0, 7) as usize;
    let p = s.p;
    if lmax == 0 {
        return Img::from_fn(hc, wc, |y, x| {
            let k = sel[y * wc + x];
            if covered.px[y * wc + x] > 0.0 && k > 0 {
                s.sample((k - 1) as usize, (x + 1) as f64, (y + 1) as f64).map(|r| r.0).unwrap_or(Rgb::zero())
            } else {
                Rgb::zero()
            }
        });
    }
    let mut dims = vec![(hc, wc)];
    for _ in 0..lmax {
        let (h, w) = *dims.last().unwrap();
        dims.push((h.div_ceil(2), w.div_ceil(2)));
    }
    let mut num: Vec<Img> = dims.iter().map(|&(h, w)| Img::new(h, w)).collect();
    let mut den: Vec<Gray> = dims.iter().map(|&(h, w)| Gray::new(h, w)).collect();
    let step = 1usize << lmax;
    for i in 0..n {
        let Some((bx0, bx1, by0, by1)) = outline_box(&outlines[i], s.umin, s.vmin, p.scale, wc, hc) else { continue };
        // Image bounding box, aligned to the coarsest level so the levels line up.
        let x0 = 1 + step * ((bx0 - 1) / step);
        let y0 = 1 + step * ((by0 - 1) / step);
        let (x1, y1) = (bx1, by1);
        if x1 < x0 || y1 < y0 {
            continue;
        }
        let (h, w) = (y1 - y0 + 1, x1 - x0 + 1);
        let mut iv = Img::new(h, w);
        let mut vl = Gray::new(h, w);
        let mut wl = Gray::new(h, w);
        iv.px.par_chunks_mut(w).zip(vl.px.par_chunks_mut(w)).zip(wl.px.par_chunks_mut(w)).enumerate().for_each(|(r, ((irow, vrow), wrow))| {
            let y = y0 + r;
            for x in x0..=x1 {
                if let Some((col, _, _)) = s.sample(i, x as f64, y as f64) {
                    irow[x - x0] = col;
                    vrow[x - x0] = 1.0;
                    if sel[(y - 1) * wc + (x - 1)] == (i + 1) as i32 {
                        wrow[x - x0] = 1.0;
                    }
                }
            }
        });
        if wl.px.iter().all(|&v| v == 0.0) {
            continue;
        }
        // Gaussian pyramids; colours normalised by coverage so the area outside
        // the photo does not darken its border.
        let mut gi: Vec<Img> = Vec::new();
        let mut gv: Vec<Gray> = Vec::new();
        let mut gw: Vec<Gray> = Vec::new();
        for l in 0..=lmax {
            gi.push(iv.zip_map(&vl, |c, v| if v > 1e-3 { c.scale(1.0 / v) } else { Rgb::zero() }));
            gv.push(vl.clone());
            gw.push(wl.clone());
            if l == lmax {
                break;
            }
            iv = pyr_down(&iv);
            vl = pyr_down(&vl);
            wl = pyr_down(&wl);
        }
        // Pull-push: outside the photo, continue each level smoothly from the coarser one.
        {
            let valid: Vec<bool> = gv[lmax].px.iter().map(|&v| v > 1e-3).collect();
            let top = &gi[lmax];
            let cnt = valid.iter().filter(|b| **b).count();
            let fillc = if cnt > 0 {
                let mut sacc = Rgb::zero();
                for (k, ok) in valid.iter().enumerate() {
                    if *ok {
                        sacc = sacc.add(top.px[k]);
                    }
                }
                sacc.scale(1.0 / cnt as f32)
            } else {
                Rgb::zero()
            };
            let px: Vec<Rgb> = top.px.iter().zip(valid.iter()).map(|(c, ok)| if *ok { *c } else { fillc }).collect();
            gi[lmax] = Img::from_vec(top.h, top.w, px);
        }
        for l in (0..lmax).rev() {
            let up = pyr_up(&gi[l + 1], gi[l].h, gi[l].w);
            let px: Vec<Rgb> = (0..gi[l].len()).map(|k| if gv[l].px[k] > 1e-3 { gi[l].px[k] } else { up.px[k] }).collect();
            gi[l] = Img::from_vec(gi[l].h, gi[l].w, px);
        }
        for l in 0..=lmax {
            let lap = if l == lmax {
                gi[l].clone()
            } else {
                let up = pyr_up(&gi[l + 1], gi[l].h, gi[l].w);
                crate::image::sub(&gi[l], &up)
            };
            let (oy, ox) = ((y0 - 1) >> l, (x0 - 1) >> l);
            let (dh, dw) = dims[l];
            if oy >= dh || ox >= dw {
                continue;
            }
            let hh = lap.h.min(dh - oy);
            let ww = lap.w.min(dw - ox);
            if hh == 0 || ww == 0 {
                continue;
            }
            let numl = &mut num[l];
            let denl = &mut den[l];
            for r in 0..hh {
                for c in 0..ww {
                    let wgt = gw[l].px[r * lap.w + c];
                    let k = (oy + r) * dw + ox + c;
                    numl.px[k] = numl.px[k].add(lap.px[r * lap.w + c].scale(wgt));
                    denl.px[k] += wgt;
                }
            }
        }
    }
    let lev: Vec<Img> = (0..=lmax).map(|l| num[l].zip_map(&den[l], |c, d| if d > 1e-6 { c.scale(1.0 / d) } else { Rgb::zero() })).collect();
    let mut out = lev[lmax].clone();
    for l in (0..lmax).rev() {
        out = crate::image::add(&pyr_up(&out, dims[l].0, dims[l].1), &lev[l]);
    }
    out.zip_map(covered, |c, w| if w > 0.0 { c } else { Rgb::zero() })
}

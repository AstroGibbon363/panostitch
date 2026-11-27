//! Auto horizon: find the horizon of a panorama (or take points the user
//! clicked on it), smooth it with a curve of the 4th order and turn the view
//! (pitch and roll; yaw keeps the framing) so that it becomes straight.
//!
//! The angles are found on the sphere, not on the picture: every point of the
//! horizon is a direction, and the view is turned until all of them lie at the
//! same height (cylindrical, spherical, Mercator, Hammer, Mollweide: a line of
//! constant height is a straight line across the picture) or on the middle
//! line (rectilinear: only there is the horizon straight). Projections that
//! bend every line of constant height (little planet, tunnel, fisheye,
//! stereographic, transverse ones) are not offered.

use super::render::{proj_forward, proj_inverse, Projection};
use super::{Pt, M3};
use crate::image::{Bits, Img};
use nalgebra::Vector3;

type V3 = Vector3<f64>;

/// Can the horizon be straightened in this projection?
pub fn supported(p: Projection) -> bool {
    use Projection::*;
    matches!(p, Rectilinear | Cylindrical | Spherical | Mercator | Hammer | Mollweide)
}

/// The view's turn (degrees; yaw right, pitch down, roll clockwise) as a
/// rotation of directions, the same as `rotation_h` does to the reference plane.
pub fn view_rotation(yaw: f64, pitch: f64, roll: f64) -> M3 {
    let (y, p, r) = (yaw.to_radians(), pitch.to_radians(), roll.to_radians());
    let ry = M3::new(y.cos(), 0.0, y.sin(), 0.0, 1.0, 0.0, -y.sin(), 0.0, y.cos());
    let rx = M3::new(1.0, 0.0, 0.0, 0.0, p.cos(), p.sin(), 0.0, -p.sin(), p.cos());
    let rz = M3::new(r.cos(), -r.sin(), 0.0, r.sin(), r.cos(), 0.0, 0.0, 0.0, 1.0);
    rz * ry * rx
}

/// Least-squares polynomial y(x) of `degree` (lower when there are fewer
/// points), coefficients from the constant up. x is scaled to −1…1 over
/// `span` for a well-conditioned fit.
#[derive(Clone, Debug)]
pub struct Poly {
    pub c: Vec<f64>,
    pub x0: f64,
    pub half: f64,
}

impl Poly {
    pub fn at(&self, x: f64) -> f64 {
        let t = (x - self.x0) / self.half;
        self.c.iter().rev().fold(0.0, |acc, &k| acc * t + k)
    }
}

/// Fit y(x) by weighted least squares.
fn poly_fit(pts: &[Pt], wts: &[f64], degree: usize) -> Option<Poly> {
    let n = pts.len();
    if n == 0 {
        return None;
    }
    let deg = degree.min(n - 1);
    let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
    let (x0, half) = ((lo + hi) / 2.0, ((hi - lo) / 2.0).max(1e-9));
    let m = deg + 1;
    let mut a = nalgebra::DMatrix::<f64>::zeros(m, m);
    let mut b = nalgebra::DVector::<f64>::zeros(m);
    for (p, &w) in pts.iter().zip(wts) {
        let t = (p[0] - x0) / half;
        let pw: Vec<f64> = (0..m).map(|k| t.powi(k as i32)).collect();
        for i in 0..m {
            b[i] += w * pw[i] * p[1];
            for j in 0..m {
                a[(i, j)] += w * pw[i] * pw[j];
            }
        }
    }
    for i in 0..m {
        a[(i, i)] += 1e-9;
    }
    let c = a.lu().solve(&b)?;
    Some(Poly { c: c.iter().copied().collect(), x0, half })
}

/// The curve through points marked by hand: least squares of the 4th order,
/// lower for fewer points (through 4 points, a curve of the 3rd order passes exactly).
pub fn curve_through(pts: &[Pt], degree: usize) -> Option<Poly> {
    poly_fit(pts, &vec![1.0; pts.len()], degree)
}

/// A robust fit of the 4th order (lower for few points): points far from the
/// curve (a tree, a mountain, a lamp) weigh less and less, so the curve
/// follows the bulk of the horizon. Returns the curve, the points' spread
/// around it (px) and each point's final weight (0 = left out). The curve
/// shows the horizon and sorts out the points; the levelling itself uses the
/// points kept (a whole turn of a 360° horizon is a sine wave, which a curve of
/// the 4th order only approximates).
pub fn fit_curve(pts: &[Pt], degree: usize) -> Option<(Poly, f64, Vec<f64>)> {
    let mut w = vec![1.0; pts.len()];
    let mut fit = poly_fit(pts, &w, degree)?;
    let mut spread = 0.0;
    for _ in 0..8 {
        let r: Vec<f64> = pts.iter().map(|p| (p[1] - fit.at(p[0])).abs()).collect();
        spread = 1.4826 * crate::image::median_f64(r.iter().copied()) + 1e-6;
        // Tukey's biweight: beyond 4 spreads a point does not count.
        let k = 4.0 * spread.max(0.5);
        for (wi, ri) in w.iter_mut().zip(&r) {
            let u = ri / k;
            *wi = if u < 1.0 { (1.0 - u * u).powi(2) } else { 0.0 };
        }
        fit = poly_fit(pts, &w, degree)?;
    }
    Some((fit, spread, w))
}

/// Points of the horizon in a rendered panorama (pixel x, y, 0-based), one per
/// few columns: where the sky above meets the ground below. `covered`: the
/// pixels any photo shows (the rest is filled from the nearest covered pixel
/// of its column, so an empty edge is not taken for ground or sky). Columns
/// that are all sky or all ground give no point.
pub fn find_horizon(img: &Img, covered: Option<&Bits>) -> Vec<Pt> {
    let (h, w) = (img.h, img.w);
    if h < 16 || w < 16 {
        return vec![];
    }
    let mut work = img.clone();
    let mut usable = vec![true; w];
    if let Some(cov) = covered.filter(|c| c.w == w && c.h == h) {
        for x in 0..w {
            let ys: Vec<usize> = (0..h).filter(|&y| cov.v[y * w + x]).collect();
            let (Some(&top), Some(&bot)) = (ys.first(), ys.last()) else {
                usable[x] = false;
                continue;
            };
            if bot - top < h / 4 {
                usable[x] = false;
            }
            for y in 0..top {
                work.px[y * w + x] = img.px[top * w + x];
            }
            for y in bot + 1..h {
                work.px[y * w + x] = img.px[bot * w + x];
            }
        }
    }
    let cols = (w / 4).clamp(8, 400);
    let (_, frac) = crate::stacker::detect_sky(&work, cols, None, 0.0);
    let mut pts = Vec::new();
    for (u, &f) in frac.iter().enumerate() {
        if !(0.02..0.98).contains(&f) {
            continue; // all sky or all ground here
        }
        let x = (u as f64 + 0.5) * w as f64 / cols as f64 - 0.5;
        if !usable[(x.round() as usize).min(w - 1)] {
            continue;
        }
        pts.push([x, f * h as f64]);
    }
    pts
}

/// Elevation of a direction (radians, up positive; y points down).
fn elevation(d: &V3) -> f64 {
    (-d[1] / d.norm()).clamp(-1.0, 1.0).asin()
}

/// The view turn that levels the horizon. `dirs`: directions of horizon
/// points in the reference camera's frame. Yaw stays; pitch and roll are
/// fitted (degrees, from the current ones): all points at one height, or, for
/// a rectilinear view, at height 0. Where the points span too little of the
/// circle to tell the pitch (a short panorama), it stays near the current one.
/// Returns (pitch, roll, spread of the heights before, after; degrees).
pub fn level(dirs: &[V3], proj: Projection, yaw: f64, pitch0: f64, roll0: f64) -> Option<(f64, f64, f64, f64)> {
    if dirs.len() < 2 {
        return None;
    }
    let flat = proj == Projection::Rectilinear;
    // The points' angular extent (rad, at most 1): the largest angle from their mean direction.
    let mean = dirs.iter().fold(V3::zeros(), |a, d| a + d.normalize()).normalize();
    let extent = dirs.iter().map(|d| d.normalize().dot(&mean).clamp(-1.0, 1.0).acos()).fold(0.0, f64::max).clamp(1e-5, 1.0);
    let resid = |p: f64, r: f64| -> Vec<f64> {
        let rot = view_rotation(yaw, p, r);
        let e: Vec<f64> = dirs.iter().map(|d| elevation(&(rot * d))).collect();
        let m = if flat { 0.0 } else { e.iter().sum::<f64>() / e.len() as f64 };
        let mut v: Vec<f64> = e.iter().map(|x| x - m).collect();
        // A pull to the current angles: decides what the points cannot (the
        // pitch of a short panorama): 1° costs as much as all points 0.005° off,
        // less for points within a narrow field (a long lens), whose heights
        // differ by little.
        let k = 0.005f64.to_radians() * (dirs.len() as f64).sqrt() * extent;
        v.push(k * (p - pitch0));
        v.push(k * (r - roll0));
        v
    };
    let spread = |p: f64, r: f64| -> f64 {
        let v = resid(p, r);
        let n = dirs.len();
        (v[..n].iter().map(|x| x * x).sum::<f64>() / n as f64).sqrt().to_degrees()
    };
    let cost = |p: f64, r: f64| resid(p, r).iter().map(|x| x * x).sum::<f64>();
    let before = spread(pitch0, roll0);
    // Gauss–Newton with damping over (pitch, roll).
    let (mut p, mut r) = (pitch0, roll0);
    let mut lambda = 1e-3;
    let mut c0 = cost(p, r);
    for _ in 0..60 {
        let r0 = resid(p, r);
        let h = 1e-4;
        let jp: Vec<f64> = resid(p + h, r).iter().zip(&r0).map(|(a, b)| (a - b) / h).collect();
        let jr: Vec<f64> = resid(p, r + h).iter().zip(&r0).map(|(a, b)| (a - b) / h).collect();
        let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
        let (a11, a12, a22) = (dot(&jp, &jp), dot(&jp, &jr), dot(&jr, &jr));
        let (g1, g2) = (dot(&jp, &r0), dot(&jr, &r0));
        let mut improved = false;
        for _ in 0..12 {
            let (b11, b22) = (a11 * (1.0 + lambda) + 1e-12, a22 * (1.0 + lambda) + 1e-12);
            let det = b11 * b22 - a12 * a12;
            if det.abs() < 1e-18 {
                lambda *= 10.0;
                continue;
            }
            let dp = -(b22 * g1 - a12 * g2) / det;
            let dr = -(b11 * g2 - a12 * g1) / det;
            let (pn, rn) = ((p + dp).clamp(-90.0, 90.0), (r + dr).clamp(-180.0, 180.0));
            let cn = cost(pn, rn);
            if cn < c0 {
                let small = (c0 - cn) < 1e-14 * c0.max(1e-30) || (dp.abs() + dr.abs()) < 1e-7;
                (p, r, c0) = (pn, rn, cn);
                lambda = (lambda / 3.0).max(1e-9);
                improved = !small;
                break;
            }
            lambda *= 4.0;
        }
        if !improved {
            break;
        }
    }
    Some((p, r, before, spread(p, r)))
}

/// How much of the full circle (°) the directions span around the vertical axis
/// of a view turn (360 minus the largest gap between them).
pub fn azimuth_span(dirs: &[V3], yaw: f64, pitch: f64, roll: f64) -> f64 {
    let rot = view_rotation(yaw, pitch, roll);
    let mut az: Vec<f64> = dirs.iter().map(|d| { let q = rot * d; q[0].atan2(q[2]).to_degrees() }).collect();
    if az.len() < 2 {
        return 0.0;
    }
    az.sort_by(f64::total_cmp);
    let mut gap = az[0] + 360.0 - az[az.len() - 1];
    for w in az.windows(2) {
        gap = gap.max(w[1] - w[0]);
    }
    360.0 - gap
}

/// Directions (reference camera frame) of points (u, v) of the output with the
/// current view turn. `f`: the output's focal length (px).
pub fn directions(pts: &[Pt], proj: Projection, f: f64, yaw: f64, pitch: f64, roll: f64) -> Vec<V3> {
    let back = view_rotation(yaw, pitch, roll).transpose();
    pts.iter()
        .filter_map(|q| proj_inverse(proj, q[0] / f, q[1] / f))
        .map(|(x, y, z)| back * V3::new(x, y, z).normalize())
        .collect()
}

/// Output point (u, v) of a direction (reference camera frame) with a view turn; None where not shown.
pub fn to_output(d: &V3, proj: Projection, f: f64, yaw: f64, pitch: f64, roll: f64) -> Option<Pt> {
    let q = view_rotation(yaw, pitch, roll) * d;
    let (a, b) = proj_forward(proj, q[0], q[1], q[2]);
    (a.is_finite() && b.is_finite()).then_some([a * f, b * f])
}

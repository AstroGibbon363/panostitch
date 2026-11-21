//! Matching frames of a night sky by their star patterns.
//!
//! Corner features fail on the sky: every star looks like every other star,
//! and a sky without a horizon has little else. But turning the camera keeps
//! the angles between stars, and so does the turning of the sky between two
//! shots. So the stars of each frame become directions (rays, from the focal
//! length), triangles of neighbouring stars are compared by their side angles,
//! and RANSAC finds the rotation most star pairs agree on.

use super::{Pt, M3};
use crate::image::{Gray, Img};
use nalgebra::Vector3;
use rayon::prelude::*;

type V3 = Vector3<f64>;

/// Stars used per frame (brightest, spread over the frame).
pub const SKY_STARS: usize = 150;
/// Neighbours per star that form its triangles.
const NEIGHBOURS: usize = 6;

/// Star positions (1-based pixels) in the sky part of a photo, brightest first,
/// spread over a 6×6 grid so that every overlap has its share.
pub fn sky_stars(img: &Img) -> Vec<Pt> {
    let (h, w) = (img.h, img.w);
    if h < 64 || w < 64 {
        return vec![];
    }
    // Sky only: lights, grass and leaves make "stars" that move with the ground.
    let (mask, _) = crate::stacker::detect_sky(img, 32, None, 0.0);
    let (mh, mw) = (mask.h, mask.w);
    let mut wt = Gray::filled(h, w, 0.0);
    wt.px.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let my = (y * mh / h).min(mh - 1);
        for (x, p) in row.iter_mut().enumerate() {
            if mask.v[my * mw + (x * mw / w).min(mw - 1)] {
                *p = 1.0;
            }
        }
    });
    let stars = crate::stacker::detect_stars(img, &wt);
    let (gx, gy) = (6usize, 6usize);
    let quota = SKY_STARS / (gx * gy) + 1;
    let mut used = vec![0usize; gx * gy];
    let (mut pick, mut rest) = (Vec::new(), Vec::new());
    for s in &stars {
        let cx = ((s.x / w as f64 * gx as f64) as usize).min(gx - 1);
        let cy = ((s.y / h as f64 * gy as f64) as usize).min(gy - 1);
        if used[cy * gx + cx] < quota && pick.len() < SKY_STARS {
            used[cy * gx + cx] += 1;
            pick.push([s.x, s.y]);
        } else {
            rest.push([s.x, s.y]);
        }
    }
    let extra = SKY_STARS.saturating_sub(pick.len()).min(rest.len());
    pick.extend_from_slice(&rest[..extra]);
    pick
}

/// The stars of one frame as rays, with their triangles, for a given focal length.
pub struct SkyIndex {
    px: Vec<Pt>,
    v: Vec<V3>,
    /// (side angles, shortest first; the vertices opposite them), sorted by the longest side.
    tris: Vec<([f64; 3], [usize; 3])>,
    c: Pt,
    f: f64,
    size: (usize, usize),
}

fn ray(p: Pt, c: Pt, f: f64) -> V3 {
    V3::new(p[0] - c[0], p[1] - c[1], f).normalize()
}

impl SkyIndex {
    /// `px`: star positions of an h×w image; `f`: its focal length in pixels.
    pub fn new(px: &[Pt], h: usize, w: usize, f: f64) -> Self {
        let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
        let v: Vec<V3> = px.iter().map(|&p| ray(p, c, f)).collect();
        let ang = |a: usize, b: usize| v[a].dot(&v[b]).clamp(-1.0, 1.0).acos();
        let n = v.len();
        let mut set = std::collections::HashSet::new();
        for i in 0..n {
            let mut idx: Vec<usize> = (0..n).filter(|&k| k != i).collect();
            idx.sort_by(|&a, &b| ang(i, a).total_cmp(&ang(i, b)));
            let nb = &idx[..NEIGHBOURS.min(idx.len())];
            for a in 0..nb.len() {
                for b in a + 1..nb.len() {
                    let mut t = [i, nb[a], nb[b]];
                    t.sort_unstable();
                    set.insert(t);
                }
            }
        }
        let min_side = 6.0 / f; // too small a triangle has no shape to compare
        let mut tris: Vec<([f64; 3], [usize; 3])> = set
            .into_iter()
            .filter_map(|[i, j, k]| {
                let mut s = [(ang(j, k), i), (ang(i, k), j), (ang(i, j), k)];
                s.sort_by(|a, b| a.0.total_cmp(&b.0));
                (s[0].0 >= min_side).then_some(([s[0].0, s[1].0, s[2].0], [s[0].1, s[1].1, s[2].1]))
            })
            .collect();
        tris.sort_by(|a, b| a.0[2].total_cmp(&b.0[2]));
        Self { px: px.to_vec(), v, tris, c, f, size: (h, w) }
    }

    pub fn len(&self) -> usize {
        self.v.len()
    }

    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }

    /// Whether the ray `d` (in this camera's frame) lands inside the image, `margin` px from the border.
    fn sees(&self, d: &V3, margin: f64) -> bool {
        if d[2] <= 0.0 {
            return false;
        }
        let (x, y) = (self.c[0] + self.f * d[0] / d[2], self.c[1] + self.f * d[1] / d[2]);
        let (h, w) = (self.size.0 as f64, self.size.1 as f64);
        x >= margin && x <= w - margin && y >= margin && y <= h - margin
    }
}

/// Rotation R with R·b ≈ a (least squares, Kabsch).
fn kabsch(a: &[V3], b: &[V3]) -> M3 {
    let mut h = M3::zeros();
    for (x, y) in a.iter().zip(b) {
        h += x * y.transpose();
    }
    let svd = h.svd(true, true);
    let (u, vt) = (svd.u.unwrap(), svd.v_t.unwrap());
    let d = (u * vt).determinant().signum();
    u * M3::from_diagonal(&V3::new(1.0, 1.0, d)) * vt
}

/// Two frames matched by their stars.
#[derive(Clone, Debug)]
pub struct StarMatch {
    /// Maps pixels of the second frame into the first.
    pub h: M3,
    /// Matched stars: in the first frame, in the second.
    pub p1: Vec<Pt>,
    pub p2: Vec<Pt>,
    /// Mean distance (px) of the matched stars after the rotation.
    pub err: f64,
    /// Stars of either frame that fall into the other one (that could match).
    pub expected: usize,
}

/// At least this many stars must match.
pub const MIN_STARS: usize = 12;

/// Match frame b to frame a by star triangles. `tol` (px): how far apart
/// matched stars may be (lens distortion and the camera's model are not known yet).
pub fn match_stars(a: &SkyIndex, b: &SkyIndex, tol: f64) -> Result<StarMatch, String> {
    if a.len() < MIN_STARS || b.len() < MIN_STARS {
        return Err("too few stars".into());
    }
    let f = a.f.min(b.f);
    // Candidate star pairs: triangles of the same shape and size.
    let mut cand: Vec<(usize, usize)> = Vec::new();
    for (sb, vb) in &b.tris {
        let t = 0.006 * sb[2] + tol / f;
        let lo = a.tris.partition_point(|x| x.0[2] < sb[2] - t);
        for (sa, va) in &a.tris[lo..] {
            if sa[2] > sb[2] + t {
                break;
            }
            if (sa[0] - sb[0]).abs() < t && (sa[1] - sb[1]).abs() < t {
                cand.extend((0..3).map(|q| (va[q], vb[q])));
            }
        }
    }
    cand.sort_unstable();
    cand.dedup();
    if cand.len() < MIN_STARS {
        return Err("no matching star patterns".into());
    }
    // RANSAC over rotations: two star pairs fix one.
    let thr = tol / f;
    let cos_thr = thr.cos();
    let mut best = (0usize, M3::identity());
    let mut state = 0x9e37_79b9_7f4a_7c15u64 ^ (a.len() as u64 * 31 + b.len() as u64);
    let mut rnd = |n: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % n as u64) as usize
    };
    for _ in 0..3000 {
        let (p, q) = (cand[rnd(cand.len())], cand[rnd(cand.len())]);
        if p.0 == q.0 || p.1 == q.1 {
            continue;
        }
        let (da, db) = (a.v[p.0].dot(&a.v[q.0]).clamp(-1.0, 1.0).acos(), b.v[p.1].dot(&b.v[q.1]).clamp(-1.0, 1.0).acos());
        if (da - db).abs() > 2.0 * thr || da < 20.0 / f {
            continue;
        }
        let r = kabsch(
            &[a.v[p.0], a.v[q.0], a.v[p.0].cross(&a.v[q.0]).normalize()],
            &[b.v[p.1], b.v[q.1], b.v[p.1].cross(&b.v[q.1]).normalize()],
        );
        let n = cand.iter().filter(|&&(x, y)| a.v[x].dot(&(r * b.v[y])) > cos_thr).count();
        if n > best.0 {
            best = (n, r);
        }
    }
    if best.0 < MIN_STARS / 2 {
        return Err("star patterns do not agree".into());
    }
    // Refine with every star: nearest neighbours under the rotation.
    let mut r = best.1;
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for _ in 0..3 {
        pairs.clear();
        for (y, vb) in b.v.iter().enumerate() {
            let d = r * vb;
            let (mut bd, mut bx) = (-1.0, 0);
            for (x, va) in a.v.iter().enumerate() {
                let c = va.dot(&d);
                if c > bd {
                    bd = c;
                    bx = x;
                }
            }
            if bd > cos_thr {
                pairs.push((bx, y));
            }
        }
        // one star of a may only take one partner
        pairs.sort_unstable();
        pairs.dedup_by_key(|p| p.0);
        if pairs.len() < 3 {
            return Err("star patterns do not agree".into());
        }
        r = kabsch(&pairs.iter().map(|p| a.v[p.0]).collect::<Vec<_>>(), &pairs.iter().map(|p| b.v[p.1]).collect::<Vec<_>>());
    }
    let n = pairs.len();
    // Stars of each frame that the other one sees.
    let margin = 12.0;
    let ea = a.v.iter().filter(|v| b.sees(&(r.transpose() * *v), margin)).count();
    let eb = b.v.iter().filter(|v| a.sees(&(r * *v), margin)).count();
    let expected = ea.min(eb);
    if n < MIN_STARS {
        return Err(format!("only {n} stars match"));
    }
    // A real overlap matches a good share of the stars in it; a chance alignment
    // of a few (or specks in the grass taken for stars) does not.
    let share = if n < 25 { 0.5 } else { 0.4 };
    if (n as f64) < share * expected as f64 {
        return Err(format!("only {n} of {expected} stars in the overlap match"));
    }
    let ka = super::kmat(a.f, a.c);
    let kb = super::kmat(b.f, b.c);
    let h = ka * r * super::inv(&kb);
    let p1: Vec<Pt> = pairs.iter().map(|p| a.px[p.0]).collect();
    let p2: Vec<Pt> = pairs.iter().map(|p| b.px[p.1]).collect();
    let err = p1.iter().zip(&p2).map(|(x, y)| super::reproj_err(&h, *y, *x)).sum::<f64>() / n as f64;
    Ok(StarMatch { h, p1, p2, err, expected })
}

/// A photo's open sky and background (see `render::SkyAlign`).
pub fn sky_map(img: &Img) -> super::SkyMap {
    // Background: block means at 1/16 size (stars are sparse), smoothed.
    let bg = crate::image::blur(&crate::image::resize(img, 1.0 / 16.0), 1.0);
    super::SkyMap { open: sky_mask(img), bg }
}

/// Where a night photo shows open sky (true), at a quarter of its resolution:
/// the sky region above the horizon (`detect_sky`), without the silhouettes of
/// trees, poles and the like that rise into it (clearly darker than the sky
/// around them), and kept a margin away from all of them.
pub fn sky_mask(img: &Img) -> crate::image::Bits {
    let (region, _) = crate::stacker::detect_sky(img, 128, None, 0.0);
    let l = crate::image::blur(&crate::image::resize(&img.luminance(), 0.25), 1.0);
    let (h, w) = (l.h, l.w);
    let inr = |y: usize, x: usize| region.v[(y * region.h / h).min(region.h - 1) * region.w + (x * region.w / w).min(region.w - 1)];
    // Sky brightness per tile: a bright share of its sky pixels (trees in the tile do not pull it down);
    // tiles without enough sky take the tile above.
    let tile = 24usize;
    let (ty, tx) = (h.div_ceil(tile), w.div_ceil(tile));
    let mut bg = vec![f32::NAN; ty * tx];
    for a in 0..ty {
        for b in 0..tx {
            let mut v: Vec<f32> = Vec::new();
            for y in a * tile..((a + 1) * tile).min(h) {
                for x in b * tile..((b + 1) * tile).min(w) {
                    if inr(y, x) {
                        v.push(l.at(y, x));
                    }
                }
            }
            if v.len() >= tile * tile / 8 {
                let k = v.len() * 4 / 5;
                v.select_nth_unstable_by(k, |p, q| p.total_cmp(q));
                bg[a * tx + b] = v[k];
            }
        }
    }
    for a in 1..ty {
        for b in 0..tx {
            if bg[a * tx + b].is_nan() {
                bg[a * tx + b] = bg[(a - 1) * tx + b];
            }
        }
    }
    let fg: Vec<bool> = (0..h * w)
        .map(|k| {
            let (y, x) = (k / w, k % w);
            let b = bg[(y / tile) * tx + x / tile];
            !inr(y, x) || (b.is_finite() && l.at(y, x) < 0.7 * b)
        })
        .collect();
    // Open sky: at least `r` pixels from any foreground (silhouette edges are soft
    // and thin twigs are missed; near them the photo is better left as shot).
    let r = (w / 50).max(2);
    let grow = |m: &[bool], horizontal: bool| -> Vec<bool> {
        (0..h * w)
            .map(|k| {
                let (y, x) = (k / w, k % w);
                if horizontal {
                    (x.saturating_sub(r)..(x + r + 1).min(w)).any(|xx| m[y * w + xx])
                } else {
                    (y.saturating_sub(r)..(y + r + 1).min(h)).any(|yy| m[yy * w + x])
                }
            })
            .collect()
    };
    let near = grow(&grow(&fg, true), false);
    crate::image::Bits { w, h, v: near.iter().map(|&n| !n).collect() }
}

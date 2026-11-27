//! Filling the empty areas of a panorama ("auto-complete"): the parts of the
//! output rectangle no photo covers get made-up content that continues what is
//! around them, as Microsoft ICE does.
//!
//! At a working size of about `work` pixels the picture is split in two: a
//! background (a median over 9×9 pixels, carried on smoothly into the empty
//! parts from their edges) and the detail on it (stars, grain, branches). The
//! detail is filled by patch-based image completion (Wexler et al. 2007, with
//! PatchMatch from Barnes et al. 2009 to find similar patches): every small
//! square around an empty pixel is matched to the most similar square of the
//! photographed part, and the empty pixels take the median of what the
//! matched squares say about them; coarse to fine over an image pyramid.
//!
//! At full size every empty pixel then copies the photographed pixel its
//! match points at, so the fill has real stars, grain and grass instead of a
//! blur, with the low frequencies of the working-size fill, so nothing jumps
//! at the edge of the empty area.

use crate::image::Img;
use rayon::prelude::*;

/// Patch radius (patches of 7×7 pixels).
const R: i64 = 3;
/// How much the background counts when patches are compared: enough to keep
/// sky from ground, little against the grain.
const BACK_WEIGHT: f32 = 0.3;

/// A picture to fill, pixel by pixel (the panorama may be kept as 8-bit,
/// 16-bit or float data).
pub trait Canvas: Sync {
    /// (h, w).
    fn size(&self) -> (usize, usize);
    fn get(&self, y: usize, x: usize) -> [f32; 3];
    fn set(&mut self, y: usize, x: usize, v: [f32; 3]);
}

impl Canvas for Img {
    fn size(&self) -> (usize, usize) {
        (self.h, self.w)
    }
    fn get(&self, y: usize, x: usize) -> [f32; 3] {
        self.px[y * self.w + x]
    }
    fn set(&mut self, y: usize, x: usize, v: [f32; 3]) {
        let w = self.w;
        self.px[y * w + x] = v;
    }
}

/// One bit per pixel: which pixels a photo covers.
#[derive(Clone, Debug)]
pub struct Cover {
    pub h: usize,
    pub w: usize,
    bits: Vec<u64>,
}

impl Cover {
    pub fn new(h: usize, w: usize) -> Self {
        Self { h, w, bits: vec![0; (h * w).div_ceil(64)] }
    }
    #[inline]
    pub fn get(&self, y: usize, x: usize) -> bool {
        let k = y * self.w + x;
        self.bits[k >> 6] >> (k & 63) & 1 == 1
    }
    #[inline]
    pub fn set(&mut self, y: usize, x: usize, on: bool) {
        let k = y * self.w + x;
        if on {
            self.bits[k >> 6] |= 1 << (k & 63);
        } else {
            self.bits[k >> 6] &= !(1 << (k & 63));
        }
    }
    /// Rows `y0…` from a mask of whole rows (`v[i]` for pixel i of those rows).
    pub fn set_rows(&mut self, y0: usize, v: &[bool]) {
        for (i, &b) in v.iter().enumerate() {
            let k = y0 * self.w + i;
            self.set(k / self.w, k % self.w, b);
        }
    }
    pub fn count(&self) -> usize {
        self.bits.iter().map(|b| b.count_ones() as usize).sum()
    }
}

/// Working image of the completion: colours and which pixels are known.
struct Level {
    h: usize,
    w: usize,
    px: Vec<[f32; 3]>,
    known: Vec<bool>,
    /// The background under the detail (everywhere): patches of the sky come
    /// from the sky, not from a ground of the same grain.
    back: Vec<[f32; 3]>,
}

impl Level {
    /// What patches are compared in (the detail, about zero): stars hidden.
    /// The best match of a patch of the fill (smoother than the sky while it is
    /// being made) is always a starless patch, so the fill would get no stars;
    /// with point sources hidden (isolated pixels far above the noise), a patch
    /// with a star looks like one without. Larger shapes (branches, the edge
    /// of the ground) are kept.
    fn guide(&self) -> Vec<[f32; 3]> {
        let (h, w) = (self.h, self.w);
        let step = (self.px.len() / 20_000).max(1);
        let mut sig = [0f32; 3];
        for (c, sg) in sig.iter_mut().enumerate() {
            let mut v: Vec<f32> = self.px.iter().zip(&self.known).step_by(step).filter(|(_, k)| **k).map(|(p, _)| p[c].abs()).collect();
            if v.is_empty() {
                *sg = f32::INFINITY;
                continue;
            }
            let m = v.len() / 2;
            v.select_nth_unstable_by(m, |a, b| a.total_cmp(b));
            *sg = (1.4826 * v[m]).max(1e-7);
        }
        let strong: Vec<bool> = self.px.par_iter().map(|p| (0..3).any(|c| p[c].abs() > 4.0 * sig[c])).collect();
        (0..h * w)
            .into_par_iter()
            .map(|k| {
                let p = self.px[k];
                if strong[k] {
                    let (y, x) = (k / w, k % w);
                    let mut n = 0;
                    for yy in y.saturating_sub(2)..(y + 3).min(h) {
                        for xx in x.saturating_sub(2)..(x + 3).min(w) {
                            n += strong[yy * w + xx] as usize;
                        }
                    }
                    if n <= 9 {
                        let b = self.back[k];
                        return [BACK_WEIGHT * b[0], BACK_WEIGHT * b[1], BACK_WEIGHT * b[2]];
                    }
                }
                let cap = |c: usize| 6.0 * sig[c];
                let b = self.back[k];
                [p[0].clamp(-cap(0), cap(0)) + BACK_WEIGHT * b[0], p[1].clamp(-cap(1), cap(1)) + BACK_WEIGHT * b[1], p[2].clamp(-cap(2), cap(2)) + BACK_WEIGHT * b[2]]
            })
            .collect()
    }

    /// Half the size: known where all four pixels below are known.
    fn down(&self) -> Level {
        let (h, w) = (self.h.div_ceil(2), self.w.div_ceil(2));
        let mut px = vec![[0f32; 3]; h * w];
        let mut known = vec![false; h * w];
        let back: Vec<[f32; 3]> = (0..h * w)
            .into_par_iter()
            .map(|k| {
                let (y, x) = (k / w, k % w);
                let (mut s, mut n) = ([0f32; 3], 0f32);
                for yy in 2 * y..(2 * y + 2).min(self.h) {
                    for xx in 2 * x..(2 * x + 2).min(self.w) {
                        let b = self.back[yy * self.w + xx];
                        for c in 0..3 {
                            s[c] += b[c];
                        }
                        n += 1.0;
                    }
                }
                [s[0] / n, s[1] / n, s[2] / n]
            })
            .collect();
        px.par_iter_mut().zip(known.par_iter_mut()).enumerate().for_each(|(k, (p, kn))| {
            let (y, x) = (k / w, k % w);
            let (mut s, mut n, mut all) = ([0f32; 3], 0, true);
            for yy in 2 * y..(2 * y + 2).min(self.h) {
                for xx in 2 * x..(2 * x + 2).min(self.w) {
                    let i = yy * self.w + xx;
                    if self.known[i] {
                        for c in 0..3 {
                            s[c] += self.px[i][c];
                        }
                        n += 1;
                    } else {
                        all = false;
                    }
                }
            }
            if n > 0 {
                *p = [s[0] / n as f32, s[1] / n as f32, s[2] / n as f32];
            }
            *kn = all && n > 0;
        });
        Level { h, w, px, known, back }
    }
}

/// Fill the unknown pixels smoothly from the known ones (pull down a pyramid of
/// weighted averages, push back up): a first guess and the fallback.
fn push_pull(px: &mut [[f32; 3]], known: &[bool], h: usize, w: usize) {
    let mut levels: Vec<(usize, usize, Vec<[f32; 4]>)> = vec![(h, w, (0..h * w).map(|k| if known[k] { [px[k][0], px[k][1], px[k][2], 1.0] } else { [0.0; 4] }).collect())];
    while {
        let (lh, lw, _) = levels.last().unwrap();
        *lh > 1 || *lw > 1
    } {
        let (lh, lw, v) = levels.last().unwrap();
        let (nh, nw) = (lh.div_ceil(2), lw.div_ceil(2));
        let mut n = vec![[0f32; 4]; nh * nw];
        for y in 0..nh {
            for x in 0..nw {
                let mut s = [0f32; 4];
                for yy in 2 * y..(2 * y + 2).min(*lh) {
                    for xx in 2 * x..(2 * x + 2).min(*lw) {
                        let p = v[yy * lw + xx];
                        for c in 0..4 {
                            s[c] += p[c];
                        }
                    }
                }
                n[y * nw + x] = s;
            }
        }
        levels.push((nh, nw, n));
    }
    // Back up: where a level has little weight, take the coarser level's colour.
    for l in (0..levels.len() - 1).rev() {
        let (ch, cw, coarse) = {
            let (a, b, c) = &levels[l + 1];
            (*a, *b, c.clone())
        };
        let (lh, lw, v) = &mut levels[l];
        for y in 0..*lh {
            for x in 0..*lw {
                let p = &mut v[y * *lw + x];
                // The coarser level, interpolated (taking its nearest pixel leaves blocks).
                let (fy, fx) = (((y as f64 + 0.5) / 2.0 - 0.5).max(0.0), ((x as f64 + 0.5) / 2.0 - 0.5).max(0.0));
                let (y0, x0) = ((fy as usize).min(ch - 1), (fx as usize).min(cw - 1));
                let (y1, x1) = ((y0 + 1).min(ch - 1), (x0 + 1).min(cw - 1));
                let (ty, tx) = ((fy - y0 as f64).min(1.0) as f32, (fx - x0 as f64).min(1.0) as f32);
                let col = |q: [f32; 4]| if q[3] > 0.0 { [q[0] / q[3], q[1] / q[3], q[2] / q[3]] } else { [0.0; 3] };
                let (a, b, c, d) = (col(coarse[y0 * cw + x0]), col(coarse[y0 * cw + x1]), col(coarse[y1 * cw + x0]), col(coarse[y1 * cw + x1]));
                let mut qc = [0f32; 3];
                for k in 0..3 {
                    qc[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty;
                }
                let a = p[3].min(1.0);
                let pc = if p[3] > 0.0 { [p[0] / p[3], p[1] / p[3], p[2] / p[3]] } else { [0.0; 3] };
                *p = [a * pc[0] + (1.0 - a) * qc[0], a * pc[1] + (1.0 - a) * qc[1], a * pc[2] + (1.0 - a) * qc[2], 1.0];
            }
        }
    }
    let v = &levels[0].2;
    for k in 0..h * w {
        if !known[k] {
            px[k] = [v[k][0], v[k][1], v[k][2]];
        }
    }
}

#[inline]
fn hash(a: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

/// Patch-based completion of one level. `nnf[p]`: the source pixel (as y·w + x)
/// matched to the patch around p, for the pixels whose patch reaches an
/// unknown pixel ("targets"); updated in place.
fn complete_level(lv: &mut Level, nnf: &mut [u32], iters: usize, seed: u64) {
    let (h, w) = (lv.h, lv.w);
    let (hi, wi) = (h as i64, w as i64);
    // Sources: patches entirely known and inside.
    let src_ok: Vec<bool> = (0..h * w)
        .into_par_iter()
        .map(|k| {
            let (y, x) = ((k / w) as i64, (k % w) as i64);
            if y < R || x < R || y >= hi - R || x >= wi - R {
                return false;
            }
            (-R..=R).all(|dy| (-R..=R).all(|dx| lv.known[((y + dy) * wi + x + dx) as usize]))
        })
        .collect();
    let sources: Vec<u32> = (0..h * w).filter(|&k| src_ok[k]).map(|k| k as u32).collect();
    if sources.is_empty() {
        push_pull(&mut lv.px, &lv.known, h, w);
        return;
    }
    // Targets: pixels whose patch reaches an unknown pixel.
    let target: Vec<bool> = (0..h * w)
        .into_par_iter()
        .map(|k| {
            let (y, x) = ((k / w) as i64, (k % w) as i64);
            (-R..=R).any(|dy| (-R..=R).any(|dx| {
                let (yy, xx) = (y + dy, x + dx);
                yy >= 0 && xx >= 0 && yy < hi && xx < wi && !lv.known[(yy * wi + xx) as usize]
            }))
        })
        .collect();
    for k in 0..h * w {
        if target[k] && !src_ok[nnf[k] as usize] {
            nnf[k] = sources[(hash(seed ^ k as u64) % sources.len() as u64) as usize];
        }
    }
    let rows: Vec<usize> = (0..h).filter(|&y| (0..w).any(|x| target[y * w + x])).collect();
    for it in 0..iters {
        let g = lv.guide();
        // Patch distance between the patch at target p (current colours) and source s.
        let dist = |p: usize, s: usize, cap: f32| -> f32 {
            let (py, px_) = ((p / w) as i64, (p % w) as i64);
            let (sy, sx) = ((s / w) as i64, (s % w) as i64);
            let (mut d, mut n) = (0f32, 0f32);
            for dy in -R..=R {
                let yy = py + dy;
                if yy < 0 || yy >= hi {
                    continue;
                }
                for dx in -R..=R {
                    let xx = px_ + dx;
                    if xx < 0 || xx >= wi {
                        continue;
                    }
                    let a = g[(yy * wi + xx) as usize];
                    let b = g[((sy + dy) * wi + sx + dx) as usize];
                    d += (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
                    n += 1.0;
                }
                if d > cap * n.max(1.0) * 1.5 && n > 14.0 {
                    return f32::INFINITY;
                }
            }
            d / n.max(1.0)
        };
        // PatchMatch: rows in parallel, scanning forwards on even passes and
        // backwards on odd ones; a pixel tries its neighbours' matches, shifted,
        // and random ones around its own at shrinking distances.
        let fwd = it % 2 == 0;
        let old = nnf.to_vec();
        let new_rows: Vec<(usize, Vec<u32>)> = rows
            .par_iter()
            .map(|&y| {
                let mut row: Vec<u32> = old[y * w..(y + 1) * w].to_vec();
                let xs: Vec<usize> = if fwd { (0..w).collect() } else { (0..w).rev().collect() };
                let step: i64 = if fwd { -1 } else { 1 };
                for x in xs {
                    let p = y * w + x;
                    if !target[p] {
                        continue;
                    }
                    let mut best = row[x] as usize;
                    let mut bd = dist(p, best, f32::INFINITY);
                    let try_src = |s: i64, best: &mut usize, bd: &mut f32| {
                        if s >= 0 && (s as usize) < h * w && src_ok[s as usize] && s as usize != *best {
                            let d = dist(p, s as usize, *bd);
                            if d < *bd {
                                *bd = d;
                                *best = s as usize;
                            }
                        }
                    };
                    // Propagation: from the pixel before in this row (new) and the row before (old).
                    let xn = x as i64 + step;
                    if xn >= 0 && xn < wi && target[y * w + xn as usize] {
                        let s = row[xn as usize] as i64;
                        let (sy, sx) = (s / wi, s % wi - step);
                        if sx >= 0 && sx < wi {
                            try_src(sy * wi + sx, &mut best, &mut bd);
                        }
                    }
                    let yn = y as i64 + step;
                    if yn >= 0 && yn < hi && target[yn as usize * w + x] {
                        let s = old[yn as usize * w + x] as i64;
                        let (sy, sx) = (s / wi - step, s % wi);
                        if sy >= 0 && sy < hi {
                            try_src(sy * wi + sx, &mut best, &mut bd);
                        }
                    }
                    // Random search.
                    let mut rad = hi.max(wi);
                    let mut k = 0u64;
                    while rad >= 1 {
                        let rnd = hash(seed ^ (p as u64) << 20 ^ (it as u64) << 8 ^ k);
                        let (by, bx) = ((best / w) as i64, (best % w) as i64);
                        let dy = (rnd % (2 * rad as u64 + 1)) as i64 - rad;
                        let dx = ((rnd >> 32) % (2 * rad as u64 + 1)) as i64 - rad;
                        let (sy, sx) = ((by + dy).clamp(R, hi - R - 1), (bx + dx).clamp(R, wi - R - 1));
                        try_src(sy * wi + sx, &mut best, &mut bd);
                        rad /= 2;
                        k += 1;
                    }
                    row[x] = best as u32;
                }
                (y, row)
            })
            .collect();
        for (y, row) in new_rows {
            nnf[y * w..(y + 1) * w].copy_from_slice(&row);
        }
        // Voting: each unknown pixel takes the mean of what the matched patches over it say.
        let px = &lv.px;
        let voted: Vec<(usize, [f32; 3])> = (0..h * w)
            .into_par_iter()
            .filter(|&k| !lv.known[k])
            .map(|k| {
                // The median of the votes: a star in a few of the matched
                // patches neither blurs into the fill nor brightens it.
                let (y, x) = ((k / w) as i64, (k % w) as i64);
                let mut votes: [Vec<f32>; 3] = [Vec::with_capacity(49), Vec::with_capacity(49), Vec::with_capacity(49)];
                for dy in -R..=R {
                    for dx in -R..=R {
                        let (qy, qx) = (y - dy, x - dx);
                        if qy < 0 || qx < 0 || qy >= hi || qx >= wi {
                            continue;
                        }
                        let q = (qy * wi + qx) as usize;
                        if !target[q] {
                            continue;
                        }
                        let sq = nnf[q] as i64;
                        let v = px[((sq / wi + dy) * wi + sq % wi + dx) as usize];
                        for c in 0..3 {
                            votes[c].push(v[c]);
                        }
                    }
                }
                if votes[0].is_empty() {
                    return (k, px[k]);
                }
                let mut o = [0f32; 3];
                for c in 0..3 {
                    let m = votes[c].len() / 2;
                    votes[c].select_nth_unstable_by(m, |a, b| a.total_cmp(b));
                    o[c] = votes[c][m];
                }
                (k, o)
            })
            .collect();
        for (k, v) in voted {
            lv.px[k] = v;
        }
    }
}

/// Fill the unknown pixels of a small image (`known` false). Returns the
/// filled image and, for every pixel, its matched source pixel (itself where known).
fn complete_small(px: Vec<[f32; 3]>, known: Vec<bool>, back: Vec<[f32; 3]>, h: usize, w: usize) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut levels = vec![Level { h, w, px, known, back }];
    while {
        let l = levels.last().unwrap();
        l.h.min(l.w) > 48 && levels.len() < 9
    } {
        let d = levels.last().unwrap().down();
        levels.push(d);
    }
    let nl = levels.len();
    // Coarsest level: a smooth first guess, random matches.
    {
        let l = &mut levels[nl - 1];
        push_pull(&mut l.px, &l.known, l.h, l.w);
    }
    let mut nnf: Vec<u32> = (0..levels[nl - 1].h * levels[nl - 1].w).map(|k| k as u32).collect();
    for li in (0..nl).rev() {
        let (h_, w_) = (levels[li].h, levels[li].w);
        if li < nl - 1 {
            // The coarser matches, doubled, start this level; the unknown pixels start from a vote of them.
            let (ch, cw) = (levels[li + 1].h, levels[li + 1].w);
            let mut fine = vec![0u32; h_ * w_];
            for y in 0..h_ {
                for x in 0..w_ {
                    let (cy, cx) = ((y / 2).min(ch - 1), (x / 2).min(cw - 1));
                    let s = nnf[cy * cw + cx] as usize;
                    let (sy, sx) = ((s / cw) * 2 + y % 2, (s % cw) * 2 + x % 2);
                    fine[y * w_ + x] = (sy.min(h_ - 1) * w_ + sx.min(w_ - 1)) as u32;
                }
            }
            nnf = fine;
            // Unknown pixels: the coarser level's colours to begin with.
            let coarse = levels[li + 1].px.clone();
            let l = &mut levels[li];
            for y in 0..h_ {
                for x in 0..w_ {
                    if !l.known[y * w_ + x] {
                        l.px[y * w_ + x] = coarse[(y / 2).min(ch - 1) * cw + (x / 2).min(cw - 1)];
                    }
                }
            }
        }
        let iters = if li == 0 { 4 } else { 6 };
        complete_level(&mut levels[li], &mut nnf, iters, 0x5eed ^ li as u64);
    }
    let l = levels.swap_remove(0);
    for k in 0..h * w {
        if l.known[k] {
            nnf[k] = k as u32;
        }
    }
    (l.px, nnf)
}

/// Fill the pixels of `pic` that `cover` leaves out. `work`: longest side of
/// the working size. Returns the number of pixels filled.
pub fn complete(pic: &mut dyn Canvas, cover: &Cover, work: usize, progress: &dyn Fn(&str)) -> usize {
    let (h, w) = pic.size();
    let nhole = h * w - cover.count();
    if nhole == 0 || nhole == h * w {
        return 0;
    }
    // Working size: whole pixels of the panorama per working pixel.
    let s = (h.max(w)).div_ceil(work.max(64)).max(1);
    let (hl, wl) = (h.div_ceil(s), w.div_ceil(s));
    progress(&format!("Filling the empty areas: preparing ({wl}×{hl} px)…"));
    let src: &dyn Canvas = &*pic;
    let (small, known): (Vec<[f32; 3]>, Vec<bool>) = (0..hl * wl)
        .into_par_iter()
        .map(|k| {
            let (y, x) = (k / wl, k % wl);
            let (mut sum, mut n, mut all) = ([0f32; 3], 0usize, true);
            for yy in y * s..((y + 1) * s).min(h) {
                for xx in x * s..((x + 1) * s).min(w) {
                    if cover.get(yy, xx) {
                        let p = src.get(yy, xx);
                        for c in 0..3 {
                            sum[c] += p[c];
                        }
                        n += 1;
                    } else {
                        all = false;
                    }
                }
            }
            let m = if n > 0 { [sum[0] / n as f32, sum[1] / n as f32, sum[2] / n as f32] } else { [0.0; 3] };
            (m, all && n > 0)
        })
        .unzip();
    progress(&format!("Filling the empty areas: making up {} % of the picture…", (100 * nhole).div_ceil(h * w)));
    // Background (low frequencies): the known part blurred, carried on into the
    // empty part smoothly from its edges. Detail (stars, grain, edges):
    // the rest, filled with matched patches. Apart, the patches need not match
    // the level of the sky (at the coarse levels the sky right at the edge of
    // an empty area cannot be a patch), and the level need not come from them.
    // The background is a median over 9×9 known pixels: no stars in it, and
    // the edge of the ground stays sharp (a blur would brighten the ground
    // next to the sky, and the fill of an empty corner of ground with it).
    let rad = 4usize;
    let mut back: Vec<[f32; 3]> = (0..hl * wl)
        .into_par_iter()
        .map(|k| {
            if !known[k] {
                return [0.0; 3];
            }
            let (y, x) = (k / wl, k % wl);
            let mut v: [Vec<f32>; 3] = [Vec::with_capacity(81), Vec::with_capacity(81), Vec::with_capacity(81)];
            for yy in y.saturating_sub(rad)..(y + rad + 1).min(hl) {
                for xx in x.saturating_sub(rad)..(x + rad + 1).min(wl) {
                    let i = yy * wl + xx;
                    if known[i] {
                        for c in 0..3 {
                            v[c].push(small[i][c]);
                        }
                    }
                }
            }
            let mut o = [0f32; 3];
            for c in 0..3 {
                let m = v[c].len() / 2;
                v[c].select_nth_unstable_by(m, |a, b| a.total_cmp(b));
                o[c] = v[c][m];
            }
            o
        })
        .collect();
    push_pull(&mut back, &known, hl, wl);
    let detail: Vec<[f32; 3]> = small.iter().zip(&back).zip(&known).map(|((p, b), &k)| if k { [p[0] - b[0], p[1] - b[1], p[2] - b[2]] } else { [0.0; 3] }).collect();
    let (dfill, nnf) = complete_small(detail, known.clone(), back.clone(), hl, wl);
    let filled: Vec<[f32; 3]> = (0..hl * wl).map(|k| if known[k] { small[k] } else { [back[k][0] + dfill[k][0], back[k][1] + dfill[k][1], back[k][2] + dfill[k][2]] }).collect();
    // The fill at the working size gives the low frequencies (a little
    // smoothed), the photographed pixel each match points at gives the detail.
    let filled = crate::image::blur(&Img::from_vec(hl, wl, filled), 1.5).px;
    // Full size: copy the photographed pixel each match points at, corrected
    // by the fill's colours at the working size (its low frequencies).
    progress("Filling the empty areas: full size…");
    let bil = |v: &[[f32; 3]], fy: f64, fx: f64| -> [f32; 3] {
        let (y0, x0) = (fy.floor().clamp(0.0, (hl - 1) as f64) as usize, fx.floor().clamp(0.0, (wl - 1) as f64) as usize);
        let (y1, x1) = ((y0 + 1).min(hl - 1), (x0 + 1).min(wl - 1));
        let (ty, tx) = ((fy - y0 as f64).clamp(0.0, 1.0) as f32, (fx - x0 as f64).clamp(0.0, 1.0) as f32);
        let mut o = [0f32; 3];
        for c in 0..3 {
            let a = v[y0 * wl + x0][c] * (1.0 - tx) + v[y0 * wl + x1][c] * tx;
            let b = v[y1 * wl + x0][c] * (1.0 - tx) + v[y1 * wl + x1][c] * tx;
            o[c] = a * (1.0 - ty) + b * ty;
        }
        o
    };
    let sf = s as f64;
    let block = 256usize;
    let (filled, nnf) = (&filled, &nnf);
    for y0 in (0..h).step_by(block) {
        // Sources are photographed pixels, never filled ones: a block reads
        // from the picture as it was, then is written.
        let src: &dyn Canvas = &*pic;
        let out: Vec<(usize, usize, [f32; 3])> = (y0..(y0 + block).min(h))
            .into_par_iter()
            .flat_map_iter(|y| {
                let bil = &bil;
                (0..w).filter(move |&x| !cover.get(y, x)).map(move |x| {
                    let (fy, fx) = ((y as f64 + 0.5) / sf - 0.5, (x as f64 + 0.5) / sf - 0.5);
                    let want = bil(filled, fy, fx);
                    let (ly, lx) = (fy.floor().max(0.0) as usize, fx.floor().max(0.0) as usize);
                    let (mut acc, mut wsum) = ([0f32; 3], 0f32);
                    for (ny, nx) in [(ly, lx), (ly, lx + 1), (ly + 1, lx), (ly + 1, lx + 1)] {
                        let (ny, nx) = (ny.min(hl - 1), nx.min(wl - 1));
                        let wgt = ((1.0 - (fy - ny as f64).abs()).max(0.0) * (1.0 - (fx - nx as f64).abs()).max(0.0)) as f32 + 1e-4;
                        let sk = nnf[ny * wl + nx] as usize;
                        let (dy, dx) = ((sk / wl) as f64 - ny as f64, (sk % wl) as f64 - nx as f64);
                        let (sy, sx) = ((y as f64 + dy * sf).round(), (x as f64 + dx * sf).round());
                        if sy < 0.0 || sx < 0.0 || sy >= h as f64 || sx >= w as f64 {
                            continue;
                        }
                        let (sy, sx) = (sy as usize, sx as usize);
                        if !cover.get(sy, sx) {
                            continue;
                        }
                        let v = src.get(sy, sx);
                        // The copy's own low frequencies at the source, replaced by the fill's.
                        let low = bil(filled, (sy as f64 + 0.5) / sf - 0.5, (sx as f64 + 0.5) / sf - 0.5);
                        for c in 0..3 {
                            acc[c] += wgt * (v[c] - low[c] + want[c]);
                        }
                        wsum += wgt;
                    }
                    let v = if wsum > 0.0 { [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum] } else { want };
                    (y, x, [v[0].max(0.0), v[1].max(0.0), v[2].max(0.0)])
                })
            })
            .collect();
        for (y, x, v) in out {
            pic.set(y, x, v);
        }
    }
    nhole
}

/// Fill an image in place (`cover`: the pixels to keep).
pub fn complete_img(img: &mut Img, cover: &Cover, work: usize) -> usize {
    complete(img, cover, work, &|_| {})
}

impl Cover {
    /// From a mask of covered pixels (true = covered).
    pub fn from_bits(b: &crate::image::Bits) -> Self {
        let mut c = Cover::new(b.h, b.w);
        c.set_rows(0, &b.v);
        c
    }
}

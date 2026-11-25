//! Noise reduction by wavelets: the image is split into layers of detail at
//! 1, 2, 4, 8 … px (the "à trous" wavelet with a B3 spline), and in each layer
//! the small wiggles that noise makes are shrunk (soft thresholding) while
//! larger detail (stars, edges, structure) stays. How large "noise" is gets
//! measured from the image itself, area by area, so it works on linear and on
//! stretched images alike (a stretch makes the noise much stronger in the dark
//! parts than in the bright ones).
//!
//! Brightness noise works on the luminance only; colour noise on the two
//! colour layers (blue − luminance, red − luminance), with more, coarser
//! layers, since colour noise forms larger blotches. The other part of the
//! image is never touched.

use crate::image::{Gray, Img};
use rayon::prelude::*;

/// Noise in each wavelet layer relative to the finest one, for white noise
/// (B3 spline à trous; Starck & Murtagh).
const LAYER_NOISE: [f32; 7] = [1.0, 0.2251, 0.0965, 0.0461, 0.0228, 0.0114, 0.0057];

/// One smoothing step of the à trous transform: the B3 spline (1 4 6 4 1)/16
/// with holes of `step` px, rows then columns, mirrored at the borders.
fn smooth(a: &Gray, step: usize) -> Gray {
    const K: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];
    let (h, w) = (a.h, a.w);
    let mirror = |i: i64, n: usize| -> usize {
        let n = n as i64;
        let mut i = i;
        if n == 1 {
            return 0;
        }
        while i < 0 || i >= n {
            i = if i < 0 { -i } else { 2 * (n - 1) - i };
        }
        i as usize
    };
    let mut tmp = vec![0f32; h * w];
    tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                let xx = mirror(x as i64 + (k as i64 - 2) * step as i64, w);
                s += kv * a.px[y * w + xx];
            }
            *o = s;
        }
    });
    let mut out = vec![0f32; h * w];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                let yy = mirror(y as i64 + (k as i64 - 2) * step as i64, h);
                s += kv * tmp[yy * w + x];
            }
            *o = s;
        }
    });
    Gray::from_vec(h, w, out)
}

/// The noise of the finest layer `d`, area by area: the median absolute value
/// on tiles (robust to stars and edges), smoothed over the neighbouring tiles
/// and filled in between tile centres.
fn local_noise(d: &Gray) -> Gray {
    let (h, w) = (d.h, d.w);
    let t = 48usize.min(h.max(w)).max(8);
    let (ny, nx) = (h.div_ceil(t), w.div_ceil(t));
    let mut tiles = vec![0f32; ny * nx];
    tiles.par_chunks_mut(nx).enumerate().for_each(|(i, row)| {
        for (j, cell) in row.iter_mut().enumerate() {
            let mut v: Vec<f32> = Vec::with_capacity(t * t);
            for y in i * t..((i + 1) * t).min(h) {
                for x in j * t..((j + 1) * t).min(w) {
                    v.push(d.px[y * w + x].abs());
                }
            }
            *cell = if v.is_empty() { 0.0 } else { crate::image::median(&mut v) / 0.6745 };
        }
    });
    // Smoothed over 3×3 tiles: the noise changes slowly over the image.
    let sm: Vec<f32> = (0..ny * nx)
        .map(|k| {
            let (i, j) = ((k / nx) as i64, (k % nx) as i64);
            let mut v: Vec<f32> = Vec::new();
            for di in -1..=1 {
                for dj in -1..=1 {
                    let (ii, jj) = (i + di, j + dj);
                    if ii >= 0 && jj >= 0 && ii < ny as i64 && jj < nx as i64 {
                        v.push(tiles[ii as usize * nx + jj as usize]);
                    }
                }
            }
            crate::image::median(&mut v)
        })
        .collect();
    let g = Gray::from_vec(ny, nx, sm);
    Gray::from_fn(h, w, |y, x| crate::image::bilinear(&g, (y as f64 + 0.5) / t as f64 + 0.5, (x as f64 + 0.5) / t as f64 + 0.5))
}

/// A plane with its noise shrunk. `strength` 0 … 1 (the threshold goes up to
/// 3.5 × the noise of each layer), `layers`: how many layers of detail are
/// treated (1 px, 2 px, 4 px …); coarser structure stays as it is.
pub fn denoise_plane(a: &Gray, strength: f64, layers: usize) -> Gray {
    if strength <= 0.0 || a.h < 4 || a.w < 4 {
        return a.clone();
    }
    let k = (3.5 * strength.clamp(0.0, 1.0)) as f32;
    let layers = layers.clamp(1, LAYER_NOISE.len());
    let shrink = |d: &Gray, sigma: &Gray, r: f32| -> Vec<f32> {
        d.px.par_iter()
            .zip(sigma.px.par_iter())
            .map(|(&v, &s)| {
                let t = r * s;
                // Soft threshold: wiggles below t go, larger ones keep their size minus t.
                if v > t { v - t } else if v < -t { v + t } else { 0.0 }
            })
            .collect()
    };
    // One layer at a time (only a few planes in memory, whatever the size):
    // the finest layer also gives the noise, area by area.
    let mut c = smooth(a, 1);
    let d0 = a.zip_map(&c, |x, y| x - y);
    let sigma = local_noise(&d0);
    let mut out = Gray::from_vec(a.h, a.w, shrink(&d0, &sigma, LAYER_NOISE[0] * k));
    drop(d0);
    for (j, &ln) in LAYER_NOISE.iter().enumerate().take(layers).skip(1) {
        let next = smooth(&c, 1 << j);
        let d = c.zip_map(&next, |x, y| x - y);
        let s = shrink(&d, &sigma, ln * k);
        out.px.par_iter_mut().zip(s.par_iter()).for_each(|(o, v)| *o += v);
        c = next;
    }
    // Plus the coarse rest, untouched.
    out.px.par_iter_mut().zip(c.px.par_iter()).for_each(|(o, v)| *o += v);
    out
}

/// Brightness noise: the luminance denoised, colours kept (each pixel moves by
/// the change of its luminance).
pub fn reduce_noise(img: &Img, strength: f64) -> Img {
    if strength <= 0.0 {
        return img.clone();
    }
    let l = img.luminance();
    let d = denoise_plane(&l, strength, 4);
    Img::from_vec(img.h, img.w, img.px.par_iter().zip(l.px.par_iter()).zip(d.px.par_iter()).map(|((c, &a), &b)| { let s = b - a; [c[0] + s, c[1] + s, c[2] + s] }).collect())
}

/// Colour noise: the two colour layers (blue − luminance, red − luminance)
/// denoised more strongly and over coarser layers, the luminance kept exactly.
pub fn reduce_color_noise(img: &Img, strength: f64) -> Img {
    if strength <= 0.0 {
        return img.clone();
    }
    let s = (strength * 1.4).min(1.6);
    let l = img.luminance();
    let cb = Gray::from_vec(img.h, img.w, img.px.iter().zip(&l.px).map(|(c, l)| c[2] - l).collect());
    let cr = Gray::from_vec(img.h, img.w, img.px.iter().zip(&l.px).map(|(c, l)| c[0] - l).collect());
    let (cb, cr) = rayon::join(|| denoise_plane(&cb, s, 6), || denoise_plane(&cr, s, 6));
    // Back to RGB with the luminance unchanged (the weights of `luminance`).
    let px: Vec<[f32; 3]> = (0..img.len())
        .into_par_iter()
        .map(|i| {
            let (y, b, r) = (l.px[i], cb.px[i] + l.px[i], cr.px[i] + l.px[i]);
            let g = (y - 0.299 * r - 0.114 * b) / 0.587;
            [r, g, b]
        })
        .collect();
    Img::from_vec(img.h, img.w, px)
}

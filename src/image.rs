//! Image basics shared by the stitcher and the stacker: RGB and grey float
//! images, separable blur, anti-aliased resize, bilinear sampling, and a few
//! robust statistics (median, quantile, MAD).
//!
//! Pixel positions in the geometry code are 1-based and continuous, as in the
//! original formulas: the centre of the pixel at row `i`, column `j` (0-based
//! indices) is at `(x, y) = (j + 1, i + 1)`.

use rayon::prelude::*;
use std::ops::{Index, IndexMut};

/// A pixel value that can be added and scaled (grey or RGB).
pub trait Px: Copy + Send + Sync + Default + 'static {
    fn zero() -> Self;
    fn add(self, o: Self) -> Self;
    fn scale(self, k: f32) -> Self;
    fn lum(self) -> f32;
}

impl Px for f32 {
    #[inline]
    fn zero() -> Self {
        0.0
    }
    #[inline]
    fn add(self, o: Self) -> Self {
        self + o
    }
    #[inline]
    fn scale(self, k: f32) -> Self {
        self * k
    }
    #[inline]
    fn lum(self) -> f32 {
        self
    }
}

pub type Rgb = [f32; 3];

impl Px for Rgb {
    #[inline]
    fn zero() -> Self {
        [0.0; 3]
    }
    #[inline]
    fn add(self, o: Self) -> Self {
        [self[0] + o[0], self[1] + o[1], self[2] + o[2]]
    }
    #[inline]
    fn scale(self, k: f32) -> Self {
        [self[0] * k, self[1] * k, self[2] * k]
    }
    #[inline]
    fn lum(self) -> f32 {
        luminance(self)
    }
}

#[inline]
pub fn luminance(c: Rgb) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

#[inline]
pub fn rgb_sub(a: Rgb, b: Rgb) -> Rgb {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub fn rgb_mul(a: Rgb, b: Rgb) -> Rgb {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

#[inline]
pub fn rgb_div(a: Rgb, b: Rgb) -> Rgb {
    [a[0] / b[0], a[1] / b[1], a[2] / b[2]]
}

#[inline]
pub fn gray(v: f32) -> Rgb {
    [v, v, v]
}

/// A row-major image of pixels `T`.
#[derive(Clone, Debug, PartialEq)]
pub struct Image<T: Px> {
    pub w: usize,
    pub h: usize,
    pub px: Vec<T>,
}

pub type Img = Image<Rgb>;
pub type Gray = Image<f32>;

impl<T: Px> Image<T> {
    pub fn new(h: usize, w: usize) -> Self {
        Self { w, h, px: vec![T::zero(); w * h] }
    }
    pub fn filled(h: usize, w: usize, v: T) -> Self {
        Self { w, h, px: vec![v; w * h] }
    }
    pub fn from_vec(h: usize, w: usize, px: Vec<T>) -> Self {
        assert_eq!(px.len(), w * h, "pixel count does not match the size");
        Self { w, h, px }
    }
    pub fn from_fn(h: usize, w: usize, f: impl Fn(usize, usize) -> T + Sync) -> Self {
        let mut px = vec![T::zero(); w * h];
        px.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                *p = f(y, x);
            }
        });
        Self { w, h, px }
    }
    #[inline]
    pub fn size(&self) -> (usize, usize) {
        (self.h, self.w)
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.px.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.px.is_empty()
    }
    #[inline]
    pub fn at(&self, y: usize, x: usize) -> T {
        self.px[y * self.w + x]
    }
    #[inline]
    pub fn set(&mut self, y: usize, x: usize, v: T) {
        self.px[y * self.w + x] = v;
    }
    pub fn row(&self, y: usize) -> &[T] {
        &self.px[y * self.w..(y + 1) * self.w]
    }
    pub fn map<U: Px>(&self, f: impl Fn(T) -> U + Sync) -> Image<U> {
        let px: Vec<U> = self.px.par_iter().map(|&p| f(p)).collect();
        Image { w: self.w, h: self.h, px }
    }
    pub fn map_inplace(&mut self, f: impl Fn(T) -> T + Sync) {
        self.px.par_iter_mut().for_each(|p| *p = f(*p));
    }
    pub fn zip_map<U: Px, V: Px>(&self, o: &Image<U>, f: impl Fn(T, U) -> V + Sync) -> Image<V> {
        assert_eq!((self.h, self.w), (o.h, o.w));
        let px: Vec<V> = self.px.par_iter().zip(o.px.par_iter()).map(|(&a, &b)| f(a, b)).collect();
        Image { w: self.w, h: self.h, px }
    }
    /// Every `step`-th pixel (a cheap subsample for statistics).
    pub fn subsample(&self, step: usize) -> impl Iterator<Item = T> + '_ {
        self.px.iter().step_by(step.max(1)).copied()
    }
    /// Step that leaves about `n` samples.
    pub fn step_for(&self, n: usize) -> usize {
        (self.len() / n.max(1)).max(1)
    }
    pub fn crop(&self, y0: usize, y1: usize, x0: usize, x1: usize) -> Self {
        let (h, w) = (y1 - y0, x1 - x0);
        let mut px = Vec::with_capacity(h * w);
        for y in y0..y1 {
            px.extend_from_slice(&self.px[y * self.w + x0..y * self.w + x1]);
        }
        Self { w, h, px }
    }
    pub fn flip_rows(&self) -> Self {
        let mut px = Vec::with_capacity(self.len());
        for y in (0..self.h).rev() {
            px.extend_from_slice(self.row(y));
        }
        Self { w: self.w, h: self.h, px }
    }
    pub fn transpose(&self) -> Self {
        Self::from_fn(self.w, self.h, |y, x| self.at(x, y))
    }
    pub fn rotate180(&self) -> Self {
        let mut px = self.px.clone();
        px.reverse();
        Self { w: self.w, h: self.h, px }
    }
    pub fn flip_cols(&self) -> Self {
        Self::from_fn(self.h, self.w, |y, x| self.at(y, self.w - 1 - x))
    }
}

impl<T: Px> Index<(usize, usize)> for Image<T> {
    type Output = T;
    #[inline]
    fn index(&self, (y, x): (usize, usize)) -> &T {
        &self.px[y * self.w + x]
    }
}
impl<T: Px> IndexMut<(usize, usize)> for Image<T> {
    #[inline]
    fn index_mut(&mut self, (y, x): (usize, usize)) -> &mut T {
        &mut self.px[y * self.w + x]
    }
}

impl Img {
    pub fn luminance(&self) -> Gray {
        self.map(luminance)
    }
    pub fn channel(&self, c: usize) -> Gray {
        self.map(move |p| p[c])
    }
}

/// Elementwise sum of two images.
pub fn add<T: Px>(a: &Image<T>, b: &Image<T>) -> Image<T> {
    a.zip_map(b, |p, q| p.add(q))
}
/// Elementwise difference of two images.
pub fn sub<T: Px>(a: &Image<T>, b: &Image<T>) -> Image<T> {
    a.zip_map(b, |p, q| p.add(q.scale(-1.0)))
}

pub fn gauss_kernel(sigma: f64) -> Vec<f32> {
    let r = (3.0 * sigma).ceil() as i64;
    let mut k: Vec<f32> = (-r..=r).map(|x| (-(x * x) as f64 / (2.0 * sigma * sigma)).exp() as f32).collect();
    let s: f32 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= s);
    k
}

/// Separable Gaussian blur with clamped borders (multi-threaded).
pub fn blur<T: Px>(a: &Image<T>, sigma: f64) -> Image<T> {
    if sigma <= 0.0 || a.is_empty() {
        return a.clone();
    }
    let k = gauss_kernel(sigma);
    let r = (k.len() / 2) as i64;
    let (h, w) = (a.h as i64, a.w as i64);
    // Vertical pass.
    let mut b = Image::<T>::new(a.h, a.w);
    b.px.par_chunks_mut(a.w).enumerate().for_each(|(i, row)| {
        for (j, out) in row.iter_mut().enumerate() {
            let mut s = T::zero();
            for t in -r..=r {
                let ii = (i as i64 + t).clamp(0, h - 1) as usize;
                s = s.add(a.px[ii * a.w + j].scale(k[(t + r) as usize]));
            }
            *out = s;
        }
    });
    // Horizontal pass.
    let mut c = Image::<T>::new(a.h, a.w);
    c.px.par_chunks_mut(a.w).enumerate().for_each(|(i, row)| {
        let src = &b.px[i * a.w..(i + 1) * a.w];
        for (j, out) in row.iter_mut().enumerate() {
            let mut s = T::zero();
            for t in -r..=r {
                let jj = (j as i64 + t).clamp(0, w - 1) as usize;
                s = s.add(src[jj].scale(k[(t + r) as usize]));
            }
            *out = s;
        }
    });
    c
}

/// Average k×k blocks (fast integer downscale).
pub fn block_average<T: Px>(a: &Image<T>, k: usize) -> Image<T> {
    let (h, w) = (a.h / k, a.w / k);
    let inv = 1.0 / (k * k) as f32;
    let mut b = Image::<T>::new(h, w);
    b.px.par_chunks_mut(w.max(1)).enumerate().for_each(|(i, row)| {
        for (j, out) in row.iter_mut().enumerate() {
            let mut s = T::zero();
            for di in 0..k {
                for dj in 0..k {
                    s = s.add(a.px[(i * k + di) * a.w + j * k + dj]);
                }
            }
            *out = s.scale(inv);
        }
    });
    b
}

/// Bilinear interpolation at 1-based position (y, x); positions outside the
/// image use its edge pixels (never extrapolates); 1-pixel-wide images are allowed.
#[inline]
pub fn bilinear<T: Px>(img: &Image<T>, y: f64, x: f64) -> T {
    let (h, w) = (img.h as i64, img.w as i64);
    let y0 = (y.floor() as i64).clamp(1, (h - 1).max(1));
    let x0 = (x.floor() as i64).clamp(1, (w - 1).max(1));
    let y1 = (y0 + 1).min(h);
    let x1 = (x0 + 1).min(w);
    let dy = ((y - y0 as f64) as f32).clamp(0.0, 1.0);
    let dx = ((x - x0 as f64) as f32).clamp(0.0, 1.0);
    let p = |yy: i64, xx: i64| img.px[((yy - 1) * w + (xx - 1)) as usize];
    let top = p(y0, x0).scale(1.0 - dx).add(p(y0, x1).scale(dx));
    let bot = p(y1, x0).scale(1.0 - dx).add(p(y1, x1).scale(dx));
    top.scale(1.0 - dy).add(bot.scale(dy))
}

/// Resize by `ratio` (≤ 1 gives an anti-aliased downscale).
pub fn resize<T: Px>(a: &Image<T>, ratio: f64) -> Image<T> {
    if ratio == 1.0 || a.is_empty() {
        return a.clone();
    }
    let mut ratio = ratio;
    let mut src = std::borrow::Cow::Borrowed(a);
    let k = if ratio < 1.0 { (1.0 / ratio).floor() as usize } else { 1 };
    if k >= 2 {
        src = std::borrow::Cow::Owned(block_average(a, k));
        ratio *= k as f64;
    }
    if ratio < 1.0 {
        src = std::borrow::Cow::Owned(blur(&src, 0.5 * (1.0 / ratio - 1.0)));
    }
    let (h, w) = (src.h, src.w);
    let hh = ((h as f64 * ratio).round() as usize).max(1);
    let ww = ((w as f64 * ratio).round() as usize).max(1);
    let s: &Image<T> = &src;
    Image::from_fn(hh, ww, |i, j| bilinear(s, (i as f64 + 0.5) / ratio + 0.5, (j as f64 + 0.5) / ratio + 0.5))
}

/// Pixel centre in a ratio-`s` resized image → pixel centre in the original.
#[inline]
pub fn to_full(v: f64, s: f64) -> f64 {
    (v - 0.5) / s + 0.5
}

// --------------------------------------------------------------------------
// Statistics
// --------------------------------------------------------------------------

/// Median of a slice (sorted in place); NaN for an empty slice.
pub fn median(v: &mut [f32]) -> f32 {
    let n = v.len();
    if n == 0 {
        return f32::NAN;
    }
    v.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Median of an iterator of values.
pub fn median_of(it: impl IntoIterator<Item = f32>) -> f32 {
    let mut v: Vec<f32> = it.into_iter().collect();
    median(&mut v)
}

/// Median of f64 values.
pub fn median_f64(it: impl IntoIterator<Item = f64>) -> f64 {
    let mut v: Vec<f64> = it.into_iter().collect();
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    v.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Quantile `p` (0…1) of a sorted slice, with linear interpolation.
pub fn quantile_sorted(v: &[f32], p: f64) -> f32 {
    let n = v.len();
    if n == 0 {
        return f32::NAN;
    }
    let pos = p.clamp(0.0, 1.0) * (n - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    let f = (pos - lo as f64) as f32;
    v[lo] * (1.0 - f) + v[hi] * f
}

/// Quantiles of an unsorted slice (sorted in place).
pub fn quantiles(v: &mut [f32], ps: &[f64]) -> Vec<f32> {
    v.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ps.iter().map(|&p| quantile_sorted(v, p)).collect()
}

pub fn quantile(v: &mut [f32], p: f64) -> f32 {
    quantiles(v, &[p])[0]
}

/// (median, 1.4826 × median absolute deviation) of a slice.
pub fn median_mad(v: &[f32]) -> (f32, f32) {
    let mut s = v.to_vec();
    let med = median(&mut s);
    if s.is_empty() {
        return (f32::NAN, f32::NAN);
    }
    let mut d: Vec<f32> = s.iter().map(|x| (x - med).abs()).collect();
    (med, 1.4826 * median(&mut d))
}

pub fn mean(v: &[f32]) -> f32 {
    if v.is_empty() {
        f32::NAN
    } else {
        v.iter().sum::<f32>() / v.len() as f32
    }
}

pub fn mean_f64(v: &[f64]) -> f64 {
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Is (x, y) inside the polygon `pts`?
pub fn inside_polygon(pts: &[(f64, f64)], x: f64, y: f64) -> bool {
    let p: Vec<(f64, f64)> = pts.iter().copied().filter(|p| p.0.is_finite() && p.1.is_finite()).collect();
    let n = p.len();
    let mut c = false;
    for k in 0..n {
        let (x1, y1) = p[k];
        let (x2, y2) = p[(k + 1) % n];
        if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
            c = !c;
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_keeps_constant() {
        let a = Gray::filled(10, 12, 3.0);
        let b = blur(&a, 1.5);
        assert!(b.px.iter().all(|v| (v - 3.0).abs() < 1e-5));
    }

    #[test]
    fn resize_halves() {
        let a = Gray::from_fn(40, 60, |i, j| (i + j) as f32);
        let b = resize(&a, 0.5);
        assert_eq!((b.h, b.w), (20, 30));
        assert!((b.at(10, 10) - a.at(20, 20)).abs() < 2.0);
    }

    #[test]
    fn median_quantile() {
        let mut v = vec![3.0, 1.0, 2.0, 4.0];
        assert_eq!(median(&mut v), 2.5);
        assert_eq!(quantile_sorted(&v, 0.0), 1.0);
        assert_eq!(quantile_sorted(&v, 1.0), 4.0);
    }
}

/// A boolean image (masks).
#[derive(Clone, Debug, PartialEq)]
pub struct Bits {
    pub w: usize,
    pub h: usize,
    pub v: Vec<bool>,
}

impl Bits {
    pub fn new(h: usize, w: usize) -> Self {
        Self { w, h, v: vec![false; w * h] }
    }
    pub fn from_fn(h: usize, w: usize, f: impl Fn(usize, usize) -> bool) -> Self {
        let mut v = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                v.push(f(y, x));
            }
        }
        Self { w, h, v }
    }
    #[inline]
    pub fn at(&self, y: usize, x: usize) -> bool {
        self.v[y * self.w + x]
    }
    #[inline]
    pub fn set(&mut self, y: usize, x: usize, b: bool) {
        self.v[y * self.w + x] = b;
    }
    pub fn count(&self) -> usize {
        self.v.iter().filter(|b| **b).count()
    }
    pub fn any(&self) -> bool {
        self.v.iter().any(|b| *b)
    }
    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
    pub fn len(&self) -> usize {
        self.v.len()
    }
    pub fn to_gray(&self) -> Gray {
        Gray::from_vec(self.h, self.w, self.v.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect())
    }
}

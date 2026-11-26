//! Finishing a stacked or stitched image. Every step is optional and they
//! always run in this order:
//!
//! 1. `prepare`: turn and crop; background extraction and colour calibration (linear data)
//! 2. `balance`: white balance and neutral background, on linear light (finished
//!    images are taken out of their sRGB curve for it and put back)
//! 3. `stretch`: linear data → viewable (MTF, arcsinh, GHS and their inverses)
//! 4. `finish_tone`: light sliders, curves, saturation, then detail (noise, stars, sharpening)
//!
//! and the export size.
//!
//! Radii are given in pixels of the full image. The window works on a small
//! copy, and `process` takes its scale so the copy looks like the full result.
//! Steps that measure the image (automatic stretch, neutral background) return
//! what they measured in `Measured`; saving the full image reuses those values
//! so the saved file matches the preview.

use crate::image::{blur, luminance, quantiles, resize, Bits, Gray, Image, Img, Rgb};
use crate::stacker::{calibrate_color, extract_background, linear_to_srgb, srgb_to_linear};
use crate::stitcher::mtf;
use serde::{Deserialize, Serialize};

// --------------------------------------------------------------------------
// Settings
// --------------------------------------------------------------------------

/// How linear data becomes a viewable image.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum StretchMode {
    /// The image is already finished (JPEG, TIFF from a camera or a stitched panorama).
    Off,
    /// The sRGB gamma curve only, as a camera would.
    Srgb,
    /// Midtones transfer function, black point and midtones measured (as PixInsight's STF / Siril's autostretch).
    MtfAuto,
    /// Midtones transfer function with black, mid and white point by hand.
    MtfManual,
    /// Inverse hyperbolic sine.
    Asinh,
    /// Generalized hyperbolic stretch (GHS).
    Ghs,
}

impl StretchMode {
    pub const ALL: [StretchMode; 6] = [StretchMode::Off, StretchMode::Srgb, StretchMode::MtfAuto, StretchMode::MtfManual, StretchMode::Asinh, StretchMode::Ghs];
    pub fn name(self) -> &'static str {
        match self {
            StretchMode::Off => "Off (already finished)",
            StretchMode::Srgb => "sRGB curve",
            StretchMode::MtfAuto => "MTF, automatic",
            StretchMode::MtfManual => "MTF, manual",
            StretchMode::Asinh => "Arcsinh",
            StretchMode::Ghs => "Generalized hyperbolic (GHS)",
        }
    }
    /// Has an inverse variant.
    pub fn invertible(self) -> bool {
        matches!(self, StretchMode::Asinh | StretchMode::Ghs)
    }
}

/// How the preview is shown (never changes the saved image), as Siril's display modes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Display {
    /// As it will be saved.
    #[default]
    Linear,
    Log,
    Sqrt,
    Squared,
    Asinh,
    AutoStretch,
    HistEq,
}

impl Display {
    pub const ALL: [Display; 7] = [Display::Linear, Display::Log, Display::Sqrt, Display::Squared, Display::Asinh, Display::AutoStretch, Display::HistEq];
    pub fn name(self) -> &'static str {
        match self {
            Display::Linear => "Linear (as saved)",
            Display::Log => "Logarithm",
            Display::Sqrt => "Square root",
            Display::Squared => "Squared",
            Display::Asinh => "Asinh",
            Display::AutoStretch => "AutoStretch",
            Display::HistEq => "Histogram equalisation",
        }
    }
}

/// A curve as points (x, y) in 0…1, sorted by x, from x = 0 to x = 1.
pub type Curve = Vec<[f64; 2]>;

pub fn identity_curve() -> Curve {
    vec![[0.0, 0.0], [1.0, 1.0]]
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FinishParams {
    // Geometry
    /// Quarter turns clockwise (0…3).
    pub quarter: u8,
    pub flip_h: bool,
    /// Fine rotation in degrees (positive = clockwise), e.g. to level a horizon.
    pub angle: f64,
    /// After a fine rotation, cut away the empty corners.
    pub auto_crop: bool,
    /// Crop: fraction of the width / height taken off each side (left, top, right, bottom).
    pub crop: [f64; 4],

    // Linear data
    pub bg_extraction: bool,
    pub color_calibration: bool,
    pub stretch: StretchMode,
    /// Use the inverse curve (Arcsinh, GHS): compresses instead of stretching.
    pub inverse: bool,
    /// Stretch the brightness and keep each pixel's colour (otherwise each colour on its own).
    pub keep_colour: bool,
    /// MTF auto: background brightness after stretching.
    pub target: f64,
    /// MTF auto: each colour measured on its own (removes a colour cast).
    pub unlinked: bool,
    /// Black point (MTF manual, Arcsinh, GHS), mid and white (MTF manual).
    pub black: f64,
    pub mid: f64,
    pub white: f64,
    /// Arcsinh: stretch factor (1 = almost none … 10000 = very strong; deep stacks need thousands).
    pub asinh: f64,
    /// GHS: stretch factor ln(D + 1), local intensity b, symmetry point, shadow and highlight protection.
    pub ghs_d: f64,
    pub ghs_b: f64,
    pub ghs_sp: f64,
    pub ghs_lp: f64,
    pub ghs_hp: f64,

    // White balance (on the viewable image)
    /// Colour multipliers, e.g. from a sampled grey area.
    pub wb: [f64; 3],
    /// Make the darker half (sky background) neutral grey.
    pub neutral_bg: bool,
    pub temperature: f64, // -1 cool … 1 warm
    pub tint: f64,        // -1 green … 1 magenta

    // Tone (on 0…1 viewable values)
    pub exposure: f64,   // EV
    pub contrast: f64,   // -1…1
    pub highlights: f64, // -1…1
    pub midtones: f64,   // -1…1
    pub shadows: f64,    // -1…1
    pub whites: f64,     // -1…1
    pub blacks: f64,     // -1…1
    /// Curves: all colours, then red, green, blue.
    pub curves: [Curve; 4],

    // Colour
    pub saturation: f64, // 0…2
    pub vibrance: f64,   // -1…1

    // Detail
    pub noise: f64,       // 0…1, brightness noise
    pub color_noise: f64, // 0…1
    pub star_reduce: f64, // 0…1
    pub sharpen: f64,     // 0…2
    pub sharpen_radius: f64,

    // Export
    /// Longest side of the saved image in pixels (0 = as it is).
    pub long_edge: usize,
    pub jpeg_quality: u8,
}

impl Default for FinishParams {
    fn default() -> Self {
        Self {
            quarter: 0,
            flip_h: false,
            angle: 0.0,
            auto_crop: true,
            crop: [0.0; 4],
            bg_extraction: false,
            color_calibration: false,
            stretch: StretchMode::Off,
            inverse: false,
            keep_colour: true,
            target: 0.2,
            unlinked: false,
            black: 0.0,
            mid: 0.5,
            white: 1.0,
            asinh: 50.0,
            ghs_d: 3.0,
            ghs_b: 0.0,
            ghs_sp: 0.0,
            ghs_lp: 0.0,
            ghs_hp: 1.0,
            wb: [1.0; 3],
            neutral_bg: false,
            temperature: 0.0,
            tint: 0.0,
            exposure: 0.0,
            contrast: 0.0,
            highlights: 0.0,
            midtones: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            curves: [identity_curve(), identity_curve(), identity_curve(), identity_curve()],
            saturation: 1.0,
            vibrance: 0.0,
            noise: 0.0,
            color_noise: 0.0,
            star_reduce: 0.0,
            sharpen: 0.0,
            sharpen_radius: 1.5,
            long_edge: 0,
            jpeg_quality: 92,
        }
    }
}

impl FinishParams {
    /// Starting settings for an image: linear astro data gets the automatic stretch.
    pub fn for_input(linear: bool) -> Self {
        // Each colour measured on its own: camera stacks usually have a strong (green) cast.
        // The automatic stretch measures each colour on its own, so a camera stack's colour
        // cast does not hide the picture. The data itself is not changed: background
        // extraction and colour calibration are separate, deliberate steps.
        Self { stretch: if linear { StretchMode::MtfAuto } else { StretchMode::Off }, unlinked: linear, ..Self::default() }
    }
    fn wb_used(&self) -> bool {
        self.neutral_bg || self.wb != [1.0; 3] || self.temperature != 0.0 || self.tint != 0.0
    }
    fn tone_used(&self) -> bool {
        self.exposure != 0.0 || self.contrast != 0.0 || self.highlights != 0.0 || self.midtones != 0.0 || self.shadows != 0.0 || self.whites != 0.0 || self.blacks != 0.0
    }
    fn curves_used(&self) -> bool {
        self.curves.iter().any(|c| *c != identity_curve())
    }
    fn colour_used(&self) -> bool {
        self.saturation != 1.0 || self.vibrance != 0.0
    }
}

/// Values measured on the preview, reused for the full image.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measured {
    /// Black point and midtones per colour (MTF auto).
    pub stretch: Option<[(f64, f64); 3]>,
    /// Background colour to subtract (neutral background).
    pub bg: Option<Rgb>,
}

// --------------------------------------------------------------------------
// Lookup tables
// --------------------------------------------------------------------------

const LUT_N: usize = 4096;

/// A function on 0…1 sampled at `LUT_N` points, applied with interpolation.
pub struct Lut(Vec<f32>);

impl Lut {
    pub fn new(f: impl Fn(f64) -> f64) -> Self {
        Lut((0..LUT_N).map(|i| f(i as f64 / (LUT_N - 1) as f64) as f32).collect())
    }
    #[inline]
    pub fn at(&self, v: f32) -> f32 {
        let t = v.clamp(0.0, 1.0) * (LUT_N - 1) as f32;
        let i = (t as usize).min(LUT_N - 2);
        let f = t - i as f32;
        self.0[i] + (self.0[i + 1] - self.0[i]) * f
    }
    /// The inverse of a rising curve from 0 to 1.
    pub fn inverse(&self) -> Self {
        let y = &self.0;
        let mut out = Vec::with_capacity(LUT_N);
        let mut j = 0;
        for i in 0..LUT_N {
            let t = i as f32 / (LUT_N - 1) as f32;
            while j + 1 < LUT_N - 1 && y[j + 1] < t {
                j += 1;
            }
            let (a, b) = (y[j], y[j + 1]);
            let f = if b > a { ((t - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
            out.push((j as f32 + f) / (LUT_N - 1) as f32);
        }
        Lut(out)
    }
}

/// Monotone cubic interpolation (Fritsch–Carlson) through the curve points: no overshoot.
pub fn curve_fn(pts: &[[f64; 2]]) -> impl Fn(f64) -> f64 + '_ {
    let n = pts.len();
    let d: Vec<f64> = (0..n.saturating_sub(1)).map(|i| (pts[i + 1][1] - pts[i][1]) / (pts[i + 1][0] - pts[i][0]).max(1e-9)).collect();
    let mut m = vec![0.0; n];
    if n >= 2 {
        m[0] = d[0];
        m[n - 1] = d[n - 2];
        for i in 1..n - 1 {
            m[i] = if d[i - 1] * d[i] <= 0.0 { 0.0 } else { (d[i - 1] + d[i]) / 2.0 };
        }
        for i in 0..n - 1 {
            if d[i] == 0.0 {
                m[i] = 0.0;
                m[i + 1] = 0.0;
            } else {
                let (a, b) = (m[i] / d[i], m[i + 1] / d[i]);
                let s = a * a + b * b;
                if s > 9.0 {
                    let t = 3.0 / s.sqrt();
                    m[i] = t * a * d[i];
                    m[i + 1] = t * b * d[i];
                }
            }
        }
    }
    move |x: f64| {
        if n < 2 {
            return x;
        }
        let x = x.clamp(pts[0][0], pts[n - 1][0]);
        let i = pts.windows(2).position(|w| x <= w[1][0]).unwrap_or(n - 2);
        let (x0, x1, y0, y1) = (pts[i][0], pts[i + 1][0], pts[i][1], pts[i + 1][1]);
        let h = (x1 - x0).max(1e-9);
        let t = (x - x0) / h;
        let (t2, t3) = (t * t, t * t * t);
        let v = (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * h * m[i] + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * h * m[i + 1];
        v.clamp(0.0, 1.0)
    }
}

// --------------------------------------------------------------------------
// Geometry
// --------------------------------------------------------------------------

/// Largest centred rectangle with the image's proportions inside the image
/// turned by `deg`, as a fraction of the original size.
pub fn inscribed_scale(w: f64, h: f64, deg: f64) -> f64 {
    let t = deg.to_radians().abs();
    let (s, c) = (t.sin(), t.cos());
    (w / (w * c + h * s)).min(h / (w * s + h * c)).min(1.0)
}

/// Turn the image by `deg` about its centre (same canvas; outside = black).
pub fn rotate(img: &Img, deg: f64) -> Img {
    if deg.abs() < 1e-6 {
        return img.clone();
    }
    let (h, w) = (img.h, img.w);
    let (s, c) = deg.to_radians().sin_cos();
    let (cy, cx) = ((h as f64 - 1.0) / 2.0, (w as f64 - 1.0) / 2.0);
    Image::from_fn(h, w, |y, x| {
        let (dx, dy) = (x as f64 - cx, y as f64 - cy);
        // Inverse rotation: where this output pixel comes from.
        let sx = c * dx + s * dy + cx;
        let sy = -s * dx + c * dy + cy;
        if sx < -0.5 || sy < -0.5 || sx > w as f64 - 0.5 || sy > h as f64 - 0.5 {
            [0.0; 3]
        } else {
            crate::image::bilinear(img, sy.clamp(0.0, h as f64 - 1.0) + 1.0, sx.clamp(0.0, w as f64 - 1.0) + 1.0)
        }
    })
}

/// Quarter turns, mirror, fine rotation and crop.
pub fn geometry(img: &Img, p: &FinishParams) -> Img {
    let mut out = match p.quarter % 4 {
        1 => img.transpose().flip_cols(),
        2 => img.rotate180(),
        3 => img.transpose().flip_rows(),
        _ => img.clone(),
    };
    if p.flip_h {
        out = out.flip_cols();
    }
    if p.angle.abs() > 1e-6 {
        out = rotate(&out, p.angle);
    }
    let (x0, y0, x1, y1) = crop_box(out.w, out.h, p);
    if x0 == 0 && y0 == 0 && x1 == out.w && y1 == out.h {
        out
    } else {
        out.crop(y0, y1, x0, x1)
    }
}

/// The crop fractions (left, top, right, bottom), kept so that at least 2 % of
/// the width and height remain.
fn crop_sides(p: &FinishParams) -> [f64; 4] {
    let mut c = p.crop.map(|v| if v.is_finite() { v.clamp(0.0, 0.98) } else { 0.0 });
    c[2] = c[2].min(0.98 - c[0]);
    c[3] = c[3].min(0.98 - c[1]);
    c
}

/// The part kept by `geometry` (x0, y0, x1, y1 in pixels) of the turned image (w × h).
fn crop_box(w: usize, h: usize, p: &FinishParams) -> (usize, usize, usize, usize) {
    let mut box_ = [0.0, 0.0, 1.0, 1.0]; // x0, y0, x1, y1 as fractions
    if p.angle.abs() > 1e-6 && p.auto_crop {
        let k = inscribed_scale(w as f64, h as f64, p.angle);
        let m = (1.0 - k) / 2.0;
        box_ = [m, m, 1.0 - m, 1.0 - m];
    }
    let (bw, bh) = (box_[2] - box_[0], box_[3] - box_[1]);
    let c = crop_sides(p);
    let (wf, hf) = (w as f64, h as f64);
    let x0 = ((box_[0] + c[0] * bw) * wf).round() as usize;
    let y0 = ((box_[1] + c[1] * bh) * hf).round() as usize;
    let x1 = (((box_[2] - c[2] * bw) * wf).round() as usize).clamp(x0 + 1, w.max(x0 + 1));
    let y1 = (((box_[3] - c[3] * bh) * hf).round() as usize).clamp(y0 + 1, h.max(y0 + 1));
    (x0, y0, x1, y1)
}

/// Size (w, h) of `geometry`'s output for a source of `w × h`, exactly.
pub fn geometry_size(w: usize, h: usize, p: &FinishParams) -> (usize, usize) {
    let (tw, th) = if p.quarter % 2 == 1 { (h, w) } else { (w, h) };
    let (x0, y0, x1, y1) = crop_box(tw, th, p);
    (x1 - x0, y1 - y0)
}

/// Where the point (x, y) of `geometry`'s output comes from in the source
/// image (w × h). Positions are continuous: pixel i covers i … i + 1.
pub fn source_point(w: usize, h: usize, p: &FinishParams, x: f64, y: f64) -> (f64, f64) {
    let (tw, th) = if p.quarter % 2 == 1 { (h, w) } else { (w, h) };
    let (x0, y0, _, _) = crop_box(tw, th, p);
    // Undo the crop, the fine rotation, the mirror, then the quarter turns.
    let (mut u, mut v) = (x + x0 as f64, y + y0 as f64);
    if p.angle.abs() > 1e-6 {
        let (s, c) = p.angle.to_radians().sin_cos();
        let (cx, cy) = (tw as f64 / 2.0, th as f64 / 2.0);
        let (dx, dy) = (u - cx, v - cy);
        (u, v) = (c * dx + s * dy + cx, -s * dx + c * dy + cy);
    }
    if p.flip_h {
        u = tw as f64 - u;
    }
    let (w, h) = (w as f64, h as f64);
    match p.quarter % 4 {
        1 => (v, h - u),
        2 => (w - u, h - v),
        3 => (w - v, u),
        _ => (u, v),
    }
}

// --------------------------------------------------------------------------
// Stretch
// --------------------------------------------------------------------------

/// Black point and midtones that put the background at `target` (as PixInsight's STF).
fn auto_bm(v: &[f32], target: f64) -> (f64, f64) {
    if v.is_empty() {
        return (0.0, 0.5);
    }
    let (med, mad) = crate::image::median_mad(v);
    let (med, mad) = (med as f64, mad as f64);
    let black = (med - 2.8 * mad).clamp(0.0, 0.99);
    let x = (med - black) / (1.0 - black);
    let mid = if x > 0.0 { (mtf(target as f32, x as f32) as f64).clamp(0.0001, 0.9999) } else { 0.5 };
    (black, mid)
}

/// Black point and midtones for each colour (all alike unless `unlinked`).
pub fn measure_stretch(img: &Img, target: f64, unlinked: bool) -> [(f64, f64); 3] {
    let step = img.step_for(200_000);
    if unlinked {
        [0, 1, 2].map(|c| auto_bm(&img.subsample(step).map(|p| p[c]).filter(|v| *v > 0.0).collect::<Vec<_>>(), target))
    } else {
        let v: Vec<f32> = img.subsample(step).map(luminance).filter(|v| *v > 0.0).collect();
        [auto_bm(&v, target); 3]
    }
}

/// Generalized hyperbolic stretch (Cranfield & Payne), normalised to 0…1.
/// `ln_d` = ln(D + 1), `b` local intensity, `sp` symmetry point, `lp` / `hp`
/// protect shadows / highlights (linear below `lp` and above `hp`).
pub fn ghs(ln_d: f64, b: f64, sp: f64, lp: f64, hp: f64) -> impl Fn(f64) -> f64 {
    let d = ln_d.exp() - 1.0;
    let sp = sp.clamp(0.0, 1.0);
    let lp = lp.clamp(0.0, sp);
    let hp = hp.clamp(sp, 1.0);
    // Base transform T(x) for x ≥ 0 and its slope.
    let t = move |x: f64| -> f64 {
        if d <= 1e-9 {
            return x;
        }
        if (b + 1.0).abs() < 1e-6 {
            (1.0 + d * x).ln()
        } else if b < 0.0 {
            (1.0 - (1.0 - b * d * x).powf((b + 1.0) / b)) / (d * (b + 1.0))
        } else if b < 1e-6 {
            1.0 - (-d * x).exp()
        } else {
            1.0 - (1.0 + b * d * x).powf(-1.0 / b)
        }
    };
    let dt = move |x: f64| -> f64 {
        if d <= 1e-9 {
            return 1.0;
        }
        if (b + 1.0).abs() < 1e-6 {
            d / (1.0 + d * x)
        } else if b < 0.0 {
            (1.0 - b * d * x).powf(1.0 / b)
        } else if b < 1e-6 {
            d * (-d * x).exp()
        } else {
            d * (1.0 + b * d * x).powf(-1.0 / b - 1.0)
        }
    };
    let piece = move |x: f64| -> f64 {
        if x < lp {
            dt(sp - lp) * (x - lp) - t(sp - lp)
        } else if x < sp {
            -t(sp - x)
        } else if x < hp {
            t(x - sp)
        } else {
            dt(hp - sp) * (x - hp) + t(hp - sp)
        }
    };
    let (t0, t1) = (piece(0.0), piece(1.0));
    let span = (t1 - t0).max(1e-12);
    move |x: f64| ((piece(x.clamp(0.0, 1.0)) - t0) / span).clamp(0.0, 1.0)
}

/// The stretch curve (after the black point) for Arcsinh and GHS.
fn stretch_lut(p: &FinishParams) -> Option<Lut> {
    let lut = match p.stretch {
        StretchMode::Asinh => {
            let k = p.asinh.max(1e-3);
            let n = k.asinh();
            Lut::new(move |x| (k * x).asinh() / n)
        }
        StretchMode::Ghs => Lut::new(ghs(p.ghs_d, p.ghs_b, p.ghs_sp, p.ghs_lp, p.ghs_hp)),
        _ => return None,
    };
    Some(if p.inverse { lut.inverse() } else { lut })
}

/// Apply a curve to the brightness (keeping the colour) or to each colour.
fn apply_curve(img: &Img, lut: &Lut, black: f32, keep_colour: bool) -> Img {
    let s = 1.0 / (1.0 - black).max(1e-6);
    img.map(move |c| {
        let c = c.map(|v| ((v - black) * s).max(0.0));
        if !keep_colour {
            return c.map(|v| lut.at(v));
        }
        let l = luminance(c);
        if l <= 1e-9 {
            return [0.0; 3];
        }
        let f = lut.at(l) / l;
        let o = c.map(|v| v * f);
        // A channel would clip: scale the pixel down to keep its colour.
        let mx = o[0].max(o[1]).max(o[2]);
        if mx > 1.0 { o.map(|v| v / mx) } else { o }
    })
}

/// Linear → viewable; measured values (MTF auto) go to / come from `m`.
pub fn stretch(img: &Img, p: &FinishParams, m: &mut Measured) -> Img {
    match p.stretch {
        StretchMode::Off => img.clone(),
        StretchMode::Srgb => img.map(|c| c.map(|v| linear_to_srgb(v.clamp(0.0, 1.0)))),
        StretchMode::MtfAuto => {
            // Measuring each colour on its own evens out the colours, so it would
            // undo the white balance (temperature, tint, a sampled grey) applied
            // just before. It is measured without those multipliers instead, and
            // they are applied afterwards as gains on the normalised values.
            let mul = if p.unlinked && p.wb_used() { wb_multipliers(p) } else { [1.0; 3] };
            let bm = *m.stretch.get_or_insert_with(|| {
                if mul == [1.0; 3] {
                    measure_stretch(img, p.target, p.unlinked)
                } else {
                    measure_stretch(&img.map(move |c| [c[0] / mul[0], c[1] / mul[1], c[2] / mul[2]]), p.target, p.unlinked)
                }
            });
            img.map(move |c| {
                let mut o = [0.0; 3];
                for k in 0..3 {
                    let (b, mid) = (bm[k].0 as f32, bm[k].1 as f32);
                    let v = c[k] / mul[k];
                    o[k] = mtf(mid, ((v - b) / (1.0 - b).max(1e-6) * mul[k]).clamp(0.0, 1.0));
                }
                o
            })
        }
        StretchMode::MtfManual => {
            let (b, mid, w) = (p.black as f32, p.mid as f32, p.white.max(p.black + 1e-4) as f32);
            img.map(move |c| c.map(|v| mtf(mid, ((v - b) / (w - b)).clamp(0.0, 1.0))))
        }
        StretchMode::Asinh | StretchMode::Ghs => {
            let lut = stretch_lut(p).expect("curve");
            apply_curve(img, &lut, p.black as f32, p.keep_colour)
        }
    }
}

// --------------------------------------------------------------------------
// White balance, tone, curves, colour (on 0…1 viewable values)
// --------------------------------------------------------------------------

/// Background colour offset: the median of each colour over the darker half, minus their mean
/// (subtracting it makes the background neutral grey without changing its brightness).
fn measure_bg(img: &Img) -> Rgb {
    let px: Vec<Rgb> = img.subsample(img.step_for(200_000)).collect();
    let mut l: Vec<f32> = px.iter().map(|c| luminance(*c)).collect();
    if l.is_empty() {
        return [0.0; 3];
    }
    let cut = quantiles(&mut l, &[0.5])[0];
    let dark: Vec<Rgb> = px.into_iter().filter(|c| luminance(*c) <= cut).collect();
    if dark.is_empty() {
        return [0.0; 3];
    }
    let med = [0, 1, 2].map(|k| crate::image::median_of(dark.iter().map(|c| c[k])));
    let mean = (med[0] + med[1] + med[2]) / 3.0;
    med.map(|m| m - mean)
}

/// Multipliers that make `grey` (an area's average colour) neutral, keeping its brightness.
pub fn wb_from_sample(grey: Rgb) -> [f64; 3] {
    let l = luminance(grey) as f64;
    if l <= 1e-9 || grey.iter().any(|v| *v <= 1e-9) {
        return [1.0; 3];
    }
    grey.map(|v| (l / v as f64).clamp(0.2, 5.0))
}

/// The colour multipliers: sampled white balance × temperature × tint.
fn wb_multipliers(p: &FinishParams) -> [f32; 3] {
    let t = p.temperature as f32 * 0.25;
    let g = 1.0 - p.tint as f32 * 0.2;
    [p.wb[0] as f32 * (1.0 + t), p.wb[1] as f32 * g, p.wb[2] as f32 * (1.0 - t)]
}

/// White balance and neutral background, on linear light. A finished image
/// (`p.stretch == Off`) is taken out of its sRGB curve for this and put back.
pub fn balance(img: &Img, p: &FinishParams, m: &mut Measured) -> Img {
    if !p.wb_used() {
        return img.clone();
    }
    let encoded = p.stretch == StretchMode::Off;
    let mul = wb_multipliers(p);
    let mut lin = img.map(move |c| {
        let c = if encoded { c.map(|v| srgb_to_linear(v.clamp(0.0, 1.0))) } else { c };
        [c[0] * mul[0], c[1] * mul[1], c[2] * mul[2]]
    });
    if p.neutral_bg {
        let bg = *m.bg.get_or_insert_with(|| measure_bg(&lin));
        lin = lin.map(move |c| [c[0] - bg[0], c[1] - bg[1], c[2] - bg[2]].map(|v| v.max(0.0)));
    }
    if encoded {
        lin.map(|c| c.map(|v| linear_to_srgb(v.clamp(0.0, 1.0))))
    } else {
        lin
    }
}

/// White balance from the average colour of the rectangle between the image
/// positions `a` and `b` (1-based, as shown) in `prepared` (the output of
/// `prepare`). None if the area is outside the image or black in some colour.
pub fn sample_wb(prepared: &Img, a: (f64, f64), b: (f64, f64), encoded: bool) -> Option<[f64; 3]> {
    let x0 = (a.0.min(b.0) - 0.5).floor().max(0.0) as usize;
    let y0 = (a.1.min(b.1) - 0.5).floor().max(0.0) as usize;
    if x0 >= prepared.w || y0 >= prepared.h {
        return None;
    }
    let x1 = ((a.0.max(b.0) - 0.5).ceil().max(0.0) as usize).clamp(x0 + 1, prepared.w);
    let y1 = ((a.1.max(b.1) - 0.5).ceil().max(0.0) as usize).clamp(y0 + 1, prepared.h);
    let mut s = [0f64; 3];
    for y in y0..y1 {
        for c in &prepared.row(y)[x0..x1] {
            for k in 0..3 {
                let v = c[k].clamp(0.0, 1.0);
                s[k] += if encoded { srgb_to_linear(v) } else { v } as f64;
            }
        }
    }
    let n = ((x1 - x0) * (y1 - y0)) as f64;
    let mean = s.map(|v| (v / n) as f32);
    mean.iter().all(|v| *v > 1e-7).then(|| wb_from_sample(mean))
}

/// Black point and (for GHS) symmetry point from the sky background of
/// `balanced` linear data. The black point goes just below the background of
/// the darkest colour (measured on the brightness it would cut away most of
/// that colour and leave a cast); the symmetry point on the background brightness.
pub fn background_points(balanced: &Img) -> (f64, f64) {
    let step = balanced.step_for(200_000);
    let stats = |f: &dyn Fn(Rgb) -> f32| {
        let v: Vec<f32> = balanced.subsample(step).map(f).filter(|v| *v > 0.0).collect();
        if v.is_empty() {
            None
        } else {
            let (m, d) = crate::image::median_mad(&v);
            Some((m as f64, d as f64))
        }
    };
    let Some((lmed, _)) = stats(&|c| luminance(c)) else { return (0.0, 0.0) };
    let black = (0..3).filter_map(|k| stats(&|c: Rgb| c[k])).map(|(m, d)| m - 2.8 * d).fold(f64::INFINITY, f64::min);
    let black = if black.is_finite() { black.clamp(0.0, 0.99) } else { 0.0 };
    (black, ((lmed - black) / (1.0 - black)).clamp(0.0, 1.0))
}

fn smoothstep(v: f64) -> f64 {
    v * v * (3.0 - 2.0 * v)
}

/// The tone curve: exposure, whites and blacks (end points), contrast, then
/// highlights, midtones and shadows (bumps peaking at 3/4, 1/2 and 1/4).
pub fn tone_fn(p: &FinishParams) -> impl Fn(f64) -> f64 {
    let ev = 2f64.powf(p.exposure);
    let bp = -p.blacks * 0.1;
    let wp = 1.0 - p.whites * 0.15;
    let (con, hi, mid, sh) = (p.contrast, p.highlights, p.midtones, p.shadows);
    move |v: f64| {
        let mut v = ((v * ev - bp) / (wp - bp)).clamp(0.0, 1.0);
        if con != 0.0 {
            v += con * (smoothstep(v) - v);
        }
        v += sh * 0.95 * v * (1.0 - v).powi(3);
        v += mid * 0.4 * v * (1.0 - v);
        v += hi * 0.95 * v.powi(3) * (1.0 - v);
        v.clamp(0.0, 1.0)
    }
}

fn tone_and_curves(img: &Img, p: &FinishParams) -> Img {
    let tone = p.tone_used().then(|| tone_fn(p));
    let master = curve_fn(&p.curves[0]);
    let lut = Lut::new(|v| {
        let v = tone.as_ref().map_or(v, |f| f(v));
        master(v)
    });
    let ch: Vec<Option<Lut>> = (1..4).map(|k| (p.curves[k] != identity_curve()).then(|| Lut::new(curve_fn(&p.curves[k])))).collect();
    img.map(move |c| {
        let mut o = c.map(|v| lut.at(v));
        for k in 0..3 {
            if let Some(l) = &ch[k] {
                o[k] = l.at(o[k]);
            }
        }
        o
    })
}

/// Histograms (brightness, red, green, blue; 128 bins, log scale, 0…1) of what
/// goes into the curves: the stretched image after the light sliders.
pub fn curve_input_histograms(stretched: &Img, p: &FinishParams) -> [Vec<f32>; 4] {
    let tone = p.tone_used().then(|| tone_fn(p));
    let mut h = [vec![0f32; 128], vec![0f32; 128], vec![0f32; 128], vec![0f32; 128]];
    let bin = |v: f32| ((v.clamp(0.0, 1.0) * 127.0) as usize).min(127);
    for c in stretched.subsample(stretched.step_for(300_000)) {
        let c = c.map(|v| tone.as_ref().map_or(v.clamp(0.0, 1.0), |f| f(v as f64) as f32));
        h[0][bin(luminance(c))] += 1.0;
        for k in 0..3 {
            h[k + 1][bin(c[k])] += 1.0;
        }
    }
    h.map(|v| {
        let mx = v.iter().map(|x| (x + 1.0).ln()).fold(1.0, f32::max);
        v.iter().map(|x| (x + 1.0).ln() / mx).collect()
    })
}

fn colour(img: &Img, p: &FinishParams) -> Img {
    let (sat, vib) = (p.saturation as f32, p.vibrance as f32);
    img.map(move |c| {
        let l = luminance(c);
        let mx = c[0].max(c[1]).max(c[2]);
        let mn = c[0].min(c[1]).min(c[2]);
        let s_now = if mx > 1e-6 { (mx - mn) / mx } else { 0.0 };
        // Vibrance boosts the dull colours more than the strong ones.
        let k = sat * (1.0 + vib * (1.0 - s_now));
        c.map(|v| (l + (v - l) * k).clamp(0.0, 1.0))
    })
}

// --------------------------------------------------------------------------
// Detail
// --------------------------------------------------------------------------

/// Minimum over a (2r+1)² square, each colour on its own (separable).
fn min_filter(img: &Img, r: usize) -> Img {
    let (h, w) = (img.h, img.w);
    let rows = Image::from_fn(h, w, |y, x| {
        let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
        let row = img.row(y);
        let mut m = row[a];
        for c in &row[a + 1..=b] {
            m = [m[0].min(c[0]), m[1].min(c[1]), m[2].min(c[2])];
        }
        m
    });
    Image::from_fn(h, w, |y, x| {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        let mut m = rows.at(a, x);
        for yy in a + 1..=b {
            let c = rows.at(yy, x);
            m = [m[0].min(c[0]), m[1].min(c[1]), m[2].min(c[2])];
        }
        m
    })
}

fn detail(img: Img, p: &FinishParams, scale: f64) -> Img {
    let mut img = img;
    // Noise: wavelets, with the noise measured area by area (see `denoise`).
    if p.noise > 0.0 {
        img = crate::denoise::reduce_noise(&img, p.noise);
    }
    if p.color_noise > 0.0 {
        img = crate::denoise::reduce_color_noise(&img, p.color_noise).map(|c| c.map(|v| v.clamp(0.0, 1.0)));
    }
    if p.star_reduce > 0.0 {
        // Stars: small bright spots above their surroundings. Pull them towards the local minimum.
        let l: Gray = img.luminance();
        let small = blur(&l, (0.7 * scale).max(0.3));
        let around = blur(&l, (6.0 * scale).max(1.5));
        let r = ((1.5 * scale).round() as usize).max(1);
        let lo = min_filter(&img, r);
        let a = p.star_reduce as f32;
        let mask = small.zip_map(&around, move |s, b| ((s - b - 0.02) * 12.0).clamp(0.0, 1.0) * a);
        img = Image::from_fn(img.h, img.w, |y, x| {
            let (c, m, n) = (img.at(y, x), mask.at(y, x), lo.at(y, x));
            [0, 1, 2].map(|k| c[k] + (n[k] - c[k]) * m)
        });
    }
    if p.sharpen > 0.0 {
        let b = blur(&img, (p.sharpen_radius * scale).max(0.3));
        let a = p.sharpen as f32;
        img = img.zip_map(&b, move |c, bl| [0, 1, 2].map(|k| (c[k] + a * (c[k] - bl[k])).clamp(0.0, 1.0)));
    }
    img
}

// --------------------------------------------------------------------------
// The whole chain
// --------------------------------------------------------------------------

/// Turned and cropped; for linear data also background extraction and colour calibration.
pub fn prepare(src: &Img, p: &FinishParams) -> Img {
    let mut img = geometry(src, p);
    if p.stretch != StretchMode::Off && (p.bg_extraction || p.color_calibration) {
        let wt = Image::filled(img.h, img.w, 1.0f32);
        if p.bg_extraction {
            img = extract_background(&img, &wt, 2);
        }
        if p.color_calibration {
            img = calibrate_color(&img, &wt);
        }
    }
    img
}

/// The settings with only turning, cropping and the stretch kept: the starting
/// point that "Compare" shows.
pub fn starting_point(p: &FinishParams) -> FinishParams {
    let d = FinishParams::default();
    FinishParams {
        wb: d.wb,
        neutral_bg: d.neutral_bg,
        temperature: d.temperature,
        tint: d.tint,
        exposure: d.exposure,
        contrast: d.contrast,
        highlights: d.highlights,
        midtones: d.midtones,
        shadows: d.shadows,
        whites: d.whites,
        blacks: d.blacks,
        curves: d.curves,
        saturation: d.saturation,
        vibrance: d.vibrance,
        noise: d.noise,
        color_noise: d.color_noise,
        star_reduce: d.star_reduce,
        sharpen: d.sharpen,
        ..p.clone()
    }
}

/// The steps after the stretch: light and curves, colour, detail.
pub fn finish_tone(img: Img, p: &FinishParams, scale: f64) -> Img {
    let mut img = img;
    if p.tone_used() || p.curves_used() {
        img = tone_and_curves(&img, p);
    } else {
        img = img.map(|c| c.map(|v| v.clamp(0.0, 1.0)));
    }
    if p.colour_used() {
        img = colour(&img, p);
    }
    detail(img, p, scale)
}

/// The finished image. `scale` is the size of `src` relative to the full image
/// (1 for the full image, below 1 for a preview copy). Values measured here are
/// written to `m`; values already in `m` are used as they are.
pub fn process(src: &Img, p: &FinishParams, scale: f64, m: &mut Measured) -> Img {
    let prepared = prepare(src, p);
    let balanced = balance(&prepared, p, m);
    let s = stretch(&balanced, p, m);
    finish_tone(s, p, scale)
}

/// The image at its export size.
pub fn export_size(img: &Img, p: &FinishParams) -> Img {
    let long = img.w.max(img.h);
    if p.long_edge == 0 || p.long_edge >= long {
        return img.clone();
    }
    resize(img, p.long_edge as f64 / long as f64)
}

/// Size (w, h) of the image after turning and cropping, before the export size.
pub fn cropped_size(w: usize, h: usize, p: &FinishParams) -> (usize, usize) {
    let (w, h) = if p.quarter % 2 == 1 { (h, w) } else { (w, h) };
    let k = if p.angle.abs() > 1e-6 && p.auto_crop { inscribed_scale(w as f64, h as f64, p.angle) } else { 1.0 };
    let c = crop_sides(p);
    (((w as f64 * k * (1.0 - c[0] - c[2])).round() as usize).max(1), ((h as f64 * k * (1.0 - c[1] - c[3])).round() as usize).max(1))
}

/// Size (w, h) of the saved image for an input of `w × h`.
pub fn output_size(w: usize, h: usize, p: &FinishParams) -> (usize, usize) {
    let (w, h) = cropped_size(w, h, p);
    let long = w.max(h);
    if p.long_edge > 0 && p.long_edge < long {
        let k = p.long_edge as f64 / long as f64;
        (((w as f64 * k).round() as usize).max(1), ((h as f64 * k).round() as usize).max(1))
    } else {
        (w, h)
    }
}

/// A display mode with Low / High cut-offs (0…1) as in Siril: values from Low
/// to High fill the view, before the mode's curve. Only the view changes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ViewStretch {
    pub mode: Display,
    pub low: f64,
    pub high: f64,
}

impl Default for ViewStretch {
    fn default() -> Self {
        ViewStretch { mode: Display::Linear, low: 0.0, high: 1.0 }
    }
}

impl ViewStretch {
    /// Shown as it is (linear, full range).
    pub fn plain(self) -> bool {
        self.mode == Display::Linear && self.low == 0.0 && self.high == 1.0
    }
    /// Whether Low / High apply (AutoStretch finds its own range).
    pub fn has_range(self) -> bool {
        self.mode != Display::AutoStretch
    }
    pub fn key(self) -> u64 {
        (self.mode as u64) ^ self.low.to_bits().rotate_left(17) ^ self.high.to_bits().rotate_left(41)
    }
}

/// How the preview is shown: a view of the (already finished) image `img`.
pub fn display(img: &Img, mode: Display) -> Img {
    display_view(img, ViewStretch { mode, ..ViewStretch::default() }, img)
}

/// `img` shown with display mode and cut-offs `v`. AutoStretch and histogram
/// equalisation are measured on `reference` (the whole preview, so a detail of
/// it is shown the same way).
pub fn display_view(img: &Img, v: ViewStretch, reference: &Img) -> Img {
    let ranged = v.has_range() && !(v.low == 0.0 && v.high == 1.0);
    let (lo, hi) = (v.low.clamp(0.0, 1.0) as f32, v.high.clamp(0.0, 1.0) as f32);
    let (lo, span) = (lo.min(hi - 1e-6), (hi - lo.min(hi - 1e-6)).max(1e-6));
    // The cut-offs are applied exactly, before the curve's table, so a narrow
    // range of faint linear data is not posterised.
    let win = move |x: f32| if ranged { ((x - lo) / span).clamp(0.0, 1.0) } else { x };
    let f: Box<dyn Fn(f64) -> f64> = match v.mode {
        Display::Linear => {
            if !ranged {
                return img.clone();
            }
            return img.map(move |c| c.map(win));
        }
        Display::Log => Box::new(|v| (1.0 + 1000.0 * v).ln() / 1001f64.ln()),
        Display::Sqrt => Box::new(|v: f64| v.sqrt()),
        Display::Squared => Box::new(|v| v * v),
        Display::Asinh => Box::new(|v| (100.0 * v).asinh() / 100f64.asinh()),
        Display::AutoStretch => {
            let bm = measure_stretch(reference, 0.25, false)[0];
            Box::new(move |v| mtf(bm.1 as f32, (((v - bm.0) / (1.0 - bm.0)).clamp(0.0, 1.0)) as f32) as f64)
        }
        Display::HistEq => {
            // Brightness histogram turned into its cumulative distribution.
            let mut hist = vec![0f64; LUT_N];
            for c in reference.subsample(reference.step_for(500_000)) {
                hist[((win(luminance(c)).clamp(0.0, 1.0)) * (LUT_N - 1) as f32) as usize] += 1.0;
            }
            let total: f64 = hist.iter().sum::<f64>().max(1.0);
            let mut acc = 0.0;
            let cdf: Vec<f64> = hist.iter().map(|h| {
                acc += h;
                acc / total
            }).collect();
            Box::new(move |v| cdf[((v.clamp(0.0, 1.0)) * (LUT_N - 1) as f64) as usize])
        }
    };
    let lut = Lut::new(f);
    img.map(move |c| c.map(|x| lut.at(win(x))))
}

/// A short description of the settings in use (for the status line), its
/// words passed through `t` (the window's translation).
pub fn summary(p: &FinishParams, t: fn(&'static str) -> &'static str) -> String {
    let d = FinishParams::default();
    let mut v: Vec<String> = Vec::new();
    if !p.quarter.is_multiple_of(4) || p.flip_h || p.angle != 0.0 {
        v.push(t("turned").into());
    }
    if p.crop != d.crop {
        v.push(t("cropped").into());
    }
    if p.stretch != StretchMode::Off {
        let inv = if p.inverse && p.stretch.invertible() { format!("{} ", t("inverse")) } else { String::new() };
        v.push(format!("{inv}{}", t(p.stretch.name()).to_lowercase()));
    }
    for (used, w) in [
        (p.wb_used(), "white balance"),
        (p.tone_used(), "tone"),
        (p.curves_used(), "curves"),
        (p.colour_used(), "colour"),
        (p.noise > 0.0 || p.color_noise > 0.0, "noise reduced"),
        (p.star_reduce > 0.0, "stars reduced"),
        (p.sharpen > 0.0, "sharpened"),
    ] {
        if used {
            v.push(t(w).into());
        }
    }
    if v.is_empty() { t("no changes").into() } else { v.join(", ") }
}

// --------------------------------------------------------------------------
// Sky and foreground: their own looks, through a painted mask
// --------------------------------------------------------------------------

/// Width of a sky mask grid; its height follows the image's shape.
pub const MASK_W: usize = 640;

/// Looks of their own for the sky and for the foreground, on top of the main
/// settings. The mask (true = sky) lies on the source image, before turning and
/// cropping, so it stays in place when those change.
#[derive(Clone, PartialEq, Debug)]
pub struct Local {
    pub mask: Bits,
    /// Width of the soft edge between sky and foreground, as a fraction of the image width.
    pub feather: f64,
    pub sky: FinishParams,
    pub ground: FinishParams,
}

impl Default for Local {
    fn default() -> Self {
        Self { mask: Bits::new(0, 0), feather: 0.02, sky: FinishParams::default(), ground: FinishParams::default() }
    }
}

impl Local {
    /// Rows of the mask for an image of `w × h`.
    pub fn mask_rows(w: usize, h: usize) -> usize {
        ((MASK_W as f64 * h as f64 / w.max(1) as f64).round() as usize).max(1)
    }
    /// A mask of the right shape for an image of `w × h` (nothing is sky).
    pub fn empty_mask(w: usize, h: usize) -> Bits {
        Bits::new(Self::mask_rows(w, h), MASK_W)
    }
    /// The mask has the shape for an image of `w × h`.
    pub fn fits(&self, w: usize, h: usize) -> bool {
        (self.mask.h, self.mask.w) == (Self::mask_rows(w, h), MASK_W)
    }
    /// Either look changes something (and there is a mask to apply it through).
    pub fn used(&self) -> bool {
        !self.mask.is_empty() && (look_used(&self.sky) || look_used(&self.ground))
    }
}

/// Only the look of `p`: white balance, light, curves, colour and detail (no
/// turning, cropping or stretch), for an image that already looks right.
pub fn look_part(p: &FinishParams) -> FinishParams {
    let d = FinishParams::default();
    FinishParams {
        wb: p.wb,
        temperature: p.temperature,
        tint: p.tint,
        exposure: p.exposure,
        contrast: p.contrast,
        highlights: p.highlights,
        midtones: p.midtones,
        shadows: p.shadows,
        whites: p.whites,
        blacks: p.blacks,
        curves: p.curves.clone(),
        saturation: p.saturation,
        vibrance: p.vibrance,
        noise: p.noise,
        color_noise: p.color_noise,
        star_reduce: p.star_reduce,
        sharpen: p.sharpen,
        sharpen_radius: p.sharpen_radius,
        ..d
    }
}

/// `look_part(p)` changes something.
pub fn look_used(p: &FinishParams) -> bool {
    look_part(p) != FinishParams::default()
}

/// The mask with its soft edge (1 = sky), at the mask's resolution.
pub fn soft_mask(mask: &Bits, feather: f64) -> Gray {
    blur(&mask.to_gray(), feather * mask.w as f64 / 4.0)
}

/// Sky weight (1 = sky) for each pixel of an `out_w × out_h` image that shows
/// `geometry(src, p)` (resized to that size), for a source of `src_w × src_h`.
pub fn sky_weight(soft: &Gray, src_w: usize, src_h: usize, p: &FinishParams, out_w: usize, out_h: usize) -> Gray {
    let (gw, gh) = geometry_size(src_w, src_h, p);
    let (kx, ky) = (gw as f64 / out_w.max(1) as f64, gh as f64 / out_h.max(1) as f64);
    let (mx, my) = (soft.w as f64 / src_w.max(1) as f64, soft.h as f64 / src_h.max(1) as f64);
    let p = p.clone();
    Gray::from_fn(out_h, out_w, move |y, x| {
        let (sx, sy) = source_point(src_w, src_h, &p, (x as f64 + 0.5) * kx, (y as f64 + 0.5) * ky);
        crate::image::bilinear(soft, sy * my + 0.5, sx * mx + 0.5)
    })
}

/// The sky and foreground looks of `local` on `img` (a finished image, e.g. the
/// output of `process`), blended by the sky weight `wt` (from `sky_weight`).
pub fn apply_local(img: &Img, local: &Local, wt: &Gray, scale: f64) -> Img {
    let run = |q: &FinishParams| look_used(q).then(|| process(img, &look_part(q), scale, &mut Measured::default()));
    let (sky, ground) = rayon::join(|| run(&local.sky), || run(&local.ground));
    let s = sky.as_ref().unwrap_or(img);
    let g = ground.as_ref().unwrap_or(img);
    Image::from_vec(
        img.h,
        img.w,
        s.px.iter().zip(&g.px).zip(&wt.px).map(|((a, b), &t)| {
            let t = t.clamp(0.0, 1.0);
            [0, 1, 2].map(|k| a[k] * t + b[k] * (1.0 - t))
        }).collect(),
    )
}

/// `img` (the output of `geometry(src, p)`, `scale` × full size) with the looks of `local` applied.
pub fn with_local(img: Img, src_w: usize, src_h: usize, p: &FinishParams, local: &Local, scale: f64) -> Img {
    if !local.used() {
        return img;
    }
    let wt = sky_weight(&soft_mask(&local.mask, local.feather), src_w, src_h, p, img.w, img.h);
    apply_local(&img, local, &wt, scale)
}

/// The image circle of a circular fisheye (lit disc, dark corners): centre
/// (x, y) and radius, all as fractions of the image width. None when the corners
/// are not dark (not a circular fisheye).
pub fn fisheye_circle(img: &Img) -> Option<(f64, f64, f64)> {
    let long = img.w.max(img.h);
    let small = if long > 400 { resize(img, 400.0 / long as f64) } else { img.clone() };
    let l = blur(&small.luminance(), 1.5);
    let (w, h) = (l.w, l.h);
    if w < 8 || h < 8 {
        return None;
    }
    let patch = |x0: usize, y0: usize, pw: usize, ph: usize| -> f32 {
        crate::image::median_of((y0..y0 + ph).flat_map(|y| (x0..x0 + pw).map(move |x| (y, x))).map(|(y, x)| l.at(y, x)))
    };
    let (cw, ch) = ((w / 20).max(2), (h / 20).max(2));
    let corners = [patch(0, 0, cw, ch), patch(w - cw, 0, cw, ch), patch(0, h - ch, cw, ch), patch(w - cw, h - ch, cw, ch)];
    let corner = corners.iter().cloned().fold(f32::MIN, f32::max);
    let centre = patch(w * 2 / 5, h * 2 / 5, w / 5, h / 5);
    if centre <= corner * 2.0 + 1e-4 {
        return None;
    }
    let t = corner + 0.3 * (centre - corner);
    // The lit area's extent: columns and rows with at least a few lit pixels.
    let (mut cols, mut rows) = (vec![0usize; w], vec![0usize; h]);
    for (y, r) in rows.iter_mut().enumerate() {
        for (x, c) in cols.iter_mut().enumerate() {
            if l.at(y, x) > t {
                *c += 1;
                *r += 1;
            }
        }
    }
    let span = |v: &[usize], need: usize| -> Option<(usize, usize)> {
        let a = v.iter().position(|&c| c >= need)?;
        let b = v.len() - 1 - v.iter().rev().position(|&c| c >= need)?;
        Some((a, b + 1))
    };
    let (x0, x1) = span(&cols, (h / 50).max(2))?;
    let (y0, y1) = span(&rows, (w / 50).max(2))?;
    let (bw, bh) = ((x1 - x0) as f64, (y1 - y0) as f64);
    // A circle cut by the top and bottom edges: the width gives the radius, the frame's middle the height.
    let cy = if bh > 0.92 * bw { (y0 + y1) as f64 / 2.0 } else { h as f64 / 2.0 };
    let (cx, r) = ((x0 + x1) as f64 / 2.0, bw.max(bh) / 2.0);
    if r < 0.15 * w as f64 {
        return None;
    }
    Some((cx / w as f64, cy / w as f64, r / w as f64))
}

/// A mask (`rows × cols`, for a `w × h` image) that is sky inside the circle
/// at (cx, cy) with radius r (fractions of the image width).
pub fn circle_mask(rows: usize, cols: usize, w: usize, h: usize, cx: f64, cy: f64, r: f64) -> Bits {
    let (w, h) = (w as f64, h as f64);
    Bits::from_fn(rows, cols, |y, x| {
        let px = (x as f64 + 0.5) / cols as f64 * w;
        let py = (y as f64 + 0.5) / rows as f64 * h;
        (px - cx * w).powi(2) + (py - cy * w).powi(2) <= (r * w).powi(2)
    })
}

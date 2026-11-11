//! Star stacking core (the window's "Star stacking" tab and `panostack`), after Sequator:
//!
//!   calibration masters (bias, darks, flats, flat darks) → every star image:
//!   load linear, calibrate, fix hot pixels, detect stars in the sky region
//!   (drawn, painted or detected automatically) → align to the base image
//!   (triangle matching + RANSAC, optionally with lens distortion and a smooth
//!   correction grid) → score and reject frames → combine (average, sigma
//!   clipping, freeze ground, star trails with fading and gap filling;
//!   optionally every N frames for a time-lapse) → optionally the ground from a
//!   separate foreground image → post-processing (`post`) → save (`run`).
//!
//! Every run returns a frame report (what was measured and decided per frame)
//! and a log of the steps with their timings, so nothing happens unseen.
//!
//! The frames are kept as 16-bit floats, in RAM or (if they do not fit) in a
//! temporary file next to the output, which is deleted when the run is done.

pub mod post;
pub mod run;

use crate::image::{bilinear, blur, luminance, median, median_f64, median_mad, median_of, Bits, Gray, Img, Rgb};
use crate::io::{RawDecoder, WhiteBalance};
use crate::stitcher::{self as st, apply_h, fit_model, inv, reproj_err, Model, Pt, M3};
use half::f16;
use rayon::prelude::*;
use std::collections::HashMap;

pub use post::*;
pub use run::*;

pub const PREVIEW_PX: usize = 1600; // longest side in the quick preview
pub const PREVIEW_FRAMES: usize = 10; // star images the quick preview uses
pub const PREVIEW_CALIB: usize = 5; // calibration frames per kind in the quick preview
pub const MAX_STARS: usize = 500; // stars kept per frame (brightest)
pub const ALIGN_STARS: usize = 50; // brightest stars used for triangle matching
pub const DETECT_SIGMA: f32 = 5.0; // star detection threshold (× background noise)
pub const GAP_SPACING: f64 = 3.0; // px between the shifted copies that fill a gap in a trail
pub const GAP_MAX_STEPS: usize = 8;

// --------------------------------------------------------------------------
// Settings
// --------------------------------------------------------------------------

macro_rules! named_enum {
    ($name:ident { $($var:ident => $s:expr),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($var),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub fn name(self) -> &'static str { match self { $($name::$var => $s),+ } }
            pub fn from_name(s: &str) -> Option<Self> { Self::ALL.iter().copied().find(|v| v.name() == s) }
        }
    };
}

named_enum!(Composition { AlignStars => "Align stars", StarTrails => "Star trails" });
named_enum!(AlignMode { Accumulation => "Accumulation", Best => "Select best pixels", Freeze => "Freeze ground", AlignOnly => "Align only" });
named_enum!(Interpolation { Lanczos => "Lanczos", Bilinear => "Bilinear" });
named_enum!(SkyMode { None => "None", All => "All sky", BoundaryLine => "Boundary line", Gradient => "Gradient", IrregularMask => "Irregular mask", Automatic => "Automatic" });
named_enum!(Distortion { Off => "Off", Auto => "Auto", Lens => "Lens", Complex => "Complex" });
named_enum!(Pollution { Off => "Off", DeepSky => "Deep sky", Uneven => "Uneven" });
named_enum!(ColorSpace { Srgb => "sRGB", AdobeRgb => "Adobe RGB" });
named_enum!(Tone { SrgbCurve => "sRGB curve", Stretch => "Stretch", Linear => "Linear" });
named_enum!(Fade { Off => "Off", In => "Fade in", Out => "Fade out", Both => "Both ends" });

#[derive(Clone, Debug)]
pub struct RawOpts {
    pub wb: WhiteBalance,
    pub quality: u32,
    pub half: bool,
    pub decoder: RawDecoder,
}
#[derive(Clone, Debug)]
pub struct Sigma {
    pub low: f64,
    pub high: f64,
    pub iters: usize,
}
#[derive(Clone, Debug)]
pub struct Reject {
    pub auto: bool,
    pub min_light: f64,
    pub min_stars: f64,
    pub max_fwhm: f64,
    pub max_elong: f64,
}
#[derive(Clone, Debug)]
pub struct Sky {
    pub mode: SkyMode,
    /// Horizon (u1, v1, u2, v2) as fractions of the width / height.
    pub line: Option<(f64, f64, f64, f64)>,
    pub gradient: f64,
    pub mask: Bits,
    pub feather: f64,
}
#[derive(Clone, Debug)]
pub struct StretchCfg {
    pub auto: bool,
    pub black: f64,
    pub mid: f64,
    pub white: f64,
}
#[derive(Clone, Debug)]
pub struct LightPollution {
    pub filter: Pollution,
    pub strength: f64,
    pub iaa: bool,
    pub degree: usize,
    pub tiles: usize,
}
#[derive(Clone, Debug)]
pub struct Foreground {
    pub path: String,
    pub match_brightness: bool,
    pub ev: f64,
}

/// Every setting of a run (see `Default` for the defaults).
#[derive(Clone, Debug)]
pub struct StackConfig {
    pub stars: Vec<String>,
    pub base: Option<String>,
    pub darks: Vec<String>,
    pub bias: Vec<String>,
    pub flats: Vec<String>,
    pub flatdarks: Vec<String>,
    pub output: String,
    pub excluded: Vec<String>,
    pub raw: RawOpts,
    pub unify_exposure: bool,
    pub homogenize_vignetting: bool,
    pub composition: Composition,
    pub align_mode: AlignMode,
    pub selective: bool,
    pub motion_effect: bool,
    pub model: Model,
    pub interpolation: Interpolation,
    pub sigma: Sigma,
    pub reject: Reject,
    pub sky: Sky,
    pub auto_brightness: bool,
    pub hdr: bool,
    pub dynamic_noise: bool,
    pub merge4: bool,
    pub color_space: ColorSpace,
    pub tone: Tone,
    pub stretch: StretchCfg,
    pub bg_extraction: (bool, usize),
    pub color_calibration: bool,
    pub color_noise: (bool, f64),
    pub distortion: Distortion,
    pub light_pollution: LightPollution,
    pub enhance_stars: (bool, f64),
    pub timelapse: (bool, usize),
    pub trails: (Fade, bool),
    pub foreground: Foreground,
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            stars: vec![],
            base: None,
            darks: vec![],
            bias: vec![],
            flats: vec![],
            flatdarks: vec![],
            output: String::new(),
            excluded: vec![],
            raw: RawOpts { wb: WhiteBalance::Camera, quality: 3, half: false, decoder: RawDecoder::Auto },
            unify_exposure: false,
            homogenize_vignetting: false,
            composition: Composition::AlignStars,
            align_mode: AlignMode::Best,
            selective: false,
            motion_effect: false,
            model: Model::Homography,
            interpolation: Interpolation::Lanczos,
            sigma: Sigma { low: 3.0, high: 3.0, iters: 3 },
            reject: Reject { auto: true, min_light: 0.6, min_stars: 0.5, max_fwhm: 1.5, max_elong: 1.3 },
            sky: Sky { mode: SkyMode::None, line: None, gradient: 0.1, mask: Bits::new(0, 0), feather: 0.01 },
            auto_brightness: true,
            hdr: false,
            dynamic_noise: false,
            merge4: false,
            color_space: ColorSpace::Srgb,
            tone: Tone::SrgbCurve,
            stretch: StretchCfg { auto: true, black: 0.0, mid: 0.25, white: 1.0 },
            bg_extraction: (false, 2),
            color_calibration: false,
            color_noise: (false, 50.0),
            distortion: Distortion::Off,
            light_pollution: LightPollution { filter: Pollution::Off, strength: 50.0, iaa: false, degree: 2, tiles: 20 },
            enhance_stars: (false, 50.0),
            timelapse: (false, 10),
            trails: (Fade::Off, false),
            foreground: Foreground { path: String::new(), match_brightness: true, ev: 0.0 },
        }
    }
}

// --------------------------------------------------------------------------
// Types
// --------------------------------------------------------------------------

/// Base pixel → frame pixel: a homography H, optionally with radial lens
/// distortion k1 (the same for all frames; r relative to the half diagonal R,
/// around c) and a smooth correction grid (frame pixels at nodes `cell` apart).
#[derive(Clone, Debug)]
pub struct Warp {
    pub h: M3,
    pub hi: M3,
    pub k1: f64,
    pub c: Pt,
    pub r: f64,
    pub grid: Vec<[f64; 2]>, // ny × nx, row-major; empty = no correction
    pub ny: usize,
    pub nx: usize,
    pub cell: f64,
}

impl Warp {
    pub fn new(h: M3) -> Self {
        Self::with_lens(h, 0.0, [0.0, 0.0], 1.0)
    }
    pub fn with_lens(h: M3, k1: f64, c: Pt, r: f64) -> Self {
        Self { hi: inv(&h), h, k1, c, r, grid: vec![], ny: 0, nx: 0, cell: 1.0 }
    }
    pub fn identity() -> Self {
        Self::new(M3::identity())
    }
    #[inline]
    fn distort(&self, x: f64, y: f64) -> (f64, f64) {
        if self.k1 == 0.0 {
            return (x, y);
        }
        let (dx, dy) = ((x - self.c[0]) / self.r, (y - self.c[1]) / self.r);
        let f = 1.0 + self.k1 * (dx * dx + dy * dy);
        (self.c[0] + dx * f * self.r, self.c[1] + dy * f * self.r)
    }
    #[inline]
    fn undistort(&self, x: f64, y: f64) -> (f64, f64) {
        if self.k1 == 0.0 {
            return (x, y);
        }
        let (mut ux, mut uy) = (x, y);
        for _ in 0..5 {
            let (dx, dy) = ((ux - self.c[0]) / self.r, (uy - self.c[1]) / self.r);
            let f = 1.0 + self.k1 * (dx * dx + dy * dy);
            ux = self.c[0] + (x - self.c[0]) / f;
            uy = self.c[1] + (y - self.c[1]) / f;
        }
        (ux, uy)
    }
    #[inline]
    fn correction(&self, x: f64, y: f64) -> (f64, f64) {
        if self.grid.is_empty() {
            return (0.0, 0.0);
        }
        let (nx, ny) = (self.nx, self.ny);
        let gx = ((x - 1.0) / self.cell + 1.0).clamp(1.0, nx as f64);
        let gy = ((y - 1.0) / self.cell + 1.0).clamp(1.0, ny as f64);
        let x0 = (gx.floor() as usize).min(nx - 1);
        let y0 = (gy.floor() as usize).min(ny - 1);
        let (fx, fy) = (gx - x0 as f64, gy - y0 as f64);
        let g = |i: usize, j: usize| self.grid[(i - 1) * nx + (j - 1)];
        let (a, b, c, d) = (g(y0, x0), g(y0, x0 + 1), g(y0 + 1, x0), g(y0 + 1, x0 + 1));
        let f = |k: usize| (1.0 - fy) * ((1.0 - fx) * a[k] + fx * b[k]) + fy * ((1.0 - fx) * c[k] + fx * d[k]);
        (f(0), f(1))
    }
    /// Where base pixel (x, y) lies in the frame.
    #[inline]
    pub fn mapto(&self, x: f64, y: f64) -> (f64, f64) {
        let (ux, uy) = self.undistort(x, y);
        let (hx, hy) = apply_h(&self.h, ux, uy);
        let (px, py) = self.distort(hx, hy);
        let (cx, cy) = self.correction(x, y);
        (px + cx, py + cy)
    }
    /// The inverse: base position of frame pixel (x, y).
    #[inline]
    pub fn unmap(&self, x: f64, y: f64) -> (f64, f64) {
        let (ux, uy) = self.undistort(x, y);
        let (hx, hy) = apply_h(&self.hi, ux, uy);
        let (mut qx, mut qy) = self.distort(hx, hy);
        let hi = &self.hi;
        let (a, b, c, d) = (hi[(0, 0)] / hi[(2, 2)], hi[(0, 1)] / hi[(2, 2)], hi[(1, 0)] / hi[(2, 2)], hi[(1, 1)] / hi[(2, 2)]);
        for _ in 0..3 {
            let (px, py) = self.mapto(qx, qy);
            let (ex, ey) = (x - px, y - py);
            qx += a * ex + b * ey;
            qy += c * ex + d * ey;
        }
        (qx, qy)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Star {
    pub x: f64,
    pub y: f64,
    pub flux: f64,
    pub fwhm: f64,
    pub elong: f64,
    pub peak: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Base,
    Used,
    Rejected,
    Failed,
    Excluded,
    Skipped,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Base => "base",
            Status::Used => "used",
            Status::Rejected => "rejected",
            Status::Failed => "failed",
            Status::Excluded => "excluded",
            Status::Skipped => "skipped",
        }
    }
}

/// What happened to one star image (shown in the frame report).
#[derive(Clone, Debug)]
pub struct FrameInfo {
    pub path: String,
    pub stars: usize,
    pub fwhm: f64,       // median star size (px)
    pub elong: f64,      // median elongation (1 = round)
    pub background: f64, // median sky brightness (linear)
    pub light: f64,      // brightness of the brightest stars, relative to the base image
    pub matched: usize,  // stars matched to the base image
    pub err: f64,        // alignment error (RMS px)
    pub status: Status,
    pub reason: String,
    pub t: M3,       // base pixel → frame pixel (homography)
    pub warp: Warp,  // … with the distortion correction
    pub level: Rgb,  // median sky level per channel
    pub gain: Rgb,   // exposure unification
    pub detected: Vec<Star>,
}

impl FrameInfo {
    pub fn new(path: &str, status: Status) -> Self {
        Self {
            path: path.to_string(),
            stars: 0,
            fwhm: f64::NAN,
            elong: f64::NAN,
            background: f64::NAN,
            light: f64::NAN,
            matched: 0,
            err: f64::NAN,
            status,
            reason: String::new(),
            t: M3::identity(),
            warp: Warp::identity(),
            level: [0.0; 3],
            gain: [1.0; 3],
            detected: vec![],
        }
    }
    pub fn used(&self) -> bool {
        matches!(self.status, Status::Base | Status::Used)
    }
}

// --------------------------------------------------------------------------
// Loading
// --------------------------------------------------------------------------

#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
#[inline]
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Linear RGB data of a file: RAW decoded without tone curve, JPEG/PNG converted
/// from sRGB, TIFF, FITS and XISF taken as they are (they should be linear already).
pub fn load_linear(path: &str, cfg: &StackConfig, preview: bool) -> Result<Img, String> {
    let ext = crate::io::ext_of(path);
    let opt = crate::io::LoadOptions {
        decoder: cfg.raw.decoder,
        half: cfg.raw.half || preview,
        linear: true,
        wb: cfg.raw.wb,
        quality: Some(cfg.raw.quality),
        orient: false,
    };
    let mut img = crate::io::load_any(path, &opt)?;
    if ["jpg", "jpeg", "png", "bmp"].contains(&ext.as_str()) {
        img.map_inplace(|c| [srgb_to_linear(c[0]), srgb_to_linear(c[1]), srgb_to_linear(c[2])]);
    }
    if preview {
        let f = PREVIEW_PX as f64 / img.w.max(img.h) as f64;
        if f < 1.0 {
            img = crate::image::resize(&img, f);
        }
    }
    Ok(img)
}

// --------------------------------------------------------------------------
// Frame storage: 16-bit floats, (frames, rows, columns, 3), in RAM or memory-mapped
// --------------------------------------------------------------------------

// The fields own the memory that `FrameStore::ptr` points into.
#[allow(dead_code)]
enum Storage {
    Ram(Vec<u16>),
    Map(memmap2::MmapMut, Option<std::path::PathBuf>),
}

pub struct FrameStore {
    pub w: usize,
    pub h: usize,
    pub k: usize,
    data: Storage,
    // The pixel memory (inside `data`, which owns it; the heap / mapping does not move).
    ptr: *mut u16,
    len: usize,
    place: String,
}

unsafe impl Sync for FrameStore {}
unsafe impl Send for FrameStore {}

impl FrameStore {
    /// A store for `k` frames of `h`×`w`; in RAM if it fits comfortably, else in a temporary file in `dir`.
    /// A store for `k` frames of `h`×`w` (16-bit floats, 6 bytes per pixel).
    /// In RAM if they take less than half of the free memory; otherwise in one
    /// temporary file in `dir` (the output folder), which is deleted at once on
    /// Linux / macOS (the space comes back when the store is dropped) and when
    /// the store is dropped on Windows. The disk space is checked first, and
    /// reserved on Linux, so a full disk gives a message instead of a crash.
    pub fn new(h: usize, w: usize, k: usize, dir: &std::path::Path) -> Result<Self, String> {
        let n = 3 * w * h * k;
        let bytes = 2 * n;
        let avail = available_memory();
        if (bytes as f64) < 0.5 * avail {
            let mut v = vec![0u16; n];
            let ptr = v.as_mut_ptr();
            return Ok(Self { w, h, k, data: Storage::Ram(v), ptr, len: n, place: format!("in memory ({})", gb(bytes as f64)) });
        }
        let dir = if dir.is_dir() { dir.to_path_buf() } else { std::env::temp_dir() };
        let free = available_disk(&dir);
        // Leave room for the output files and the rest of the system.
        let margin = 2e9_f64.max(0.02 * bytes as f64);
        if let Some(free) = free {
            if (bytes as f64) + margin > free as f64 {
                return Err(format!(
                    "Not enough memory or disk space for {k} frames of {w}×{h}: they need {}, but only {} of memory and {} of disk space in {} are free. \
                     Switch on Half size (needs a quarter), use fewer frames, or save the output on a disk with more free space.",
                    gb(bytes as f64),
                    gb(avail),
                    gb(free as f64),
                    dir.display()
                ));
            }
        }
        let path = dir.join(format!("panostitch-frames-{}-{:08x}.tmp", std::process::id(), rand::random::<u32>()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| format!("Cannot create the frame file {}: {e}", path.display()))?;
        let fail = |what: String| {
            let _ = std::fs::remove_file(&path);
            what
        };
        file.set_len(bytes.max(2) as u64).map_err(|e| fail(format!("Cannot size the frame file: {e}")))?;
        reserve(&file, bytes as u64).map_err(|e| fail(format!("Cannot reserve {} of disk space in {}: {e}", gb(bytes as f64), dir.display())))?;
        let mut map = unsafe { memmap2::MmapMut::map_mut(&file) }.map_err(|e| fail(format!("Cannot map the frame file: {e}")))?;
        let ptr = map.as_mut_ptr() as *mut u16;
        drop(file);
        // The mapping stays valid without the file on Unix; Windows deletes it when the store is dropped.
        let keep = if cfg!(windows) {
            Some(path)
        } else {
            let _ = std::fs::remove_file(&path);
            None
        };
        let place = format!("in a temporary file in {} ({}, deleted when done)", dir.display(), gb(bytes as f64));
        Ok(Self { w, h, k, data: Storage::Map(map, keep), ptr, len: n, place })
    }

    /// Where the frames are kept (for the log).
    pub fn place(&self) -> &str {
        &self.place
    }
    #[inline]
    fn slice(&self) -> &[u16] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
    /// Value of channel `c` of pixel (x, y) (0-based) of frame `k`.
    #[inline]
    pub fn get(&self, k: usize, x: usize, y: usize, c: usize) -> f32 {
        f16::from_bits(self.slice()[((k * self.h + y) * self.w + x) * 3 + c]).to_f32()
    }
    #[inline]
    pub fn pixel(&self, k: usize, x: usize, y: usize) -> Rgb {
        let i = ((k * self.h + y) * self.w + x) * 3;
        let s = self.slice();
        [f16::from_bits(s[i]).to_f32(), f16::from_bits(s[i + 1]).to_f32(), f16::from_bits(s[i + 2]).to_f32()]
    }
    /// Store `img` as frame `k`.
    pub fn store(&mut self, k: usize, img: &Img) {
        self.store_shared(k, img);
    }

    /// Store `img` as frame `k` from any thread, without a lock: each frame is a
    /// separate part of the memory. Callers write every frame from one thread
    /// only and read frames only after all writing is done (as `stack` does).
    /// Single-threaded on purpose: the callers already work in parallel.
    pub fn store_shared(&self, k: usize, img: &Img) {
        let (w, h) = (self.w, self.h);
        assert!(k < self.k && img.w == w && img.h == h, "frame {k} does not fit the store");
        let off = k * h * w * 3;
        // SAFETY: the range of frame k lies inside the buffer and no other thread writes it.
        let dst = unsafe { std::slice::from_raw_parts_mut(self.ptr.add(off), h * w * 3) };
        for (q, p) in dst.chunks_exact_mut(3).zip(img.px.iter()) {
            q[0] = f16::from_f32(p[0]).to_bits();
            q[1] = f16::from_f32(p[1]).to_bits();
            q[2] = f16::from_f32(p[2]).to_bits();
        }
    }
    /// Bilinear sample of frame k at 1-based pixel position (x, y); None outside the frame.
    #[inline]
    pub fn sample(&self, k: usize, x: f64, y: f64) -> Option<Rgb> {
        let (w, h) = (self.w, self.h);
        if !(0.5 <= x && x <= w as f64 + 0.5 && 0.5 <= y && y <= h as f64 + 0.5) {
            return None;
        }
        let x0 = (x.floor() as i64).clamp(1, (w as i64 - 1).max(1)) as usize;
        let y0 = (y.floor() as i64).clamp(1, (h as i64 - 1).max(1)) as usize;
        let x1 = (x0 + 1).min(w);
        let y1 = (y0 + 1).min(h);
        let fx = ((x - x0 as f64) as f32).clamp(0.0, 1.0);
        let fy = ((y - y0 as f64) as f32).clamp(0.0, 1.0);
        let (p00, p10, p01, p11) = (self.pixel(k, x0 - 1, y0 - 1), self.pixel(k, x1 - 1, y0 - 1), self.pixel(k, x0 - 1, y1 - 1), self.pixel(k, x1 - 1, y1 - 1));
        let mut out = [0f32; 3];
        for c in 0..3 {
            let a = (1.0 - fx) * p00[c] + fx * p10[c];
            let b = (1.0 - fx) * p01[c] + fx * p11[c];
            out[c] = (1.0 - fy) * a + fy * b;
        }
        Some(out)
    }
    /// Lanczos-3 sample (sharper than bilinear, keeps the noise grain fine). The
    /// result is clamped to the four nearest pixels, so bright stars get no dark rings.
    #[inline]
    pub fn sample_lanczos(&self, k: usize, x: f64, y: f64) -> Option<Rgb> {
        let (w, h) = (self.w as i64, self.h as i64);
        if !(0.5 <= x && x <= w as f64 + 0.5 && 0.5 <= y && y <= h as f64 + 0.5) {
            return None;
        }
        let (x0, y0) = (x.floor() as i64, y.floor() as i64);
        if !(3 <= x0 && x0 <= w - 3 && 3 <= y0 && y0 <= h - 3) {
            return self.sample(k, x, y);
        }
        let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
        let mut wx = [0f32; 6];
        let mut wy = [0f32; 6];
        for i in 0..6 {
            wx[i] = lanczos(fx - (i as f32 - 2.0));
            wy[i] = lanczos(fy - (i as f32 - 2.0));
        }
        let (sx, sy) = (wx.iter().sum::<f32>(), wy.iter().sum::<f32>());
        let s = self.slice();
        let base = k as usize * self.h * self.w;
        let mut out = [0f32; 3];
        for c in 0..3 {
            let mut acc = 0f32;
            for j in 0..6 {
                let yy = (y0 + j as i64 - 2 - 1) as usize;
                let mut row = 0f32;
                let roff = (base + yy * self.w) * 3 + c;
                for i in 0..6 {
                    let xx = (x0 + i as i64 - 2 - 1) as usize;
                    row += wx[i] * f16::from_bits(s[roff + 3 * xx]).to_f32();
                }
                acc += wy[j] * row;
            }
            acc /= sx * sy;
            let g = |xx: i64, yy: i64| f16::from_bits(s[(base + (yy - 1) as usize * self.w + (xx - 1) as usize) * 3 + c]).to_f32();
            let (a, b, d, e) = (g(x0, y0), g(x0 + 1, y0), g(x0, y0 + 1), g(x0 + 1, y0 + 1));
            out[c] = acc.clamp(a.min(b).min(d).min(e), a.max(b).max(d).max(e));
        }
        Some(out)
    }
    /// Combine frames `ks` pixel by pixel with `f(values)` per channel.
    pub fn reduce(&self, ks: &[usize], f: impl Fn(&mut [f32]) -> f32 + Sync) -> Img {
        let (w, h) = (self.w, self.h);
        let mut out = Img::new(h, w);
        out.px.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let mut v = vec![0f32; ks.len()];
            for (x, p) in row.iter_mut().enumerate() {
                for c in 0..3 {
                    for (t, &k) in ks.iter().enumerate() {
                        v[t] = self.get(k, x, y, c);
                    }
                    p[c] = f(&mut v);
                }
            }
        });
        out
    }
}

impl Drop for FrameStore {
    fn drop(&mut self) {
        if let Storage::Map(_, Some(path)) = &self.data {
            let path = path.clone();
            // Unmap first (replace the mapping), then delete.
            let dummy = memmap2::MmapMut::map_anon(2).ok();
            if let Some(d) = dummy {
                let old = std::mem::replace(&mut self.data, Storage::Map(d, None));
                drop(old);
            }
            let _ = std::fs::remove_file(path);
        }
    }
}

fn gb(bytes: f64) -> String {
    format!("{:.1} GB", bytes / 1e9)
}

/// Free space (bytes) of the disk holding `dir`, if known.
pub fn available_disk(dir: &std::path::Path) -> Option<u64> {
    let dir = std::fs::canonicalize(dir).ok()?;
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks.list().iter().filter(|d| dir.starts_with(d.mount_point())).max_by_key(|d| d.mount_point().as_os_str().len()).map(|d| d.available_space())
}

/// Allocate the file's blocks now (Linux), so writing into the mapping later
/// cannot fail on a full disk (which would end the program with SIGBUS).
#[cfg(target_os = "linux")]
fn reserve(file: &std::fs::File, len: u64) -> Result<(), String> {
    use std::os::fd::AsRawFd;
    let r = unsafe { libc::posix_fallocate(file.as_raw_fd(), 0, len as libc::off_t) };
    // EOPNOTSUPP / EINVAL: the file system cannot reserve; the free-space check has to do.
    if r == 0 || r == libc::EOPNOTSUPP || r == libc::EINVAL {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(r).to_string())
    }
}
#[cfg(not(target_os = "linux"))]
fn reserve(_file: &std::fs::File, _len: u64) -> Result<(), String> {
    Ok(())
}

/// Memory available for frames (bytes).
pub fn available_memory() -> f64 {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};
    let s = System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()));
    // macOS counts the file cache as used, so its "available" memory is misleadingly small.
    if cfg!(target_os = "macos") {
        0.4 * s.total_memory() as f64
    } else {
        s.available_memory() as f64
    }
}

// Lanczos-3 kernel, tabulated.
fn lanczos_table() -> &'static [f32; 1024] {
    use std::sync::OnceLock;
    static T: OnceLock<[f32; 1024]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0f32; 1024];
        for (i, v) in t.iter_mut().enumerate() {
            let x = 3.0 * i as f64 / 1023.0;
            *v = if x == 0.0 {
                1.0
            } else {
                (3.0 * (std::f64::consts::PI * x).sin() * (std::f64::consts::PI * x / 3.0).sin() / (std::f64::consts::PI.powi(2) * x * x)) as f32
            };
        }
        t
    })
}
#[inline]
fn lanczos(t: f32) -> f32 {
    let a = t.abs();
    if a >= 3.0 {
        0.0
    } else {
        lanczos_table()[(a * 341.0).round() as usize]
    }
}

// --------------------------------------------------------------------------
// Calibration
// --------------------------------------------------------------------------

pub type Loader<'a> = &'a (dyn Fn(&str) -> Result<Img, String> + Sync);
pub type Progress<'a> = &'a (dyn Fn(&str) -> Result<(), String> + Sync);

/// Median of several frames (None if there are none).
pub fn master(paths: &[String], load: Loader, dir: &std::path::Path, progress: Progress, what: &str) -> Result<Option<Img>, String> {
    if paths.is_empty() {
        return Ok(None);
    }
    let first = load(&paths[0])?;
    let (h, w) = (first.h, first.w);
    let mut d = FrameStore::new(h, w, paths.len(), dir)?;
    for (k, p) in paths.iter().enumerate() {
        progress(&format!("Master {what}: {}/{} {}", k + 1, paths.len(), crate::io::basename(p)))?;
        let img = if k == 0 { first.clone() } else { load(p)? };
        if (img.h, img.w) != (h, w) {
            return Err(format!("{} is {}×{}, not {}×{}.", crate::io::basename(p), img.w, img.h, w, h));
        }
        d.store(k, &img);
    }
    let ks: Vec<usize> = (0..paths.len()).collect();
    Ok(Some(d.reduce(&ks, |v| median(v))))
}

pub struct Calibration {
    pub dark: Option<Img>, // subtracted from star images (dark, or bias without darks)
    pub flat: Option<Img>, // star images are divided by it (mean 1)
    pub hot: Vec<(usize, usize)>, // hot pixels (y, x) found in the master dark
    /// What was noticed or done while making the masters (for the log).
    pub notes: Vec<String>,
}

pub fn calibration(cfg: &StackConfig, load: Loader, dir: &std::path::Path, progress: Progress) -> Result<Calibration, String> {
    let mut notes = Vec::new();
    let bias = master(&cfg.bias, load, dir, progress, "bias")?;
    let mut dark = master(&cfg.darks, load, dir, progress, "dark")?;
    let mut flat = master(&cfg.flats, load, dir, progress, "flat")?;
    let fdark = master(&cfg.flatdarks, load, dir, progress, "flat dark")?;
    if dark.is_none() {
        dark = bias.clone();
    }
    if let Some(mut f) = flat.take() {
        let off = fdark.as_ref().or(bias.as_ref());
        if let Some(o) = off {
            if (o.h, o.w) == (f.h, f.w) {
                f = crate::image::sub(&f, o);
            }
        }
        if cfg.homogenize_vignetting {
            f = homogenize(&f);
        }
        let (h, w) = (f.h, f.w);
        let mid = f.crop(h / 4, 3 * h / 4, w / 4, 3 * w / 4);
        let n = mid.len().max(1) as f32;
        let mut m = [0f32; 3];
        for p in &mid.px {
            for c in 0..3 {
                m[c] += p[c];
            }
        }
        m.iter_mut().for_each(|v| *v /= n);
        let f = f.map(move |c| [(c[0] / m[0]).max(0.05), (c[1] / m[1]).max(0.05), (c[2] / m[2]).max(0.05)]);
        let (f, note) = denoise_flat(f);
        notes.extend(note);
        flat = Some(f);
    }
    let mut hot = Vec::new();
    if !cfg.darks.is_empty() {
        if let Some(d) = &dark {
            let l = d.luminance();
            let v: Vec<f32> = l.subsample(l.step_for(200_000)).collect();
            let (med, sigma) = median_mad(&v);
            let thr = med + (10.0 * sigma).max(0.002);
            for y in 0..l.h {
                for x in 0..l.w {
                    if l.at(y, x) > thr {
                        hot.push((y, x));
                    }
                }
            }
        }
    }
    Ok(Calibration { dark, flat, hot, notes })
}

/// Largest pixel-to-pixel noise left in a normalized master flat (fraction).
/// Above this, the flat would stamp its own noise onto every star image (and,
/// as the frames drift, into streaks across the stack).
pub const FLAT_NOISE_MAX: f64 = 0.004;

/// Pixel-to-pixel noise of each channel of a normalized flat (robust: MAD of the fine detail).
pub fn flat_noise(f: &Img) -> [f64; 3] {
    let hp = crate::image::sub(f, &blur(f, 2.0));
    let step = hp.step_for(200_000);
    let mut out = [0.0; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let v: Vec<f32> = hp.subsample(step).map(|p| p[c]).collect();
        *o = median_mad(&v).1 as f64;
    }
    out
}

/// A noisy master flat (too few or too dark flats, e.g. a channel the light
/// source hardly has) is smoothed per channel until its pixel noise is below
/// FLAT_NOISE_MAX. Vignetting and dust shadows are much wider than the
/// smoothing, so they stay. Returns the flat and a log note (if it was smoothed).
pub fn denoise_flat(f: Img) -> (Img, Option<String>) {
    let noise = flat_noise(&f);
    const NAMES: [&str; 3] = ["red", "green", "blue"];
    let mut planes: Vec<Gray> = (0..3).map(|c| f.channel(c)).collect();
    let mut done = Vec::new();
    for c in 0..3 {
        if noise[c] <= FLAT_NOISE_MAX {
            continue;
        }
        // A Gaussian of σ px lowers white noise by about 2·√π·σ.
        let sigma = (noise[c] / FLAT_NOISE_MAX / (2.0 * std::f64::consts::PI.sqrt())).clamp(1.0, 8.0);
        planes[c] = blur(&planes[c], sigma);
        done.push(format!("{} {:.1} % → smoothed over {:.1} px", NAMES[c], 100.0 * noise[c], sigma));
    }
    if done.is_empty() {
        return (f, None);
    }
    let out = Img::from_fn(f.h, f.w, |y, x| [planes[0].at(y, x), planes[1].at(y, x), planes[2].at(y, x)]);
    let after = flat_noise(&out);
    (
        out,
        Some(format!(
            "The flat frames are noisy ({}; now {:.2} / {:.2} / {:.2} %) – brighter or more flats help",
            done.join(", "),
            100.0 * after[0],
            100.0 * after[1],
            100.0 * after[2]
        )),
    )
}

/// Vignetting averaged over the four mirrored quadrants and smoothed (for uneven flats).
pub fn homogenize(f: &Img) -> Img {
    let (h, w) = (f.h, f.w);
    let s = Img::from_fn(h, w, |y, x| {
        let a = f.at(y, x);
        let b = f.at(h - 1 - y, x);
        let c = f.at(y, w - 1 - x);
        let d = f.at(h - 1 - y, w - 1 - x);
        [(a[0] + b[0] + c[0] + d[0]) / 4.0, (a[1] + b[1] + c[1] + d[1]) / 4.0, (a[2] + b[2] + c[2] + d[2]) / 4.0]
    });
    blur(&s, 0.01 * h.max(w) as f64)
}

pub fn calibrate(mut img: Img, cal: &Calibration) -> Result<Img, String> {
    if let Some(d) = &cal.dark {
        if (d.h, d.w) != (img.h, img.w) {
            return Err("The dark / bias frames do not have the size of the light frames.".into());
        }
        img = crate::image::sub(&img, d);
    }
    if let Some(f) = &cal.flat {
        if (f.h, f.w) != (img.h, img.w) {
            return Err("The flat frames do not have the size of the light frames.".into());
        }
        img = img.zip_map(f, |c, f| [c[0] / f[0], c[1] / f[1], c[2] / f[2]]);
    }
    fix_pixels(&mut img, &cal.hot);
    Ok(img)
}

/// Replace pixels by the median of their 8 neighbours.
pub fn fix_pixels(img: &mut Img, idx: &[(usize, usize)]) {
    let (h, w) = (img.h, img.w);
    for &(y, x) in idx {
        let mut nb: [Vec<f32>; 3] = [vec![], vec![], vec![]];
        for j in y.saturating_sub(1)..=(y + 1).min(h - 1) {
            for i in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                if (j, i) == (y, x) {
                    continue;
                }
                let p = img.at(j, i);
                for c in 0..3 {
                    nb[c].push(p[c]);
                }
            }
        }
        img.set(y, x, [median(&mut nb[0]), median(&mut nb[1]), median(&mut nb[2])]);
    }
}

/// Single pixels much brighter than all their neighbours (hot pixels without darks).
pub fn dynamic_hot_pixels(img: &Img) -> Vec<(usize, usize)> {
    let l = img.luminance();
    let (h, w) = (l.h, l.w);
    let v: Vec<f32> = l.subsample(l.step_for(200_000)).collect();
    let (med, sigma) = median_mad(&v);
    let sigma = sigma + 1e-6;
    (1..h.saturating_sub(1))
        .into_par_iter()
        .flat_map_iter(|y| {
            let l = &l;
            (1..w - 1).filter_map(move |x| {
                let p = l.at(y, x);
                if p <= med + 8.0 * sigma {
                    return None;
                }
                let m = [l.at(y - 1, x - 1), l.at(y - 1, x), l.at(y - 1, x + 1), l.at(y, x - 1), l.at(y, x + 1), l.at(y + 1, x - 1), l.at(y + 1, x), l.at(y + 1, x + 1)]
                    .into_iter()
                    .fold(f32::MIN, f32::max);
                if p > m + 6.0 * sigma && p > 2.0 * m {
                    Some((y, x))
                } else {
                    None
                }
            })
        })
        .collect()
}

pub fn merge4(img: &Img) -> Img {
    let mut b = crate::image::block_average(img, 2);
    b.map_inplace(|p| [4.0 * p[0], 4.0 * p[1], 4.0 * p[2]]);
    b
}

// --------------------------------------------------------------------------
// Sky region: weight 1 = sky, 0 = foreground (fractions in between: gradient)
// --------------------------------------------------------------------------

/// Sky weight (h × w): 1 = sky, 0 = foreground. Boundary line / Gradient: the
/// line, with the gradient zone below it. Irregular mask: the painted mask with a
/// soft edge `feather` (fraction of the width) wide, inside the sky only.
pub fn sky_weight(sky: &Sky, h: usize, w: usize) -> Gray {
    let mut wt = Gray::filled(h, w, 1.0);
    match sky.mode {
        SkyMode::BoundaryLine | SkyMode::Gradient => {
            let Some((u1, v1, u2, v2)) = sky.line else { return wt };
            if (u2 - u1).abs() < 0.02 {
                return wt;
            }
            let gw = (if sky.mode == SkyMode::Gradient { sky.gradient } else { 0.0 }).max(0.005); // a little feathering
            wt.px.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                for (x, p) in row.iter_mut().enumerate() {
                    let u = (x as f64 + 0.5) / w as f64;
                    let v0 = v1 + (u - u1) * (v2 - v1) / (u2 - u1);
                    *p = (1.0 - ((y as f64 + 0.5) / h as f64 - v0) / gw).clamp(0.0, 1.0) as f32;
                }
            });
        }
        SkyMode::IrregularMask if !sky.mask.is_empty() && sky.mask.any() => {
            let m = mask_weight(&sky.mask, sky.feather);
            let (mh, mw) = (m.h as f64, m.w as f64);
            wt.px.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                for (x, p) in row.iter_mut().enumerate() {
                    *p = bilinear(&m, (y as f64 + 0.5) / h as f64 * mh + 0.5, (x as f64 + 0.5) / w as f64 * mw + 0.5);
                }
            });
        }
        _ => {}
    }
    wt
}

/// Painted mask → weights at the mask's resolution: 0 outside, rising to 1 over `feather` × width inside.
pub fn mask_weight(mask: &Bits, feather: f64) -> Gray {
    let sigma = (feather * mask.w as f64 / 2.0).max(0.5);
    let b = blur(&mask.to_gray(), sigma);
    Gray::from_vec(mask.h, mask.w, b.px.iter().zip(mask.v.iter()).map(|(&b, &m)| if m { (2.0 * b - 1.0).clamp(0.0, 1.0) } else { 0.0 }).collect())
}

/// Sky mask (true = sky) of a landscape shot with the sky at the top, `rows` × `cols`
/// (rows: same aspect by default). Returns (mask, horizon as a fraction of the height per mask column).
pub fn detect_sky(img: &Img, cols: usize, rows: Option<usize>, margin: f64) -> (Bits, Vec<f64>) {
    let l = img.luminance();
    let (h, w) = (l.h, l.w);
    let rows = rows.unwrap_or_else(|| ((cols as f64 * h as f64 / w as f64).round() as usize).max(1));
    let v: Vec<f32> = l.subsample(l.step_for(200_000)).collect();
    let m = median_of(v.iter().copied()).max(1e-5);
    let a = l.map(move |x| (x + 0.05 * m).ln()); // brightness, scale-free
    let hp = crate::image::sub(&l, &blur(&l, 2.0));
    let hv: Vec<f32> = hp.subsample(hp.step_for(200_000)).collect();
    let (_, sigma) = median_mad(&hv);
    let sigma = sigma + 1e-7;
    let c = (w / 48).max(4);
    let (nx, ny) = (w.div_ceil(c), h.div_ceil(c));
    let mut bgrid = vec![0f64; ny * nx];
    let mut dgrid = vec![0f64; ny * nx];
    bgrid.par_chunks_mut(nx).zip(dgrid.par_chunks_mut(nx)).enumerate().for_each(|(i, (brow, drow))| {
        for j in 0..nx {
            let (y0, y1) = (i * c, ((i + 1) * c).min(h));
            let (x0, x1) = (j * c, ((j + 1) * c).min(w));
            let mut vals: Vec<f32> = Vec::with_capacity((y1 - y0) * (x1 - x0));
            let mut n = 0;
            for y in y0..y1 {
                for x in x0..x1 {
                    vals.push(a.at(y, x));
                    if !(1 <= y && y <= h - 2 && 1 <= x && x <= w - 2) {
                        continue;
                    }
                    let p = hp.at(y, x);
                    if p <= 6.0 * sigma {
                        continue;
                    }
                    if p >= hp.at(y - 1, x) && p >= hp.at(y + 1, x) && p >= hp.at(y, x - 1) && p >= hp.at(y, x + 1) {
                        n += 1;
                    }
                }
            }
            brow[j] = median(&mut vals) as f64;
            drow[j] = n as f64;
        }
    });
    let mut diffs: Vec<f32> = Vec::new();
    for i in 0..ny.saturating_sub(1) {
        for j in 0..nx {
            diffs.push((bgrid[(i + 1) * nx + j] - bgrid[i * nx + j]).abs() as f32);
        }
    }
    let sb = (1.4826 * median(&mut diffs) as f64).max(0.02); // row-to-row brightness noise
    let topn = (ny / 4).max(1);
    let sd = dgrid[..topn * nx].iter().sum::<f64>() / (topn * nx) as f64 + 0.5; // star density near the top
    let mut hz = vec![ny as f64; nx]; // coarse horizon (rows of sky)
    for j in 0..nx {
        let (mut best, mut at) = (0.0, ny);
        let col_b: Vec<f64> = (0..ny).map(|i| bgrid[i * nx + j]).collect();
        let col_d: Vec<f64> = (0..ny).map(|i| dgrid[i * nx + j]).collect();
        for r in 1..ny {
            let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
            let db = mean(&col_b[..r]) - mean(&col_b[r..]);
            let dd = mean(&col_d[..r]) - mean(&col_d[r..]);
            let sc = 2.0 * ((r * (ny - r)) as f64).sqrt() / ny as f64 * (db.max(0.0) / (4.0 * sb) + dd.max(0.0) / sd);
            if sc > best {
                best = sc;
                at = r;
            }
        }
        if best > 1.0 {
            hz[j] = at as f64;
        }
    }
    let hz: Vec<f64> = (0..nx).map(|j| median_f64(hz[j.saturating_sub(2)..(j + 3).min(nx)].iter().copied())).collect();
    // Fine: the steepest drop of the smoothed brightness within a cell of the coarse horizon.
    let a_s = blur(&a, 1.5);
    let mut horizon = vec![0f64; cols];
    for u in 0..cols {
        let x0 = ((u * w) / cols).min(w - 1);
        let x1 = (((u + 1) * w) / cols).max(x0 + 1).min(w);
        let j = (((x0 + x1) as f64 / 2.0) / c as f64).floor().clamp(0.0, (nx - 1) as f64) as usize;
        let y0 = hz[j] * c as f64;
        if hz[j] >= ny as f64 {
            horizon[u] = 1.0 + margin; // all sky
            continue;
        }
        let (yb, mut ystar, mut drop) = (y0.round() as i64, y0.round() as i64, 0.0);
        let lo = ((y0 - c as f64).round() as i64).max(2);
        let hi = ((y0 + c as f64).round() as i64).min(h as i64 - 3);
        let rowmean = |y: i64| -> f64 {
            let yy = y as usize;
            (x0..x1).map(|x| a_s.at(yy, x) as f64).sum::<f64>() / (x1 - x0) as f64
        };
        let mut y = lo;
        while y <= hi {
            let d = rowmean(y - 2) - rowmean(y + 2);
            if d > drop {
                drop = d;
                ystar = y;
            }
            y += 1;
        }
        horizon[u] = (if drop > 2.0 * sb { ystar } else { yb }) as f64 / h as f64;
    }
    let horizon: Vec<f64> = (0..cols).map(|u| median_f64(horizon[u.saturating_sub(3)..(u + 4).min(cols)].iter().copied()) - margin).collect();
    let mask = Bits::from_fn(rows, cols, |r, u| (r as f64 + 0.5) / (rows as f64) < horizon[u]);
    (mask, horizon.iter().map(|v| v.clamp(0.0, 1.0)).collect())
}

// --------------------------------------------------------------------------
// Stars
// --------------------------------------------------------------------------

/// Median and noise (MAD σ) of `l` on a grid of `tile`-pixel tiles.
pub fn background_grid(l: &Gray, tile: usize) -> (Gray, Gray) {
    let (h, w) = (l.h, l.w);
    let (ny, nx) = (h.div_ceil(tile), w.div_ceil(tile));
    let mut b = Gray::new(ny, nx);
    let mut n = Gray::new(ny, nx);
    b.px.par_chunks_mut(nx).zip(n.px.par_chunks_mut(nx)).enumerate().for_each(|(i, (brow, nrow))| {
        for j in 0..nx {
            let mut v: Vec<f32> = Vec::new();
            let mut y = i * tile;
            while y < ((i + 1) * tile).min(h) {
                let mut x = j * tile;
                while x < ((j + 1) * tile).min(w) {
                    v.push(l.at(y, x));
                    x += 2;
                }
                y += 2;
            }
            let m = median(&mut v);
            let mut d: Vec<f32> = v.iter().map(|x| (x - m).abs()).collect();
            brow[j] = m;
            nrow[j] = 1.4826 * median(&mut d);
        }
    });
    (b, n)
}

#[inline]
fn grid_at(g: &Gray, tile: usize, y: usize, x: usize) -> f32 {
    bilinear(g, (y as f64 + 0.5) / tile as f64 + 0.5, (x as f64 + 0.5) / tile as f64 + 0.5)
}

/// Stars in the sky region (weight ≈ 1): local maxima of the smoothed luminance
/// DETECT_SIGMA × noise above the background, measured by moments (centroid,
/// flux, FWHM, elongation). Brightest first, at most MAX_STARS. Positions are 1-based.
pub fn detect_stars(img: &Img, wt: &Gray) -> Vec<Star> {
    let l = img.luminance();
    let s = blur(&l, 1.0);
    let tile = 64;
    let (b, n) = background_grid(&s, tile);
    let (h, w) = (l.h, l.w);
    let (r, rr, margin) = (3i64, 6i64, 9usize);
    if h <= 2 * margin || w <= 2 * margin {
        return vec![];
    }
    let mut stars: Vec<Star> = (margin..h - margin)
        .into_par_iter()
        .flat_map_iter(|y| {
            let (l, s, b, n, wt) = (&l, &s, &b, &n, wt);
            (margin..w - margin).filter_map(move |x| {
                let v = s.at(y, x);
                let bg = grid_at(b, tile, y, x);
                let sigma = grid_at(n, tile, y, x).max(1e-6);
                if !(v > bg + DETECT_SIGMA * sigma && wt.at(y, x) >= 0.99) {
                    return None;
                }
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let u = s.at((y as i64 + dy) as usize, (x as i64 + dx) as usize);
                        if u > v || (u == v && (dy, dx) < (0, 0)) {
                            return None;
                        }
                    }
                }
                let lp = |dy: i64, dx: i64| l.at((y as i64 + dy) as usize, (x as i64 + dx) as usize) as f64;
                let (mut sum, mut sx, mut sy) = (0.0, 0.0, 0.0);
                for dy in -rr..=rr {
                    for dx in -rr..=rr {
                        let p = lp(dy, dx) - bg as f64 - sigma as f64;
                        if p > 0.0 {
                            sum += p;
                            sx += p * dx as f64;
                            sy += p * dy as f64;
                        }
                    }
                }
                if sum <= 0.0 {
                    return None;
                }
                let (mx, my) = (sx / sum, sy / sum);
                let (mut mxx, mut myy, mut mxy) = (0.0, 0.0, 0.0);
                for dy in -rr..=rr {
                    for dx in -rr..=rr {
                        let p = lp(dy, dx) - bg as f64 - sigma as f64;
                        if p > 0.0 {
                            mxx += p * (dx as f64 - mx).powi(2);
                            myy += p * (dy as f64 - my).powi(2);
                            mxy += p * (dx as f64 - mx) * (dy as f64 - my);
                        }
                    }
                }
                mxx /= sum;
                myy /= sum;
                mxy /= sum;
                let d = (((mxx - myy) / 2.0).powi(2) + mxy * mxy).sqrt();
                let l1 = (mxx + myy) / 2.0 + d;
                let l2 = ((mxx + myy) / 2.0 - d).max(1e-3);
                // FWHM from the area above half the peak (robust to noise and the window size)
                let half = (lp(0, 0) - bg as f64) / 2.0;
                let mut area = 0usize;
                for dy in -rr..=rr {
                    for dx in -rr..=rr {
                        if lp(dy, dx) - bg as f64 > half {
                            area += 1;
                        }
                    }
                }
                let fwhm = 2.0 * (area as f64 / std::f64::consts::PI).sqrt();
                if area as f64 >= 0.6 * ((2 * rr + 1) * (2 * rr + 1)) as f64 {
                    return None; // fills the window: not a star
                }
                Some(Star { x: x as f64 + 1.0 + mx, y: y as f64 + 1.0 + my, flux: sum, fwhm, elong: (l1 / l2).sqrt(), peak: lp(0, 0) })
            })
        })
        .collect();
    // Extended sources (horizon edges, lit clouds, nebulae) are not stars.
    if !stars.is_empty() {
        let mf = median_f64(stars.iter().map(|s| s.fwhm));
        stars.retain(|s| s.fwhm < 2.5 * mf);
    }
    stars.sort_by(|a, b| b.flux.partial_cmp(&a.flux).unwrap_or(std::cmp::Ordering::Equal));
    stars.truncate(MAX_STARS);
    stars
}

// --------------------------------------------------------------------------
// Alignment: triangles of neighbouring bright stars are matched by their shape
// (ratios of the sides), which gives candidate star pairs; RANSAC picks the
// consistent ones, then all stars refine the transform.
// --------------------------------------------------------------------------

type Tri = ((f64, f64), [usize; 3]);

fn triangles(stars: &[Star]) -> Vec<Tri> {
    let n = stars.len().min(ALIGN_STARS);
    let p: Vec<(f64, f64)> = stars[..n].iter().map(|s| (s.x, s.y)).collect();
    let dist = |i: usize, j: usize| (p[i].0 - p[j].0).hypot(p[i].1 - p[j].1);
    let mut tris = std::collections::HashSet::<[usize; 3]>::new();
    for i in 0..n {
        let mut idx: Vec<usize> = (0..n).collect();
        idx.sort_by(|&a, &b| dist(i, a).partial_cmp(&dist(i, b)).unwrap());
        let nb = &idx[1..(6).min(n)]; // 5 nearest neighbours
        for a in 0..nb.len() {
            for b in a + 1..nb.len() {
                let mut t = [i, nb[a], nb[b]];
                t.sort();
                tris.insert(t);
            }
        }
    }
    let mut out = Vec::new();
    for [i, j, k] in tris {
        // vertices ordered by the length of the opposite side (shortest first)
        let mut sides = [(dist(j, k), i), (dist(i, k), j), (dist(i, j), k)];
        sides.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let ((l0, a), (l1, b), (l2, c)) = (sides[0], sides[1], sides[2]);
        if l0 <= 3.0 {
            continue;
        }
        out.push(((l2 / l1, l1 / l0), [a, b, c]));
    }
    out
}

/// Transform T (base pixel → frame pixel) from the stars of both images. Returns
/// (T, number of matched stars, RMS error in px); fails if they cannot be matched.
pub fn align_stars(base: &[Star], frame: &[Star], w: usize, model: Model) -> Result<(M3, usize, f64), String> {
    if base.len() < 8 || frame.len() < 8 {
        return Err("too few stars".into());
    }
    let (tb, tf) = (triangles(base), triangles(frame));
    let mut bp: Vec<Pt> = Vec::new();
    let mut fp: Vec<Pt> = Vec::new();
    for (inv_, v) in &tf {
        if tb.is_empty() {
            break;
        }
        let (mut d, mut k) = (f64::MAX, 0);
        for (t, tri) in tb.iter().enumerate() {
            let dd = (tri.0 .0 - inv_.0).hypot(tri.0 .1 - inv_.1);
            if dd < d {
                d = dd;
                k = t;
            }
        }
        if d >= 0.02 {
            continue;
        }
        for q in 0..3 {
            let (b, f) = (base[tb[k].1[q]], frame[v[q]]);
            bp.push([b.x, b.y]);
            fp.push([f.x, f.y]);
        }
    }
    if bp.len() < 12 {
        return Err("no matching star patterns".into());
    }
    let tol = (0.004 * w as f64).max(3.0);
    let (mut t, inl) = st::ransac(&bp, &fp, Model::Similarity, 1000, tol)?;
    if inl.len() < 9 {
        return Err("star patterns do not agree".into());
    }
    // Refine with every star: nearest neighbours under the current transform.
    let bm: Vec<Pt> = base.iter().map(|s| [s.x, s.y]).collect();
    let fm: Vec<Pt> = frame.iter().map(|s| [s.x, s.y]).collect();
    let (mut matched, mut err) = (0usize, f64::NAN);
    for r in [2.0 * tol, tol, (0.001 * w as f64).max(1.5), (0.0007 * w as f64).max(1.0)] {
        let (src, dst) = star_pairs(|x, y| apply_h(&t, x, y), &bm, &fm, r);
        let n = src.len();
        if n < 6 {
            return Err(format!("only {n} stars match"));
        }
        t = fit_model(if n >= 15 { model } else { Model::Similarity }, &src, &dst)?;
        matched = n;
        err = (src.iter().zip(dst.iter()).map(|(s, d)| reproj_err(&t, *s, *d).powi(2)).sum::<f64>() / n as f64).sqrt();
    }
    Ok((t, matched, err))
}

pub fn starmatrix(stars: &[Star]) -> Vec<Pt> {
    stars.iter().map(|s| [s.x, s.y]).collect()
}

/// Star pairs (base, frame) that land within r of each other under `f`.
pub fn star_pairs(f: impl Fn(f64, f64) -> (f64, f64), b: &[Pt], fr: &[Pt], r: f64) -> (Vec<Pt>, Vec<Pt>) {
    let mut src = Vec::new();
    let mut dst = Vec::new();
    if fr.is_empty() {
        return (src, dst);
    }
    for p in b {
        let (px, py) = f(p[0], p[1]);
        let (mut d, mut j) = (f64::MAX, 0);
        for (k, q) in fr.iter().enumerate() {
            let dd = (q[0] - px).hypot(q[1] - py);
            if dd < d {
                d = dd;
                j = k;
            }
        }
        if d < r {
            src.push(*p);
            dst.push(fr[j]);
        }
    }
    (src, dst)
}

// --------------------------------------------------------------------------
// Distortion: "Lens" fits one radial distortion term for all frames; "Complex"
// adds, per frame, a smooth correction grid learned from the remaining offsets.
// --------------------------------------------------------------------------

/// Homography of undistorted star pairs and its RMS error (undistorted pixels).
fn lens_fit(lens: &Warp, b: &[Pt], f: &[Pt]) -> Result<(M3, f64), String> {
    let n = b.len();
    let ub: Vec<Pt> = b.iter().map(|p| { let (x, y) = lens.undistort(p[0], p[1]); [x, y] }).collect();
    let uf: Vec<Pt> = f.iter().map(|p| { let (x, y) = lens.undistort(p[0], p[1]); [x, y] }).collect();
    let h = fit_model(Model::Homography, &ub, &uf)?;
    let mut e: Vec<f64> = (0..n).map(|i| reproj_err(&h, ub[i], uf[i]).powi(2)).collect();
    e.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let keep = ((0.8 * n as f64).round() as usize).max(1); // the worst 20 % may be wrong pairs
    Ok((h, (e[..keep].iter().sum::<f64>() / keep as f64).sqrt()))
}

/// Radial distortion k1 that lets one homography per frame fit best (golden-section search).
fn fit_k1(pairs: &[(Vec<Pt>, Vec<Pt>)], c: Pt, r: f64) -> f64 {
    let cost = |k: f64| -> f64 {
        let lens = Warp::with_lens(M3::identity(), k, c, r);
        pairs.iter().map(|(b, f)| lens_fit(&lens, b, f).map(|(_, e)| e * e * b.len() as f64).unwrap_or(f64::MAX)).sum()
    };
    let (mut a, mut b) = (-0.3f64, 0.3f64);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut x1, mut x2) = (b - g * (b - a), a + g * (b - a));
    let (mut f1, mut f2) = (cost(x1), cost(x2));
    for _ in 0..30 {
        if f1 < f2 {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = cost(x1);
        } else {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = cost(x2);
        }
    }
    let k = (a + b) / 2.0;
    if cost(k) < cost(0.0) {
        k
    } else {
        0.0
    }
}

/// Correction grid: star offsets left by `wp`, smoothed (kernel average, shrunk to 0 where stars are sparse).
fn fit_grid(wp: &Warp, b: &[Pt], f: &[Pt], h: usize, w: usize) -> Warp {
    let nx = 12usize;
    let cell = (w as f64 - 1.0) / (nx - 1) as f64;
    let ny = (((h as f64 - 1.0) / cell).ceil() as usize + 1).max(2);
    let r: Vec<(f64, f64)> = (0..b.len()).map(|i| { let (px, py) = wp.mapto(b[i][0], b[i][1]); (f[i][0] - px, f[i][1] - py) }).collect();
    let mut grid = vec![[0.0; 2]; ny * nx];
    let s2 = 2.0 * (1.2 * cell).powi(2);
    for i in 0..ny {
        for j in 0..nx {
            let (gx, gy) = (j as f64 * cell + 1.0, i as f64 * cell + 1.0);
            let (mut sw, mut sx, mut sy) = (2.0, 0.0, 0.0); // 2.0: pull towards no correction
            for k in 0..b.len() {
                let wk = (-((b[k][0] - gx).powi(2) + (b[k][1] - gy).powi(2)) / s2).exp();
                sw += wk;
                sx += wk * r[k].0;
                sy += wk * r[k].1;
            }
            grid[i * nx + j] = [sx / sw, sy / sw];
        }
    }
    Warp { h: wp.h, hi: wp.hi, k1: wp.k1, c: wp.c, r: wp.r, grid, ny, nx, cell }
}

/// Replace each aligned frame's homography by a warp with distortion correction
/// (`complex` adds the grid). Updates matched stars and errors. Returns k1.
pub fn correct_distortion(frames: &mut [FrameInfo], base_idx: usize, complex: bool, h: usize, w: usize) -> f64 {
    let c = [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0];
    let r = (w as f64).hypot(h as f64) / 2.0;
    let b = starmatrix(&frames[base_idx].detected);
    let (rwide, rtight) = ((0.006 * w as f64).max(4.0), (0.0012 * w as f64).max(1.5));
    let mut pairs: HashMap<usize, (Vec<Pt>, Vec<Pt>)> = HashMap::new();
    for (k, fi) in frames.iter().enumerate() {
        if k == base_idx {
            continue;
        }
        let t = fi.t;
        let p = star_pairs(|x, y| apply_h(&t, x, y), &b, &starmatrix(&fi.detected), rwide);
        if p.0.len() >= 12 {
            pairs.insert(k, p);
        }
    }
    if pairs.is_empty() {
        return 0.0;
    }
    let k1 = fit_k1(&pairs.values().cloned().collect::<Vec<_>>(), c, r);
    let lens = Warp::with_lens(M3::identity(), k1, c, r);
    frames[base_idx].warp = lens.clone(); // the identity, whatever k1
    for (k, (bp, fp)) in pairs {
        let Ok((hm, _)) = lens_fit(&lens, &bp, &fp) else { continue };
        let mut wp = Warp::with_lens(hm, k1, c, r);
        if complex {
            wp = fit_grid(&wp, &bp, &fp, h, w);
        }
        // Pairs again with the better model, tighter; refit once.
        let (bp, fp) = star_pairs(|x, y| wp.mapto(x, y), &b, &starmatrix(&frames[k].detected), rtight);
        if bp.len() < 12 {
            continue;
        }
        let Ok((hm, _)) = lens_fit(&lens, &bp, &fp) else { continue };
        let mut wp = Warp::with_lens(hm, k1, c, r);
        if complex {
            wp = fit_grid(&wp, &bp, &fp, h, w);
        }
        let err = (bp.iter().zip(fp.iter()).map(|(bq, fq)| { let (px, py) = wp.mapto(bq[0], bq[1]); (fq[0] - px).powi(2) + (fq[1] - py).powi(2) }).sum::<f64>() / bp.len() as f64).sqrt();
        let fi = &mut frames[k];
        fi.warp = wp;
        fi.matched = bp.len();
        fi.err = err;
    }
    k1
}

// --------------------------------------------------------------------------
// Combining
// --------------------------------------------------------------------------

/// Mean of v[..n] after iterative sigma clipping (first centre: the median).
pub fn sigma_mean(v: &mut [f32], n: usize, lo: f64, hi: f64, iters: usize) -> f32 {
    if n == 0 {
        return 0.0;
    }
    if n <= 2 {
        return v[..n].iter().sum::<f32>() / n as f32;
    }
    let c = median(&mut v[..n]);
    let mut m = n;
    for it in 0..iters {
        let mu = v[..m].iter().sum::<f32>() / m as f32;
        let sigma = (v[..m].iter().map(|x| (x - mu).powi(2)).sum::<f32>() / (m - 1) as f32).sqrt();
        if sigma <= 0.0 {
            break;
        }
        let ctr = if it == 0 { c } else { mu };
        let mut j = 0;
        for i in 0..m {
            if ctr - lo as f32 * sigma <= v[i] && v[i] <= ctr + hi as f32 * sigma {
                v[j] = v[i];
                j += 1;
            }
        }
        if j == m || j < 2 {
            break;
        }
        m = j;
    }
    v[..m].iter().sum::<f32>() / m as f32
}

/// Brightness of each of K trail frames: 1, or rising from 0.1 (Fade in), falling (Fade out), or both.
pub fn trail_fade(kind: Fade, k: usize) -> Vec<f32> {
    (1..=k)
        .map(|i| {
            let (i, kf) = (i as f64, k as f64);
            (match kind {
                Fade::In => 0.1 + 0.9 * (i / kf).powi(2),
                Fade::Out => 0.1 + 0.9 * ((kf - i + 1.0) / kf).powi(2),
                Fade::Both => 0.1 + 0.9 * (3.0 * ((i - 0.5) / kf).min(1.0 - (i - 0.5) / kf)).min(1.0).powi(2),
                Fade::Off => 1.0,
            }) as f32
        })
        .collect()
}

pub fn fade_kind(cfg: &StackConfig) -> Fade {
    if cfg.trails.0 == Fade::Off && cfg.motion_effect {
        Fade::In
    } else {
        cfg.trails.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Accumulation,
    Best,
    Freeze,
    AlignOnly,
    Trails,
}

/// The stacked image (linear) of the frames `ks` of the store with infos `frames`.
/// Star trails: `gaps[k]` (frame k pixel → frame k+1 pixel) fills the gap between
/// frames k and k+1 with `steps` copies of frame k shifted along the way.
#[allow(clippy::too_many_arguments)]
pub fn combine(d: &FrameStore, ks: &[usize], frames: &[&FrameInfo], wt: &Gray, cfg: &StackConfig, mode: Mode, progress: Progress, ref_: Option<&Warp>, gaps: Option<&[Option<M3>]>, steps: usize) -> Img {
    let (w, h) = (d.w, d.h);
    let kn = frames.len();
    let (lo, hi, its) = (cfg.sigma.low, cfg.sigma.high, cfg.sigma.iters);
    let warps: Vec<&Warp> = frames.iter().map(|f| &f.warp).collect();
    let lz = cfg.interpolation == Interpolation::Lanczos;
    let gains: Vec<Rgb> = frames.iter().map(|f| f.gain).collect();
    let fade = trail_fade(fade_kind(cfg), kn);
    let fill = mode == Mode::Trails && gaps.is_some() && steps > 0;
    let sel = cfg.selective;
    let mut out = Img::new(h, w);
    let done = std::sync::atomic::AtomicUsize::new(0);
    out.px.par_chunks_mut(w * 4).enumerate().for_each(|(chunk, rows)| {
        let mut sky = [vec![0f32; kn], vec![0f32; kn], vec![0f32; kn]];
        let mut gnd = [vec![0f32; kn], vec![0f32; kn], vec![0f32; kn]];
        for (ri, row) in rows.chunks_mut(w).enumerate() {
            let y = chunk * 4 + ri;
            for (xi, outp) in row.iter_mut().enumerate() {
                let x = xi;
                let m = wt.at(y, x);
                let (mut ns, mut ng) = (0usize, 0usize);
                let (x1, y1) = (x as f64 + 1.0, y as f64 + 1.0);
                if mode != Mode::Trails && (m > 0.0 || mode != Mode::Freeze) {
                    // time-lapse: output pixels belong to the window's reference frame
                    let (bx, by) = match ref_ {
                        None => (x1, y1),
                        Some(r) => r.unmap(x1, y1),
                    };
                    for k in 0..kn {
                        let (px, py) = warps[k].mapto(bx, by);
                        let s = if lz { d.sample_lanczos(ks[k], px, py) } else { d.sample(ks[k], px, py) };
                        let Some(s) = s else { continue };
                        for c in 0..3 {
                            sky[c][ns] = s[c] * gains[k][c];
                        }
                        ns += 1;
                    }
                }
                if (mode == Mode::Freeze && m < 1.0) || mode == Mode::Trails {
                    for k in 0..kn {
                        let p = d.pixel(ks[k], x, y);
                        for c in 0..3 {
                            gnd[c][ng] = p[c] * gains[k][c];
                        }
                        ng += 1;
                    }
                }
                let mut gap = [0f32; 3];
                if fill && m > 0.0 {
                    let gaps = gaps.unwrap();
                    for k in 0..kn - 1 {
                        let Some(mm) = &gaps[k] else { continue };
                        let (qx, qy) = apply_h(mm, x1, y1);
                        let (dx, dy) = (qx - x1, qy - y1);
                        for j in 1..=steps {
                            let t = j as f64 / (steps + 1) as f64;
                            let Some(s) = d.sample(ks[k], x1 - t * dx, y1 - t * dy) else { continue };
                            let f = fade[k] + t as f32 * (fade[k + 1] - fade[k]);
                            for c in 0..3 {
                                gap[c] = gap[c].max(f * s[c] * gains[k][c]);
                            }
                        }
                    }
                }
                let mut val = [0f32; 3];
                for c in 0..3 {
                    val[c] = match mode {
                        Mode::Trails => {
                            let t = if m > 0.0 { (0..ng).map(|k| fade[k] * gnd[c][k]).fold(f32::MIN, f32::max).max(gap[c]) } else { 0.0 };
                            let g = if m < 1.0 { sigma_mean(&mut gnd[c], ng, lo, hi, its) } else { 0.0 };
                            m * t + (1.0 - m) * g
                        }
                        Mode::Freeze => {
                            let s = if m > 0.0 {
                                if sel {
                                    sigma_mean(&mut sky[c], ns, lo, hi, its)
                                } else if ns == 0 {
                                    0.0
                                } else {
                                    sky[c][..ns].iter().sum::<f32>() / ns as f32
                                }
                            } else {
                                0.0
                            };
                            let g = if m < 1.0 { sigma_mean(&mut gnd[c], ng, lo, hi, its) } else { 0.0 };
                            m * s + (1.0 - m) * g
                        }
                        Mode::Best => sigma_mean(&mut sky[c], ns, lo, hi, its),
                        _ => {
                            let a = if ns == 0 { 0.0 } else { sky[c][..ns].iter().sum::<f32>() / ns as f32 };
                            if sel && m > 0.0 {
                                m * sigma_mean(&mut sky[c], ns, lo, hi, its) + (1.0 - m) * a
                            } else {
                                a
                            }
                        }
                    };
                }
                *outp = val;
            }
        }
        let n = done.fetch_add(rows.len() / w, std::sync::atomic::Ordering::Relaxed) + rows.len() / w;
        if rayon::current_thread_index() == Some(0) {
            let _ = progress(&format!("Combining: {} %", (100 * n / h).min(100)));
        }
    });
    out
}

/// Frame k resampled onto the base image (for Align only).
pub fn resample(d: &FrameStore, k: usize, wp: &Warp, lanczos_: bool) -> Img {
    let (w, h) = (d.w, d.h);
    Img::from_fn(h, w, |y, x| {
        let (px, py) = wp.mapto(x as f64 + 1.0, y as f64 + 1.0);
        let s = if lanczos_ { d.sample_lanczos(k, px, py) } else { d.sample(k, px, py) };
        s.unwrap_or([0.0; 3])
    })
}

#[allow(dead_code)]
fn _lum(c: Rgb) -> f32 {
    luminance(c)
}

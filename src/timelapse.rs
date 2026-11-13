//! Time-lapse videos from a sequence of frames. Every frame is cut to the
//! video's shape (16:9, a slow zoom, a slider-like move across the frame),
//! scaled, evened out in brightness (deflicker) and given the look: the
//! Post-processing settings, a look for the whole frame, and looks for the sky
//! and the foreground through a painted mask.
//!
//! Numbered images and animated GIF are written here; MP4 and WebM go through
//! ffmpeg (found on the PATH, next to the program, or in PANOSTITCH_FFMPEG).

use crate::finish::{self, FinishParams, Local, Measured};
use crate::image::{crop_resize, luminance, resize, Gray, Img};
use crate::io::{self, basename};
use crate::stacker::{linear_to_srgb, srgb_to_linear};
use rayon::prelude::*;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Which way a slider move goes: the direction the view travels across the frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pan {
    LeftToRight,
    RightToLeft,
    Up,
    Down,
}

impl Pan {
    pub const ALL: [Pan; 4] = [Pan::LeftToRight, Pan::RightToLeft, Pan::Up, Pan::Down];
    pub fn name(self) -> &'static str {
        match self {
            Pan::LeftToRight => "Left → right",
            Pan::RightToLeft => "Right → left",
            Pan::Up => "Up",
            Pan::Down => "Down",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Mp4,
    WebM,
    Gif,
    Sequence,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Mp4, Format::WebM, Format::Gif, Format::Sequence];
    pub fn name(self) -> &'static str {
        match self {
            Format::Mp4 => "MP4 video (H.264)",
            Format::WebM => "WebM video (VP9)",
            Format::Gif => "Animated GIF",
            Format::Sequence => "Numbered images (TIFF / PNG / JPEG)",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::WebM => "webm",
            Format::Gif => "gif",
            Format::Sequence => "tif",
        }
    }
    /// Made by ffmpeg.
    pub fn needs_ffmpeg(self) -> bool {
        matches!(self, Format::Mp4 | Format::WebM)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolution {
    Frames,
    Uhd,
    Qhd,
    FullHd,
    Hd,
}

impl Resolution {
    pub const ALL: [Resolution; 5] = [Resolution::Frames, Resolution::Uhd, Resolution::Qhd, Resolution::FullHd, Resolution::Hd];
    pub fn name(self) -> &'static str {
        match self {
            Resolution::Frames => "As the frames",
            Resolution::Uhd => "4K UHD (3840 × 2160)",
            Resolution::Qhd => "1440p (2560 × 1440)",
            Resolution::FullHd => "1080p (1920 × 1080)",
            Resolution::Hd => "720p (1280 × 720)",
        }
    }
    /// The largest video (w, h) for a landscape frame; None = the frames' size.
    pub fn limit(self) -> Option<(usize, usize)> {
        match self {
            Resolution::Frames => None,
            Resolution::Uhd => Some((3840, 2160)),
            Resolution::Qhd => Some((2560, 1440)),
            Resolution::FullHd => Some((1920, 1080)),
            Resolution::Hd => Some((1280, 720)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Deflicker {
    Off,
    Light,
    Strong,
}

/// Everything a video is made from.
#[derive(Clone, Debug)]
pub struct Settings {
    pub frames: Vec<String>,
    /// How camera RAW frames are decoded. Built-in by default: fast, and the
    /// same for every frame. RawTherapee applies its .pp3 edits but takes
    /// about ten times as long per frame.
    pub decoder: io::RawDecoder,
    pub deflicker: Deflicker,
    /// Deflicker: brightness is evened out over this many neighbouring frames (Strong: three times as many).
    pub deflicker_window: u32,
    /// Deflicker each colour on its own (evens out colour changes too).
    pub deflicker_colour: bool,
    /// Deflicker region by region (a coarse grid), evening out large-scale changes
    /// across the frame; otherwise the whole frame alike.
    pub deflicker_regions: bool,
    /// The Post-processing settings, applied first (turning and cropping left out).
    pub base: Option<FinishParams>,
    /// The look for the whole frame.
    pub look: FinishParams,
    /// Sky and foreground looks with their mask (on the whole frame).
    pub local: Local,
    /// Slow zoom in over the whole video, in percent.
    pub zoom: f64,
    /// Slider move: the view is this many percent smaller than the frame and
    /// travels across it during the video (0 = no move).
    pub pan: f64,
    pub pan_dir: Pan,
    /// Zoom and move start and stop gently instead of at a constant speed.
    pub ease: bool,
    pub format: Format,
    pub fps: u32,
    pub resolution: Resolution,
    pub crop_16_9: bool,
    /// 30…100: video quality, JPEG quality of numbered images.
    pub quality: u32,
    /// Play forward, then back.
    pub bounce: bool,
    /// Seconds the last frame stays on.
    pub hold_last: f64,
    pub out: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            decoder: io::RawDecoder::Builtin,
            deflicker: Deflicker::Light,
            deflicker_window: 7,
            deflicker_colour: true,
            deflicker_regions: true,
            base: None,
            look: FinishParams::default(),
            local: Local::default(),
            zoom: 0.0,
            pan: 0.0,
            pan_dir: Pan::LeftToRight,
            ease: true,
            format: Format::Mp4,
            fps: 25,
            resolution: Resolution::FullHd,
            crop_16_9: true,
            quality: 80,
            bounce: false,
            hold_last: 0.0,
            out: String::new(),
        }
    }
}

/// What was made.
#[derive(Clone, Debug)]
pub struct Done {
    /// Frames read.
    pub frames: usize,
    /// Frames in the video (with the held last frame and the way back).
    pub video_frames: usize,
    pub w: usize,
    pub h: usize,
    pub out: String,
    /// Files written (numbered images; 1 for a video).
    pub files: usize,
}

pub const NO_FFMPEG: &str = "MP4 and WebM are made with ffmpeg, which was not found. Install it (ffmpeg.org; on Windows e.g. “winget install ffmpeg”, on Linux from the package manager), put ffmpeg next to this program, or choose Animated GIF or Numbered images.";

/// Frames loaded at the same time (each needs memory for a full frame).
const PAR: usize = 4;

// --------------------------------------------------------------------------
// Frame shape
// --------------------------------------------------------------------------

/// The part of a `w × h` frame the video shows at `t` (0 = start … 1 = end):
/// the 16:9 cut, the slow zoom and the slider move, as `finish::geometry`
/// settings (crop fractions; `render` uses them without rounding to pixels).
pub fn frame_crop(w: usize, h: usize, s: &Settings, t: f64) -> FinishParams {
    let (w, h) = (w as f64, h as f64);
    let (mut kx, mut ky) = (1.0, 1.0);
    if s.crop_16_9 {
        let r = if w >= h { 16.0 / 9.0 } else { 9.0 / 16.0 };
        if w / h > r {
            kx = h * r / w;
        } else {
            ky = w / r / h;
        }
    }
    let t = t.clamp(0.0, 1.0);
    let t = if s.ease { t * t * (3.0 - 2.0 * t) } else { t };
    // The zoomed part, centred; the view (smaller by the move) travels inside it.
    let z = 1.0 + s.zoom.clamp(0.0, 50.0) / 100.0 * t;
    let (zx, zy) = (kx / z, ky / z);
    let p = 1.0 + s.pan.clamp(0.0, 30.0) / 100.0;
    let (vx, vy) = (zx / p, zy / p);
    let (cx, cy) = ((1.0 - zx) / 2.0, (1.0 - zy) / 2.0);
    let (rx, ry) = (zx - vx, zy - vy);
    let (x0, y0) = match s.pan_dir {
        Pan::LeftToRight => (cx + rx * t, cy + ry / 2.0),
        Pan::RightToLeft => (cx + rx * (1.0 - t), cy + ry / 2.0),
        Pan::Up => (cx + rx / 2.0, cy + ry * (1.0 - t)),
        Pan::Down => (cx + rx / 2.0, cy + ry * t),
    };
    FinishParams { crop: [x0, y0, 1.0 - x0 - vx, 1.0 - y0 - vy], ..FinishParams::default() }
}

/// Size (w, h) of the video for `w × h` frames: the cut frame, scaled down to the
/// chosen resolution (never up), even numbers (as video needs).
pub fn video_size(w: usize, h: usize, s: &Settings) -> (usize, usize) {
    let (cw, ch) = finish::geometry_size(w, h, &frame_crop(w, h, s, 0.0));
    let k = match s.resolution.limit() {
        Some((lw, lh)) => {
            let (lw, lh) = if ch > cw { (lh, lw) } else { (lw, lh) };
            (lw as f64 / cw as f64).min(lh as f64 / ch as f64).min(1.0)
        }
        None => 1.0,
    };
    let even = |v: f64| ((v.round() as usize) & !1).max(2);
    let (w, h) = (even(cw as f64 * k), even(ch as f64 * k));
    // A 16:9 cut scaled down to a standard size gets that size exactly (1920 × 1080, not 1920 × 1078).
    if let Some((lw, lh)) = s.resolution.limit().filter(|_| k < 1.0) {
        let (lw, lh) = if h > w { (lh, lw) } else { (lw, lh) };
        if w.abs_diff(lw) <= 4 && h.abs_diff(lh) <= 4 {
            return (lw, lh);
        }
    }
    (w, h)
}

// --------------------------------------------------------------------------
// The look of a frame
// --------------------------------------------------------------------------

/// The look prepared once for a whole video.
pub struct Look {
    base: Option<FinishParams>,
    look: Option<FinishParams>,
    local: Option<(Local, Gray)>,
    /// The frames are finished (sRGB) images, not linear data.
    encoded: bool,
}

impl Look {
    pub fn new(s: &Settings) -> Self {
        let base = s.base.as_ref().map(|b| FinishParams { quarter: 0, flip_h: false, angle: 0.0, crop: [0.0; 4], long_edge: 0, ..b.clone() });
        let base = base.filter(|b| *b != FinishParams::default());
        let encoded = base.as_ref().is_none_or(|b| b.stretch == finish::StretchMode::Off);
        let look = finish::look_used(&s.look).then(|| finish::look_part(&s.look));
        let local = s.local.used().then(|| (s.local.clone(), finish::soft_mask(&s.local.mask, s.local.feather)));
        Self { base, look, local, encoded }
    }
}

/// The look on a frame already cut and scaled to `img`: the Post-processing
/// settings, the whole-frame look, then sky and foreground. `src` is the size
/// of the frame before cutting, `geo` the cut, `scale` the size of `img`
/// relative to the frame.
pub fn apply_look(img: Img, lk: &Look, src: (usize, usize), geo: &FinishParams, scale: f64, m: &mut Measured) -> Img {
    let mut img = img;
    if let Some(b) = &lk.base {
        img = finish::process(&img, b, scale, m);
    }
    if let Some(l) = &lk.look {
        img = finish::process(&img, l, scale, &mut Measured::default());
    }
    if let Some((local, soft)) = &lk.local {
        let wt = finish::sky_weight(soft, src.0, src.1, geo, img.w, img.h);
        img = finish::apply_local(&img, local, &wt, scale);
    }
    img
}

/// A frame as the video frame of `size` at `t` (0 = start … 1 = end), with the
/// deflicker factors `gains` (from `deflicker`).
fn render(frame: &Img, s: &Settings, lk: &Look, size: (usize, usize), t: f64, gains: Option<&Img>, m: &mut Measured) -> Img {
    let geo = frame_crop(frame.w, frame.h, s, t);
    let c = geo.crop;
    let (fw, fh) = (frame.w as f64, frame.h as f64);
    // Not rounded to whole pixels, so a slow move glides instead of stepping.
    let mut img = crop_resize(frame, c[0] * fw, c[1] * fh, (1.0 - c[0] - c[2]) * fw, (1.0 - c[1] - c[3]) * fh, size.1, size.0);
    if let Some(g) = gains {
        img = apply_gains(img, g, c, lk.encoded);
    }
    apply_look(img, lk, (frame.w, frame.h), &geo, size.0 as f64 / frame.w as f64, m)
}

// --------------------------------------------------------------------------
// Deflicker
// --------------------------------------------------------------------------

/// A frame's deflicker levels (per region and colour) and its grid (columns, rows).
type FrameLevels = (Vec<[f64; 3]>, (usize, usize));

/// Regions across the frame for deflicker "by region".
const REGIONS: usize = 6;

/// The deflicker grid (columns, rows) for a `w × h` frame.
pub fn grid(w: usize, h: usize, regions: bool) -> (usize, usize) {
    if !regions {
        return (1, 1);
    }
    (REGIONS, ((REGIONS as f64 * h as f64 / w.max(1) as f64).round() as usize).max(1))
}

/// Brightness of a frame for deflicker, per colour (or the same for all three
/// without `colour`), on a grid of `gx × gy` regions: the log of the mean linear
/// value, leaving out the darkest and brightest 5 % of each region (stars, lights).
/// With more than one region, the whole frame's levels follow the regions'.
pub fn levels(img: &Img, encoded: bool, gx: usize, gy: usize, colour: bool) -> Vec<[f64; 3]> {
    let long = img.w.max(img.h);
    let small = if long > 480 { resize(img, 480.0 / long as f64) } else { img.clone() };
    let lin = move |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if encoded { srgb_to_linear(v) } else { v }
    };
    let trimmed = |v: &mut Vec<f32>| -> f64 {
        if v.is_empty() {
            return (1e-5f64).ln();
        }
        v.sort_by(|a, b| a.total_cmp(b));
        let (a, b) = (v.len() / 20, v.len() - v.len() / 20);
        let part = &v[a..b.max(a + 1)];
        (part.iter().map(|x| *x as f64).sum::<f64>() / part.len() as f64 + 1e-5).ln()
    };
    let (w, h) = (small.w, small.h);
    let mut out = Vec::with_capacity(gx * gy);
    for ty in 0..gy {
        let (y0, y1) = (ty * h / gy, ((ty + 1) * h / gy).max(ty * h / gy + 1).min(h));
        for tx in 0..gx {
            let (x0, x1) = (tx * w / gx, ((tx + 1) * w / gx).max(tx * w / gx + 1).min(w));
            let mut ch: [Vec<f32>; 3] = Default::default();
            for y in y0..y1 {
                for c in &small.row(y)[x0..x1] {
                    let c = c.map(lin);
                    if colour {
                        for k in 0..3 {
                            ch[k].push(c[k]);
                        }
                    } else {
                        ch[0].push(luminance(c));
                    }
                }
            }
            out.push(if colour {
                [trimmed(&mut ch[0]), trimmed(&mut ch[1]), trimmed(&mut ch[2])]
            } else {
                let l = trimmed(&mut ch[0]);
                [l, l, l]
            });
        }
    }
    if gx * gy > 1 {
        out.extend(levels(&small, encoded, 1, 1, colour));
    }
    out
}

/// Deflicker factors per frame, per region and colour (each a `gy × gx` image),
/// from the levels of every frame (`levels`: the `gx × gy` regions, then the
/// whole frame). Two stages: each colour of the whole frame is evened out
/// (measured on the whole frame, so steadily); then each region only for what
/// it does differently from the whole frame, within narrower limits. (Tested on
/// an all-sky fisheye night: this left less flicker in brightness and colour,
/// overall and in every quadrant, than one stage or smoothing across regions.)
pub fn deflicker(levels: &[Vec<[f64; 3]>], gx: usize, gy: usize, window: u32, mode: Deflicker) -> Vec<Img> {
    let n = levels.len();
    let nt = gx * gy;
    let gi = if nt > 1 { nt } else { 0 }; // the whole frame's levels
    let mut out: Vec<Img> = (0..n).map(|_| Img::filled(gy, gx, [1.0; 3])).collect();
    let mut global = vec![[1.0f32; 3]; n];
    for k in 0..3 {
        let series: Vec<f64> = levels.iter().map(|l| l[gi][k]).collect();
        for (i, g) in deflicker_gains(&series, window, mode).into_iter().enumerate() {
            global[i][k] = g;
        }
    }
    if nt == 1 {
        for (o, g) in out.iter_mut().zip(&global) {
            o.px[0] = *g;
        }
        return out;
    }
    for t in 0..nt {
        for k in 0..3 {
            let rel: Vec<f64> = levels.iter().map(|l| l[t][k] - l[gi][k]).collect();
            for (i, g) in deflicker_gains(&rel, window, mode).into_iter().enumerate() {
                out[i].px[t][k] = global[i][k] * g.clamp(0.67, 1.5);
            }
        }
    }
    out
}

/// The deflicker factors `g` (from `deflicker`, on the whole frame) applied to
/// `img`, the part `crop` of the frame, on linear light.
pub fn apply_gains(img: Img, g: &Img, crop: [f64; 4], encoded: bool) -> Img {
    if g.px.iter().all(|c| c.iter().all(|v| (v - 1.0).abs() < 1e-4)) {
        return img;
    }
    let (w, h) = (img.w as f64, img.h as f64);
    let (cw, ch) = (1.0 - crop[0] - crop[2], 1.0 - crop[1] - crop[3]);
    Img::from_fn(img.h, img.w, |y, x| {
        let gg = if g.px.len() == 1 {
            g.px[0]
        } else {
            let u = crop[0] + (x as f64 + 0.5) / w * cw;
            let v = crop[1] + (y as f64 + 0.5) / h * ch;
            crate::image::bilinear(g, v * g.h as f64 + 0.5, u * g.w as f64 + 0.5)
        };
        let c = img.at(y, x);
        if encoded {
            [0, 1, 2].map(|k| linear_to_srgb((srgb_to_linear(c[k].clamp(0.0, 1.0)) * gg[k]).clamp(0.0, 1.0)))
        } else {
            [0, 1, 2].map(|k| c[k] * gg[k])
        }
    })
}

/// Brightness factors that even out flicker: each frame is brought to the
/// weighted average of its neighbours (`window` frames, centred, so slow changes
/// such as dusk stay; the first and last frames have fewer neighbours).
pub fn deflicker_gains(levels: &[f64], window: u32, mode: Deflicker) -> Vec<f32> {
    let n = levels.len();
    let window = match mode {
        Deflicker::Off => return vec![1.0; n],
        Deflicker::Light => window.max(3) as usize,
        Deflicker::Strong => window.max(3) as usize * 3,
    };
    // Gaussian weights: unlike a plain average they also remove a flicker that alternates frame by frame.
    let half = window / 2;
    let sigma = window as f64 / 4.0;
    (0..n).map(|i| {
        let r = half.min(i).min(n - 1 - i);
        let (mut s, mut ws) = (0.0, 0.0);
        for k in 0..=2 * r {
            let d = k as f64 - r as f64;
            let wt = (-d * d / (2.0 * sigma * sigma)).exp();
            s += wt * levels[i - r + k];
            ws += wt;
        }
        ((s / ws - levels[i]).exp() as f32).clamp(0.25, 4.0)
    }).collect()
}

// --------------------------------------------------------------------------
// Writing
// --------------------------------------------------------------------------

/// ffmpeg: PANOSTITCH_FFMPEG, next to this program, on the PATH, or in the usual places.
pub fn find_ffmpeg() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Some(p) = std::env::var_os("PANOSTITCH_FFMPEG").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())) {
        dirs.push(d.join("ffmpeg").join("bin"));
        dirs.push(d.join("ffmpeg"));
        dirs.push(d);
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if cfg!(target_os = "macos") {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    }
    if cfg!(windows) {
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(l).join("Microsoft").join("WinGet").join("Links"));
        }
        dirs.push(PathBuf::from(r"C:\ffmpeg\bin"));
    }
    dirs.into_iter().map(|d| d.join(exe)).find(|p| p.is_file())
}

/// Where the frames go.
enum Sink {
    Ffmpeg { child: std::process::Child, stdin: Option<std::process::ChildStdin>, err: Option<std::thread::JoinHandle<String>>, out: String },
    Gif { enc: Box<image::codecs::gif::GifEncoder<std::io::BufWriter<std::fs::File>>>, delay: image::Delay, out: String },
    Seq { stem: PathBuf, ext: String, quality: u8, written: Vec<String> },
}

impl Sink {
    fn open(s: &Settings, ffmpeg: Option<&Path>, w: usize, h: usize) -> Result<Sink, String> {
        let out = s.out.trim().to_string();
        match s.format {
            Format::Mp4 | Format::WebM => {
                let ff = ffmpeg.ok_or(NO_FFMPEG)?;
                let q = s.quality.clamp(30, 100) as f64;
                let mut cmd = std::process::Command::new(ff);
                cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24"]);
                cmd.args(["-s", &format!("{w}x{h}"), "-framerate", &s.fps.to_string(), "-i", "-"]);
                cmd.args(["-vf", "scale=out_color_matrix=bt709:out_range=tv", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709"]);
                if s.format == Format::Mp4 {
                    let crf = (40.0 - 0.25 * q).round().to_string();
                    cmd.args(["-c:v", "libx264", "-preset", "medium", "-crf", &crf, "-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
                } else {
                    let crf = (60.0 - 0.4 * q).round().to_string();
                    cmd.args(["-c:v", "libvpx-vp9", "-crf", &crf, "-b:v", "0", "-row-mt", "1", "-pix_fmt", "yuv420p"]);
                }
                cmd.arg(&out);
                cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x0800_0000); // no console window
                }
                let mut child = cmd.spawn().map_err(|e| format!("Cannot start ffmpeg ({}): {e}", ff.display()))?;
                let stdin = child.stdin.take();
                let err = child.stderr.take().map(|mut e| {
                    std::thread::spawn(move || {
                        let mut s = String::new();
                        let _ = e.read_to_string(&mut s);
                        s
                    })
                });
                Ok(Sink::Ffmpeg { child, stdin, err, out })
            }
            Format::Gif => {
                let f = std::fs::File::create(&out).map_err(|e| format!("Cannot write {}: {e}", basename(&out)))?;
                let mut enc = image::codecs::gif::GifEncoder::new_with_speed(std::io::BufWriter::new(f), 10);
                enc.set_repeat(image::codecs::gif::Repeat::Infinite).map_err(|e| e.to_string())?;
                Ok(Sink::Gif { enc: Box::new(enc), delay: image::Delay::from_numer_denom_ms(1000, s.fps.max(1)), out })
            }
            Format::Sequence => {
                let p = Path::new(&out);
                let ext = match io::ext_of(&out).as_str() {
                    e @ ("tif" | "tiff" | "png" | "jpg" | "jpeg") => e.to_string(),
                    _ => "tif".into(),
                };
                let stem = p.with_extension("");
                Ok(Sink::Seq { stem, ext, quality: s.quality.clamp(30, 100) as u8, written: Vec::new() })
            }
        }
    }

    fn push(&mut self, img: &Img) -> Result<(), String> {
        match self {
            Sink::Ffmpeg { stdin, .. } => {
                let buf = io::to_rgb8(img);
                stdin.as_mut().ok_or("ffmpeg is not running")?.write_all(&buf).map_err(|_| "ffmpeg stopped".to_string())
            }
            Sink::Gif { enc, delay, .. } => {
                let mut rgba = Vec::with_capacity(img.len() * 4);
                for c in io::to_rgb8(img).chunks_exact(3) {
                    rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
                }
                let buf = image::RgbaImage::from_raw(img.w as u32, img.h as u32, rgba).ok_or("bad frame")?;
                enc.encode_frame(image::Frame::from_parts(buf, 0, 0, *delay)).map_err(|e| format!("GIF: {e}"))
            }
            Sink::Seq { stem, ext, quality, written } => {
                let name = format!("{}_{:04}.{ext}", stem.to_string_lossy(), written.len() + 1);
                io::save_image(&name, img, *quality)?;
                written.push(name);
                Ok(())
            }
        }
    }

    /// Close the output; the number of files written.
    fn finish(self) -> Result<usize, String> {
        match self {
            Sink::Ffmpeg { mut child, stdin, err, .. } => {
                drop(stdin);
                let st = child.wait().map_err(|e| e.to_string())?;
                let msg = err.and_then(|h| h.join().ok()).unwrap_or_default();
                if st.success() {
                    Ok(1)
                } else {
                    Err(format!("ffmpeg failed: {}", msg.trim().lines().last().unwrap_or("no message")))
                }
            }
            Sink::Gif { enc, .. } => {
                drop(enc);
                Ok(1)
            }
            Sink::Seq { written, .. } => Ok(written.len()),
        }
    }

    /// Stop and remove what was written.
    fn abort(self) -> String {
        match self {
            Sink::Ffmpeg { mut child, stdin, err, out } => {
                drop(stdin);
                let _ = child.kill();
                let _ = child.wait();
                let msg = err.and_then(|h| h.join().ok()).unwrap_or_default();
                let _ = std::fs::remove_file(&out);
                msg.trim().lines().last().unwrap_or("").to_string()
            }
            Sink::Gif { enc, out, .. } => {
                drop(enc);
                let _ = std::fs::remove_file(&out);
                String::new()
            }
            Sink::Seq { written, .. } => {
                for f in written {
                    let _ = std::fs::remove_file(f);
                }
                String::new()
            }
        }
    }
}

// --------------------------------------------------------------------------
// Making the video
// --------------------------------------------------------------------------

/// Make the video. `progress(message, done, total)` is called as it goes; an
/// error from it stops the work (cancel) and removes what was written.
pub fn make(s: &Settings, progress: &(dyn Fn(&str, f64, f64) -> Result<(), String> + Sync)) -> Result<Done, String> {
    let n = s.frames.len();
    if n == 0 {
        return Err("No frames: add them first.".into());
    }
    let out = s.out.trim();
    if out.is_empty() {
        return Err("Choose where to save the video.".into());
    }
    crate::sys::check_writable(out)?;
    let ffmpeg = if s.format.needs_ffmpeg() { Some(find_ffmpeg().ok_or(NO_FFMPEG)?) } else { None };
    // RAW at half size (about 3000 px wide) is plenty for 1080p and quicker; larger videos need the full size.
    let opt = io::LoadOptions { decoder: s.decoder, half: s.resolution.limit().is_some_and(|(lw, _)| lw <= 1920), ..Default::default() };
    let load = |i: usize| io::load_any(&s.frames[i], &opt).map_err(|e| format!("Frame {} ({}): {e}", i + 1, basename(&s.frames[i])));
    let lk = Look::new(s);
    let measuring = s.deflicker != Deflicker::Off && n > 2;
    let total = if measuring { 2 * n } else { n } as f64;
    let mut done = 0.0;

    // Pass 1: the brightness and colour of every frame, region by region (deflicker).
    let gains: Option<Vec<Img>> = if measuring {
        let mut lv: Vec<Vec<[f64; 3]>> = vec![Vec::new(); n];
        let mut g = (1, 1);
        let idx: Vec<usize> = (0..n).collect();
        for chunk in idx.chunks(PAR) {
            progress(&format!("Measuring brightness and colour: frame {} of {n}", chunk[0] + 1), done, total)?;
            let r: Vec<Result<FrameLevels, String>> = chunk.par_iter().map(|&i| {
                load(i).map(|f| {
                    let (gx, gy) = grid(f.w, f.h, s.deflicker_regions);
                    (levels(&f, lk.encoded, gx, gy, s.deflicker_colour), (gx, gy))
                })
            }).collect();
            for (&i, v) in chunk.iter().zip(r) {
                let (l, gg) = v?;
                if i == 0 {
                    g = gg;
                }
                if gg != g {
                    return Err(format!("Frame {} ({}) has a different shape from the first frame.", i + 1, basename(&s.frames[i])));
                }
                lv[i] = l;
            }
            done += chunk.len() as f64;
        }
        Some(deflicker(&lv, g.0, g.1, s.deflicker_window, s.deflicker))
    } else {
        None
    };
    let gain = |i: usize| gains.as_ref().map(|g| &g[i]);

    // Pass 2: every frame, in order.
    progress(&format!("Frame 1 of {n}"), done, total)?;
    let t_of = |i: usize| if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 };
    let first = load(0)?;
    let size = video_size(first.w, first.h, s);
    // Values measured on the first frame (automatic stretch) serve all, so the video does not flicker.
    let mut m = Measured::default();
    let f0 = render(&first, s, &lk, size, 0.0, gain(0), &mut m);
    drop(first);
    let mut sink = Sink::open(s, ffmpeg.as_deref(), size.0, size.1)?;
    let mut written = 0usize;
    let r = (|| -> Result<(), String> {
        let dir = Path::new(out).parent().filter(|d| !d.as_os_str().is_empty()).map(|d| d.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        let mut back = if s.bounce && n > 2 { Some(tempfile::tempfile_in(&dir).map_err(|e| format!("Cannot write a temporary file in {}: {e}", dir.display()))?) } else { None };
        let mut last: Option<Img> = None;
        let mut emit = |img: Img, sink: &mut Sink, written: &mut usize| -> Result<(), String> {
            sink.push(&img)?;
            *written += 1;
            if let Some(f) = back.as_mut() {
                f.write_all(&io::to_rgb8(&img)).map_err(|e| format!("Temporary file: {e}"))?;
            }
            last = Some(img);
            Ok(())
        };
        emit(f0, &mut sink, &mut written)?;
        done += 1.0;
        let idx: Vec<usize> = (1..n).collect();
        for chunk in idx.chunks(PAR) {
            progress(&format!("Frame {} of {n}", chunk[0] + 1), done, total)?;
            let r: Vec<Result<Img, String>> = chunk.par_iter().map(|&i| {
                let mut mm = m;
                load(i).map(|f| render(&f, s, &lk, size, t_of(i), gain(i), &mut mm))
            }).collect();
            for img in r {
                emit(img?, &mut sink, &mut written)?;
            }
            done += chunk.len() as f64;
        }
        // The last frame held, then the way back.
        if let Some(last) = &last {
            for _ in 0..(s.hold_last.max(0.0) * s.fps as f64).round() as usize {
                sink.push(last)?;
                written += 1;
            }
        }
        if let Some(mut f) = back {
            progress("Playing back", done, total)?;
            let (w, h) = size;
            let bytes = w * h * 3;
            let mut buf = vec![0u8; bytes];
            for i in (1..n - 1).rev() {
                f.seek(SeekFrom::Start((i * bytes) as u64)).map_err(|e| e.to_string())?;
                f.read_exact(&mut buf).map_err(|e| format!("Temporary file: {e}"))?;
                let img = Img::from_vec(h, w, buf.chunks_exact(3).map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]).collect());
                sink.push(&img)?;
                written += 1;
            }
        }
        progress("Finishing the file", done, total)?;
        Ok(())
    })();
    match r {
        Ok(()) => {
            let files = sink.finish()?;
            Ok(Done { frames: n, video_frames: written, w: size.0, h: size.1, out: out.to_string(), files })
        }
        Err(e) => {
            let why = sink.abort();
            Err(if why.is_empty() || e != "ffmpeg stopped" { e } else { format!("ffmpeg stopped: {why}") })
        }
    }
}

/// The look on a frame preview (no cut, no zoom, no deflicker): what the tab shows.
/// `scale` is the preview's size relative to the frame.
pub fn preview(frame: &Img, s: &Settings, scale: f64) -> Img {
    let lk = Look::new(s);
    let geo = FinishParams::default();
    apply_look(frame.clone(), &lk, (frame.w, frame.h), &geo, scale, &mut Measured::default())
}

//! Camera RAW decoding.
//!
//! The built-in decoder (the `rawler` crate, pure Rust) handles CR2, CR3, NEF,
//! ARW, DNG, RAF, ORF, RW2, PEF and more without any external program:
//! black / white levels, demosaicing (PPG), the camera's colour matrix and
//! white balance, no auto-brightening, so a panorama's frames keep their
//! relative exposure. The other decoders are optional programs the user may
//! have installed, each writing a 16-bit TIFF into a private temporary folder
//! (never next to the original, which may be on a read-only card):
//!   libraw       LibRaw's dcraw_emu
//!   rawtherapee  rawtherapee-cli: applies a <photo>.pp3 sidecar if present
//!   darktable    darktable-cli with its default pipeline, slowest
//!   dcraw        classic dcraw: no CR3 support
//! `Auto` tries them in that order (RawTherapee first if the photo has a .pp3
//! sidecar) and uses the first that succeeds.

use super::{basename, from_dynamic, load_plain, LoadOptions};
use crate::image::{block_average, resize, Img};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RawDecoder {
    Auto,
    Builtin,
    LibRaw,
    RawTherapee,
    Darktable,
    Dcraw,
}

impl RawDecoder {
    pub const ALL: [RawDecoder; 6] =
        [Self::Auto, Self::Builtin, Self::LibRaw, Self::RawTherapee, Self::Darktable, Self::Dcraw];
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Builtin => "built-in",
            Self::LibRaw => "libraw",
            Self::RawTherapee => "rawtherapee",
            Self::Darktable => "darktable",
            Self::Dcraw => "dcraw",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|d| d.name() == s)
    }
    pub fn exe(self) -> &'static str {
        match self {
            Self::LibRaw => "dcraw_emu",
            Self::RawTherapee => "rawtherapee-cli",
            Self::Darktable => "darktable-cli",
            Self::Dcraw => "dcraw",
            _ => "",
        }
    }
    /// Can skip the tone curve and choose white balance / demosaicing.
    fn linear_capable(self) -> bool {
        matches!(self, Self::Builtin | Self::LibRaw | Self::Dcraw)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteBalance {
    Camera,
    Auto,
    Daylight,
}

impl WhiteBalance {
    pub fn name(self) -> &'static str {
        match self {
            Self::Camera => "camera",
            Self::Auto => "auto",
            Self::Daylight => "daylight",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        [Self::Camera, Self::Auto, Self::Daylight].into_iter().find(|w| w.name() == s)
    }
}

/// Full path of a program on PATH, if any.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        if cfg!(windows) {
            let p = dir.join(format!("{name}.exe"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// RAW decoders that can be used, in the order `Auto` tries them.
pub fn available_raw_decoders() -> Vec<RawDecoder> {
    let mut v = vec![RawDecoder::Builtin];
    for d in [RawDecoder::LibRaw, RawDecoder::RawTherapee, RawDecoder::Darktable, RawDecoder::Dcraw] {
        if which(d.exe()).is_some() {
            v.push(d);
        }
    }
    v
}

/// Decode a RAW file with the chosen decoder (or the first that works).
pub fn load_raw(path: &str, opt: &LoadOptions) -> Result<Img, String> {
    if !Path::new(path).is_file() {
        return Err(format!("File not found: {path}"));
    }
    let mut found = available_raw_decoders();
    if opt.linear || opt.wb != WhiteBalance::Camera || opt.quality.is_some() {
        found.retain(|d| d.linear_capable());
    }
    let tries: Vec<RawDecoder> = if opt.decoder == RawDecoder::Auto {
        if super::ext_of(path) == "cr3" {
            found.retain(|d| *d != RawDecoder::Dcraw);
        }
        // A RawTherapee sidecar means the photo was edited there: honour it first.
        if Path::new(&format!("{path}.pp3")).is_file() && found.contains(&RawDecoder::RawTherapee) {
            found.retain(|d| *d != RawDecoder::RawTherapee);
            found.insert(0, RawDecoder::RawTherapee);
        }
        found
    } else {
        if !found.contains(&opt.decoder) {
            return Err(format!("RAW decoder {} ({}) is not available.", opt.decoder.name(), opt.decoder.exe()));
        }
        vec![opt.decoder]
    };
    let mut errs = Vec::new();
    for d in tries {
        match decode_with(d, path, opt) {
            Ok(img) => return Ok(img),
            Err(e) => errs.push(format!("  {}: {e}", d.name())),
        }
    }
    Err(format!("Could not decode {}:\n{}", basename(path), errs.join("\n")))
}

fn decode_with(d: RawDecoder, path: &str, opt: &LoadOptions) -> Result<Img, String> {
    if d == RawDecoder::Builtin {
        return decode_builtin(path, opt);
    }
    let dir = tempfile::Builder::new().prefix("panostitch-raw-").tempdir().map_err(|e| e.to_string())?;
    let out = dir.path().join("decoded.tif");
    let mut cmd = raw_cmd(d, path, &out, dir.path(), opt);
    let output = cmd.output().map_err(|e| format!("{} failed to start: {e}", d.exe()))?;
    if !output.status.success() {
        let mut msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if msg.is_empty() {
            msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        let tail: String = msg.chars().rev().take(300).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("{} failed{}", d.exe(), if tail.is_empty() { String::new() } else { format!(": {tail}") }));
    }
    if d == RawDecoder::Dcraw {
        // dcraw writes to stdout
        std::fs::write(&out, &output.stdout).map_err(|e| e.to_string())?;
    }
    if !out.is_file() {
        return Err(format!("{} produced no output", d.exe()));
    }
    let mut img = load_plain(out.to_str().unwrap())?;
    // Only LibRaw and dcraw can decode at half size themselves.
    if opt.half && matches!(d, RawDecoder::RawTherapee | RawDecoder::Darktable) {
        img = resize(&img, 0.5);
    }
    Ok(img)
}

fn raw_cmd(d: RawDecoder, path: &str, out: &Path, dir: &Path, opt: &LoadOptions) -> Command {
    // -w camera WB (-a average, none = daylight), -W no auto-brighten, -6 16-bit,
    // -g 1 1 linear, -q demosaicing, -h half size, -T TIFF, -Z output file
    let mut opts: Vec<String> = Vec::new();
    match opt.wb {
        WhiteBalance::Camera => opts.push("-w".into()),
        WhiteBalance::Auto => opts.push("-a".into()),
        WhiteBalance::Daylight => {}
    }
    opts.push("-W".into());
    opts.push("-6".into());
    if opt.linear {
        opts.extend(["-g".into(), "1".into(), "1".into()]);
    }
    if let Some(q) = opt.quality {
        opts.extend(["-q".into(), q.to_string()]);
    }
    if opt.half {
        opts.push("-h".into());
    }
    if !opt.orient {
        opts.extend(["-t".into(), "0".into()]); // no rotation
    }
    match d {
        RawDecoder::LibRaw => {
            let mut c = Command::new("dcraw_emu");
            c.args(&opts).arg("-T").arg("-Z").arg(out).arg(path);
            c
        }
        RawDecoder::Dcraw => {
            let mut c = Command::new("dcraw");
            c.arg("-c").args(&opts).arg("-T").arg(path);
            c
        }
        RawDecoder::RawTherapee => {
            // -s apply the photo's .pp3 sidecar if present, else -d the default
            // profile; -t TIFF, -b16 16-bit, -Y overwrite, -c input (last)
            let mut c = Command::new("rawtherapee-cli");
            c.args(["-q", "-s", "-d", "-t", "-b16", "-Y", "-o"]).arg(out).arg("-c").arg(path);
            c
        }
        _ => {
            // A private config dir avoids clashing with a running darktable.
            let mut c = Command::new("darktable-cli");
            c.arg(path)
                .arg(out)
                .args(["--hq", "true", "--apply-custom-presets", "false", "--core", "--configdir"])
                .arg(dir.join("dtconf"))
                .args(["--conf", "plugins/imageio/format/tiff/bpp=16"]);
            c
        }
    }
}

// --------------------------------------------------------------------------
// Built-in decoder (rawler)
// --------------------------------------------------------------------------

fn decode_builtin(path: &str, opt: &LoadOptions) -> Result<Img, String> {
    let opt = opt.clone();
    let p = path.to_string();
    // rawler may panic on unusual sensors; report that as an error instead.
    match std::panic::catch_unwind(move || decode_builtin_inner(&p, &opt)) {
        Ok(r) => r,
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unsupported file".into());
            Err(format!("built-in decoder: {msg}"))
        }
    }
}

fn decode_builtin_inner(path: &str, opt: &LoadOptions) -> Result<Img, String> {
    use rawler::decoders::Orientation;

    let src = rawler::rawsource::RawSource::new(Path::new(path)).map_err(|e| e.to_string())?;
    let dec = rawler::get_decoder(&src).map_err(|e| e.to_string())?;
    let params = rawler::decoders::RawDecodeParams { image_index: 0 };
    let raw = dec.raw_image(&src, &params, false).map_err(|e| e.to_string())?;
    // The orientation may sit in the EXIF data rather than in the raw image itself.
    let mut orientation = raw.orientation;
    if matches!(orientation, Orientation::Normal | Orientation::Unknown) {
        if let Some(o) = dec.raw_metadata(&src, &params).ok().and_then(|m| m.exif.orientation) {
            orientation = Orientation::from_u16(o);
        }
    }
    develop(raw, orientation, opt)
}

/// Sensor data → RGB image (black / white levels, white balance with clipped
/// highlights, demosaicing, camera colours, orientation, half size, tone).
#[doc(hidden)]
pub fn develop(mut raw: rawler::RawImage, orientation: rawler::decoders::Orientation, opt: &LoadOptions) -> Result<Img, String> {
    use rawler::decoders::Orientation;
    use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
    use rawler::rawimage::RawPhotometricInterpretation;
    raw.apply_scaling().map_err(|e| e.to_string())?;
    match opt.wb {
        WhiteBalance::Camera => {
            if raw.wb_coeffs[0].is_nan() || raw.wb_coeffs[0] <= 0.0 {
                raw.wb_coeffs = raw.neutralwb();
            }
        }
        WhiteBalance::Daylight => raw.wb_coeffs = raw.neutralwb(),
        WhiteBalance::Auto => {
            // Grey world: the average of each colour of the mosaic becomes equal.
            if let RawPhotometricInterpretation::Cfa(cfg) = &raw.photometric {
                let data = raw.data.as_f32();
                let (w, h) = (raw.width, raw.height);
                let mut sum = [0f64; 4];
                let mut cnt = [0f64; 4];
                let step = ((w * h) / 2_000_000).max(1);
                let mut i = 0;
                while i < w * h {
                    let c = cfg.cfa.color_at(i / w, i % w).min(3);
                    sum[c] += data[i] as f64;
                    cnt[c] += 1.0;
                    i += step;
                }
                let m: Vec<f64> = (0..4).map(|c| if cnt[c] > 0.0 { sum[c] / cnt[c] } else { 0.0 }).collect();
                if m[1] > 0.0 {
                    let g = m[1];
                    raw.wb_coeffs = [
                        if m[0] > 0.0 { (g / m[0]) as f32 } else { 1.0 },
                        1.0,
                        if m[2] > 0.0 { (g / m[2]) as f32 } else { 1.0 },
                        if m[3] > 0.0 { (g / m[3]) as f32 } else { 1.0 },
                    ];
                }
            }
        }
    }
    // White balance on the sensor data, with highlights clipped (as dcraw / LibRaw
    // do): the factors are scaled so the smallest is 1, and every channel is cut
    // at the sensor's saturation. Otherwise blown areas (the sun, lamps) turn
    // pink, since only the clipped channels get multiplied up.
    {
        let wb = raw.wb_coeffs;
        let wb = if wb.iter().take(3).all(|v| v.is_finite() && *v > 0.0) { wb } else { [1.0; 4] };
        let m = wb.iter().take(3).cloned().fold(f32::MAX, f32::min);
        let k: Vec<f32> = (0..4).map(|c| if wb[c].is_finite() && wb[c] > 0.0 { wb[c] / m } else { 1.0 }).collect();
        let (w, cpp) = (raw.width, raw.cpp);
        let cfa = match &raw.photometric {
            RawPhotometricInterpretation::Cfa(cfg) => Some(cfg.cfa.clone()),
            _ => None,
        };
        let mut data = raw.data.as_f32().into_owned();
        match (&cfa, cpp) {
            (Some(cfa), 1) => data.iter_mut().enumerate().for_each(|(i, v)| *v = (*v * k[cfa.color_at(i / w, i % w).min(3)]).min(1.0)),
            _ => data.iter_mut().enumerate().for_each(|(i, v)| *v = (*v * k[(i % cpp).min(3)]).min(1.0)),
        }
        raw.data = rawler::RawImageData::Float(data);
        raw.wb_coeffs = [1.0; 4];
    }
    let dev = RawDevelop::new_with(&[
        ProcessingStep::Demosaic,
        ProcessingStep::FujiRotate,
        ProcessingStep::CropActiveArea,
        ProcessingStep::Calibrate,
        ProcessingStep::CropDefault,
    ]);
    let inter = dev.develop_intermediate(&raw).map_err(|e| e.to_string())?;
    let mut img = match inter {
        Intermediate::ThreeColor(p) => Img::from_vec(p.height, p.width, p.data),
        Intermediate::Monochrome(p) => Img::from_vec(p.height, p.width, p.data.iter().map(|&v| [v, v, v]).collect()),
        Intermediate::FourColor(p) => {
            Img::from_vec(p.height, p.width, p.data.iter().map(|v| [v[0], v[1], v[2]]).collect())
        }
    };
    img.map_inplace(|p| [p[0].max(0.0), p[1].max(0.0), p[2].max(0.0)]);
    let orientation = if opt.orient { orientation } else { Orientation::Normal };
    img = match orientation {
        Orientation::Normal | Orientation::Unknown => img,
        Orientation::HorizontalFlip => img.flip_cols(),
        Orientation::Rotate180 => img.rotate180(),
        Orientation::VerticalFlip => img.flip_rows(),
        Orientation::Transpose => img.transpose(),
        Orientation::Rotate90 => img.transpose().flip_cols(),
        Orientation::Transverse => img.transpose().rotate180(),
        Orientation::Rotate270 => img.transpose().flip_rows(),
    };
    if opt.half {
        img = block_average(&img, 2);
    }
    if !opt.linear {
        img.map_inplace(|p| [srgb(p[0]), srgb(p[1]), srgb(p[2])]);
    }
    Ok(img)
}

#[inline]
fn srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[allow(dead_code)]
fn _unused(d: image::DynamicImage) -> Img {
    from_dynamic(d)
}

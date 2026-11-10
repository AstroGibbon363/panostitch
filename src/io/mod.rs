//! Loading and saving images: JPEG, PNG, TIFF, BMP (the `image` crate), camera
//! RAW (`raw`), FITS (`fits`) and XISF (`xisf`).

pub mod fits;
pub mod raw;
pub mod xisf;

use crate::image::{Img, Rgb};
use std::path::Path;

pub use raw::{available_raw_decoders, RawDecoder, WhiteBalance};

pub const RAW_EXTS: &[&str] = &[
    "cr2", "cr3", "nef", "nrw", "arw", "srf", "dng", "raf", "orf", "rw2", "pef", "srw", "raw", "3fr", "iiq",
];
pub const FITS_EXTS: &[&str] = &["fits", "fit", "fts"];
pub const XISF_EXTS: &[&str] = &["xisf"];
pub const PLAIN_EXTS: &[&str] = &["jpg", "jpeg", "png", "tif", "tiff", "bmp"];

/// Lower-case extension of a path, without the dot.
pub fn ext_of(path: &str) -> String {
    Path::new(path).extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default()
}

pub fn is_raw(path: &str) -> bool {
    RAW_EXTS.contains(&ext_of(path).as_str())
}
pub fn is_fits(path: &str) -> bool {
    FITS_EXTS.contains(&ext_of(path).as_str())
}
pub fn is_xisf(path: &str) -> bool {
    XISF_EXTS.contains(&ext_of(path).as_str())
}
/// FITS or XISF: astronomy formats, saved as linear 32-bit float.
pub fn is_astro(path: &str) -> bool {
    is_fits(path) || is_xisf(path)
}
pub fn is_image(path: &str) -> bool {
    let e = ext_of(path);
    let e = e.as_str();
    RAW_EXTS.contains(&e) || FITS_EXTS.contains(&e) || XISF_EXTS.contains(&e) || PLAIN_EXTS.contains(&e)
}
/// All image extensions (with the dot), sorted.
pub fn image_exts() -> Vec<String> {
    let mut v: Vec<String> =
        RAW_EXTS.iter().chain(FITS_EXTS).chain(XISF_EXTS).chain(PLAIN_EXTS).map(|e| format!(".{e}")).collect();
    v.sort();
    v
}

/// Options for loading a RAW file (ignored for the other formats).
#[derive(Clone, Debug)]
pub struct LoadOptions {
    pub decoder: RawDecoder,
    /// Decode RAW files and colour-camera (Bayer) FITS / XISF files at half size.
    pub half: bool,
    /// Keep RAW data linear (no tone curve); for stacking.
    pub linear: bool,
    pub wb: WhiteBalance,
    /// Demosaicing for dcraw-style decoders (0 bilinear, 1 VNG, 2 PPG, 3 AHD, …); None = default.
    pub quality: Option<u32>,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self { decoder: RawDecoder::Auto, half: false, linear: false, wb: WhiteBalance::Camera, quality: None }
    }
}

/// Load RAW, FITS, XISF, JPEG, PNG, TIFF or BMP into an RGB float image (0…1 nominal).
pub fn load_any(path: &str, opt: &LoadOptions) -> Result<Img, String> {
    let e = ext_of(path);
    let e = e.as_str();
    if FITS_EXTS.contains(&e) {
        return fits::read_fits(path, opt.half);
    }
    if XISF_EXTS.contains(&e) {
        return xisf::read_xisf(path, opt.half);
    }
    if RAW_EXTS.contains(&e) {
        return raw::load_raw(path, opt);
    }
    load_plain(path)
}

/// JPEG, PNG, TIFF, BMP through the `image` crate (8 or 16 bit → 0…1).
pub fn load_plain(path: &str) -> Result<Img, String> {
    let mut reader = image::ImageReader::open(path).map_err(|e| format!("{}: {e}", basename(path)))?;
    reader.no_limits();
    let reader = reader.with_guessed_format().map_err(|e| format!("{}: {e}", basename(path)))?;
    let dyn_img = reader.decode().map_err(|e| format!("{}: {e}", basename(path)))?;
    Ok(from_dynamic(dyn_img))
}

pub fn from_dynamic(dyn_img: image::DynamicImage) -> Img {
    let (w, h) = (dyn_img.width() as usize, dyn_img.height() as usize);
    let f = dyn_img.into_rgb32f();
    let v = f.into_raw();
    let px: Vec<Rgb> = v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    Img::from_vec(h, w, px)
}

pub fn basename(path: &str) -> String {
    Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or(path).to_string()
}

/// Save with a sensible bit depth: 16-bit for TIFF/PNG, 8-bit for JPEG/BMP, and
/// 32-bit float FITS / XISF (values kept as they are, not clipped).
pub fn save_panorama(path: &str, img: &Img) -> Result<(), String> {
    if is_fits(path) {
        return fits::write_fits(path, img);
    }
    if is_xisf(path) {
        return xisf::write_xisf(path, img);
    }
    let c01 = |x: f32| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
    let e = ext_of(path);
    let (w, h) = (img.w as u32, img.h as u32);
    let res = match e.as_str() {
        "tif" | "tiff" | "png" => {
            let mut buf: Vec<u16> = Vec::with_capacity(img.len() * 3);
            for p in &img.px {
                for c in p {
                    buf.push((c01(*c) * 65535.0).round() as u16);
                }
            }
            let im: image::ImageBuffer<image::Rgb<u16>, Vec<u16>> =
                image::ImageBuffer::from_raw(w, h, buf).ok_or("bad buffer")?;
            im.save(path)
        }
        "jpg" | "jpeg" => {
            let buf = to_rgb8(img);
            let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            let mut wr = std::io::BufWriter::new(file);
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut wr, 95);
            use image::ImageEncoder;
            enc.write_image(&buf, w, h, image::ExtendedColorType::Rgb8)
        }
        _ => {
            let buf = to_rgb8(img);
            let im: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
                image::ImageBuffer::from_raw(w, h, buf).ok_or("bad buffer")?;
            im.save(path)
        }
    };
    res.map_err(|e| format!("Cannot save {}: {e}", basename(path)))
}

/// 8-bit RGB bytes (clamped to 0…1).
pub fn to_rgb8(img: &Img) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(img.len() * 3);
    for p in &img.px {
        for c in p {
            let v = if c.is_nan() { 0.0 } else { c.clamp(0.0, 1.0) };
            buf.push((v * 255.0).round() as u8);
        }
    }
    buf
}

/// Colour image from a Bayer mosaic `m` (rows top first) with pattern `pat`
/// (e.g. "RGGB"). `half` makes one colour pixel per 2×2 cell; otherwise each
/// colour is interpolated bilinearly (3×3, centre weighted).
pub fn debayer(m: &crate::image::Gray, pat: &str, half: bool) -> Img {
    let (h, w) = (m.h, m.w);
    let pb = pat.as_bytes();
    let chan = |r: usize, c: usize| -> usize {
        match pb[2 * (r % 2) + (c % 2)] {
            b'R' => 0,
            b'G' => 1,
            _ => 2,
        }
    };
    if half {
        let (hh, ww) = (h / 2, w / 2);
        return Img::from_fn(hh, ww, |i, j| {
            let mut v = [0f32; 3];
            for r in 0..2 {
                for c in 0..2 {
                    v[chan(r, c)] += m.at(2 * i + r, 2 * j + c);
                }
            }
            [v[0], v[1] / 2.0, v[2]]
        });
    }
    let wts = [1f32, 2.0, 1.0];
    Img::from_fn(h, w, |i, j| {
        let mut acc = [0f32; 3];
        let mut cnt = [0f32; 3];
        let own = chan(i, j);
        for di in -1i64..=1 {
            for dj in -1i64..=1 {
                let (ii, jj) = (i as i64 + di, j as i64 + dj);
                if ii < 0 || jj < 0 || ii >= h as i64 || jj >= w as i64 {
                    continue;
                }
                let ch = chan(ii as usize, jj as usize);
                let wgt = wts[(di + 1) as usize] * wts[(dj + 1) as usize];
                acc[ch] += wgt * m.at(ii as usize, jj as usize);
                cnt[ch] += wgt;
            }
        }
        let val = |ch: usize| if ch == own { m.at(i, j) } else { acc[ch] / cnt[ch].max(1e-6) };
        [val(0), val(1), val(2)]
    })
}

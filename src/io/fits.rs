//! FITS (astronomy images).
//!
//! Reads the first image of a FITS file: 8/16/32/64-bit integer or 32/64-bit
//! float, mono (NAXIS = 2) or RGB planes (NAXIS3 = 3). Mono images with a Bayer
//! pattern (BAYERPAT, from colour astro cameras) are debayered. ROWORDER is
//! honoured (FITS default: bottom row first). Values are scaled to about 0…1.
//! Tile-compressed files (.fz) are not supported. Writes 32-bit float RGB.

use super::{basename, debayer};
use crate::image::{quantile, Gray, Img};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl Value {
    fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
    fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }
    fn as_str(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Bool(b) => if *b { "T".into() } else { "F".into() },
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
        }
    }
}

/// Parse the value of a FITS header card (after "= ").
fn fits_value(v: &str) -> Value {
    let v = v.trim();
    if let Some(rest) = v.strip_prefix('\'') {
        let mut out = String::new();
        let chars: Vec<char> = rest.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\'' {
                if i + 1 < chars.len() && chars[i + 1] == '\'' {
                    out.push('\'');
                    i += 2;
                    continue;
                }
                break;
            }
            out.push(chars[i]);
            i += 1;
        }
        return Value::Str(out.trim_end().to_string());
    }
    let v = v.split('/').next().unwrap_or("").trim();
    if v == "T" {
        return Value::Bool(true);
    }
    if v == "F" {
        return Value::Bool(false);
    }
    if let Ok(x) = v.parse::<i64>() {
        return Value::Int(x);
    }
    if let Ok(x) = v.replace('D', "E").parse::<f64>() {
        return Value::Float(x);
    }
    Value::Str(v.to_string())
}

type Header = HashMap<String, Value>;

/// Header of the HDU starting at byte `pos`: (cards, start of its data).
fn fits_header(bytes: &[u8], mut pos: usize) -> Result<(Header, usize), String> {
    let mut hdr = Header::new();
    loop {
        if pos + 2880 > bytes.len() {
            return Err("Truncated FITS header".into());
        }
        let block = &bytes[pos..pos + 2880];
        pos += 2880;
        for k in 0..36 {
            let card = String::from_utf8_lossy(&block[80 * k..80 * k + 80]).to_string();
            let key = card[..8].trim().to_string();
            if key == "END" {
                return Ok((hdr, pos));
            }
            if &card[8..10] == "= " {
                hdr.insert(key, fits_value(&card[10..]));
            }
        }
    }
}

fn bytes_per(bitpix: i64) -> Result<usize, String> {
    Ok(match bitpix {
        8 => 1,
        16 | -16 => 2,
        32 | -32 => 4,
        64 | -64 => 8,
        _ => return Err(format!("Unsupported BITPIX {bitpix}")),
    })
}

/// Read the first image of a FITS file as RGB (see the module comment).
pub fn read_fits(path: &str, half: bool) -> Result<Img, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", basename(path)))?;
    let mut pos = 0usize;
    loop {
        let (hdr, data) = fits_header(&bytes, pos)?;
        let naxis = hdr.get("NAXIS").and_then(|v| v.as_i64()).unwrap_or(0);
        let dims: Vec<usize> = (1..=naxis)
            .map(|k| hdr.get(&format!("NAXIS{k}")).and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize)
            .collect();
        let bitpix = hdr.get("BITPIX").and_then(|v| v.as_i64()).ok_or("FITS header without BITPIX")?;
        let bp = bytes_per(bitpix)?;
        let n: usize = if naxis == 0 { 0 } else { dims.iter().product() };
        let xt = hdr.get("XTENSION").map(|v| v.as_str()).unwrap_or_else(|| "IMAGE".into());
        if naxis >= 2 && n > 0 && xt.trim() == "IMAGE" {
            return fits_image(&bytes, data, &hdr, bitpix, &dims, half);
        }
        if xt.trim() == "BINTABLE" && hdr.get("ZIMAGE") == Some(&Value::Bool(true)) {
            return Err(format!("{} is tile-compressed (.fz); decompress it first (e.g. with funpack).", basename(path)));
        }
        let pcount = hdr.get("PCOUNT").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
        let nbytes = n * bp + pcount;
        pos = data + 2880 * nbytes.div_ceil(2880);
        if pos >= bytes.len() {
            return Err(format!("{} contains no image.", basename(path)));
        }
    }
}

fn fits_image(bytes: &[u8], data: usize, hdr: &Header, bitpix: i64, dims: &[usize], half: bool) -> Result<Img, String> {
    let n: usize = dims.iter().product();
    let bp = bytes_per(bitpix)?;
    if data + n * bp > bytes.len() {
        return Err("Truncated FITS data".into());
    }
    let raw = &bytes[data..data + n * bp];
    let bscale = hdr.get("BSCALE").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let bzero = hdr.get("BZERO").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let mut a: Vec<f32> = Vec::with_capacity(n);
    match bitpix {
        8 => a.extend(raw.iter().map(|&b| b as f32)),
        16 => a.extend(raw.chunks_exact(2).map(|c| i16::from_be_bytes([c[0], c[1]]) as f32)),
        32 => a.extend(raw.chunks_exact(4).map(|c| i32::from_be_bytes([c[0], c[1], c[2], c[3]]) as f32)),
        64 => a.extend(raw.chunks_exact(8).map(|c| i64::from_be_bytes(c.try_into().unwrap()) as f32)),
        -32 => a.extend(raw.chunks_exact(4).map(|c| f32::from_be_bytes([c[0], c[1], c[2], c[3]]))),
        -64 => a.extend(raw.chunks_exact(8).map(|c| f64::from_be_bytes(c.try_into().unwrap()) as f32)),
        _ => return Err(format!("Unsupported BITPIX {bitpix}")),
    }
    a.iter_mut().for_each(|v| *v = *v * bscale + bzero);
    // Scale to about 0…1.
    let top = if bitpix > 0 {
        let bits = (8 * bp) as i32;
        if bitpix == 8 {
            255.0
        } else if (bzero - 2f32.powi(bits - 1)).abs() < 0.5 {
            2f32.powi(bits) - 1.0
        } else {
            2f32.powi(bits - 1) - 1.0
        }
    } else {
        // Floats are usually 0…1 (bright stars may go above) or 0…65535: judge by
        // the bulk of the pixels, not the brightest one.
        let step = (a.len() / 1_000_000).max(1);
        let mut fin: Vec<f32> = a.iter().step_by(step).copied().filter(|v| v.is_finite()).collect();
        if fin.is_empty() {
            1.0
        } else {
            let hi = quantile(&mut fin, 0.99);
            if hi <= 1.5 {
                1.0
            } else if hi <= 65535.0 * 1.01 {
                65535.0
            } else {
                fin.iter().cloned().fold(f32::MIN, f32::max)
            }
        }
    };
    a.iter_mut().for_each(|v| *v = if v.is_finite() { *v / top } else { 0.0 });
    let (w, h) = (dims[0], dims[1]);
    let planes = if dims.len() >= 3 { dims[2] } else { 1 };
    // FITS stores x (NAXIS1) fastest, rows bottom first by default.
    let plane = |k: usize| Gray::from_vec(h, w, a[k * w * h..(k + 1) * w * h].to_vec());
    let pat = hdr
        .get("BAYERPAT")
        .or_else(|| hdr.get("COLORTYP"))
        .map(|v| v.as_str().trim().to_uppercase())
        .unwrap_or_default();
    let mut img = if planes == 1 && ["RGGB", "BGGR", "GRBG", "GBRG"].contains(&pat.as_str()) {
        let mut p: Vec<u8> = pat.bytes().collect();
        // Bayer offsets shift the pattern by one row / column.
        if hdr.get("XBAYROFF").and_then(|v| v.as_i64()).unwrap_or(0) % 2 != 0 {
            p = vec![p[1], p[0], p[3], p[2]];
        }
        if hdr.get("YBAYROFF").and_then(|v| v.as_i64()).unwrap_or(0) % 2 != 0 {
            p = vec![p[2], p[3], p[0], p[1]];
        }
        debayer(&plane(0), std::str::from_utf8(&p).unwrap(), half)
    } else if planes >= 3 {
        let (r, g, b) = (plane(0), plane(1), plane(2));
        Img::from_fn(h, w, |i, j| [r.at(i, j), g.at(i, j), b.at(i, j)])
    } else {
        plane(0).map(|v| [v, v, v])
    };
    let order = hdr.get("ROWORDER").map(|v| v.as_str().trim().to_uppercase()).unwrap_or_else(|| "BOTTOM-UP".into());
    if order != "TOP-DOWN" {
        img = img.flip_rows();
    }
    Ok(img)
}

/// Write `img` as a 32-bit float FITS file with three colour planes (bottom
/// row first, the FITS default), so astronomy software can process it further.
pub fn write_fits(path: &str, img: &Img) -> Result<(), String> {
    let (h, w) = (img.h, img.w);
    fn card(k: &str, v: &str, string: bool) -> String {
        let val = if string { format!("{:<20}", format!("'{v}'")) } else { format!("{v:>20}") };
        format!("{:<80}", format!("{:<8}= {val}", k))
    }
    let mut header = String::new();
    header += &card("SIMPLE", "T", false);
    header += &card("BITPIX", "-32", false);
    header += &card("NAXIS", "3", false);
    header += &card("NAXIS1", &w.to_string(), false);
    header += &card("NAXIS2", &h.to_string(), false);
    header += &card("NAXIS3", "3", false);
    header += &card("BZERO", "0.0", false);
    header += &card("BSCALE", "1.0", false);
    header += &card("ROWORDER", "BOTTOM-UP", true);
    header += &card("CREATOR", "PanoStitch", true);
    header += &format!("{:<80}", "END");
    let pad = 2880 * header.len().div_ceil(2880) - header.len();
    header += &" ".repeat(pad);
    let mut data: Vec<u8> = Vec::with_capacity(12 * h * w);
    for c in 0..3 {
        for y in (0..h).rev() {
            for x in 0..w {
                let v = img.at(y, x)[c];
                data.extend_from_slice(&(if v.is_finite() { v } else { 0.0 }).to_be_bytes());
            }
        }
    }
    let dpad = 2880 * data.len().div_ceil(2880) - data.len();
    data.extend(std::iter::repeat_n(0u8, dpad));
    let mut out = header.into_bytes();
    out.extend(data);
    std::fs::write(path, out).map_err(|e| format!("Cannot save {}: {e}", basename(path)))
}

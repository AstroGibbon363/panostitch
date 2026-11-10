//! XISF (PixInsight images).
//!
//! Reads the first image of an XISF file: 8/16/32-bit integer or 32/64-bit
//! float, mono or RGB, planar or interleaved, little or big endian, stored as an
//! attachment, uncompressed or compressed with zlib, LZ4 or Zstandard (with or
//! without byte shuffling). Mono images with a colour filter array (one-shot
//! colour cameras) are debayered. Writes 32-bit float RGB, uncompressed.

use super::{basename, debayer};
use crate::image::{Gray, Img};
use std::collections::HashMap;
use std::io::Read;

fn xml_attrs(s: &str) -> HashMap<String, String> {
    // name = "value" pairs
    let mut out = HashMap::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // skip to a name start
        while i < b.len() && !(b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b':') {
            i += 1;
        }
        let ns = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b':' || b[i] == b'-') {
            i += 1;
        }
        let name = &s[ns..i];
        let mut j = i;
        while j < b.len() && b[j] == b' ' {
            j += 1;
        }
        if j < b.len() && b[j] == b'=' {
            j += 1;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                let vs = j + 1;
                let mut k = vs;
                while k < b.len() && b[k] != b'"' {
                    k += 1;
                }
                out.insert(name.to_string(), s[vs..k].to_string());
                i = k + 1;
                continue;
            }
        }
        if i == ns {
            i += 1;
        }
    }
    out
}

/// The `<Image …>` element: (attributes string, body).
fn find_image(xml: &str) -> Option<(String, String)> {
    let start = xml.find("<Image")?;
    let rest = &xml[start + 6..];
    // Attributes end at the first '>' (self-closing or not).
    let gt = rest.find('>')?;
    let attrs = rest[..gt].trim_end_matches('/').to_string();
    let self_closing = rest[..gt].trim_end().ends_with('/');
    if self_closing {
        return Some((attrs, String::new()));
    }
    let body = &rest[gt + 1..];
    let end = body.find("</Image>").unwrap_or(body.len());
    Some((attrs, body[..end].to_string()))
}

/// Read the first image of an XISF file as RGB (see the module comment).
pub fn read_xisf(path: &str, half: bool) -> Result<Img, String> {
    let name = basename(path);
    let bytes = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
    if bytes.len() < 16 || &bytes[..8] != b"XISF0100" {
        return Err(format!("{name} is not an XISF file."));
    }
    let hlen = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if 16 + hlen > bytes.len() {
        return Err(format!("{name}: truncated header."));
    }
    let xml = String::from_utf8_lossy(&bytes[16..16 + hlen]).to_string();
    let (attrs, body) = find_image(&xml).ok_or_else(|| format!("{name} contains no image."))?;
    let a = xml_attrs(&attrs);
    let geom: Vec<usize> = a
        .get("geometry")
        .ok_or_else(|| format!("{name}: no geometry."))?
        .split(':')
        .map(|s| s.parse::<usize>().unwrap_or(0))
        .collect();
    if geom.len() > 3 {
        return Err(format!("{name}: only 2-D images are supported."));
    }
    let (w, h) = (geom[0], geom[1]);
    let c = if geom.len() >= 3 { geom[2] } else { 1 };
    let fmt = a.get("sampleFormat").cloned().unwrap_or_else(|| "UInt16".into());
    let bp = match fmt.as_str() {
        "UInt8" => 1,
        "UInt16" => 2,
        "UInt32" | "Float32" => 4,
        "Float64" => 8,
        _ => return Err(format!("{name}: sample format {fmt} is not supported.")),
    };
    let loc: Vec<&str> = a.get("location").map(|s| s.as_str()).unwrap_or("").split(':').collect();
    if loc[0] != "attachment" || loc.len() < 3 {
        return Err(format!("{name}: {} image data is not supported (only attachments).", loc[0]));
    }
    let pos: usize = loc[1].parse().map_err(|_| "bad location")?;
    let size: usize = loc[2].parse().map_err(|_| "bad location")?;
    if pos + size > bytes.len() {
        return Err(format!("{name}: the image data is incomplete."));
    }
    let blk = xisf_decompress(
        &bytes[pos..pos + size],
        a.get("compression").map(|s| s.as_str()).unwrap_or(""),
        a.get("subblocks").map(|s| s.as_str()).unwrap_or(""),
    )?;
    let n = w * h * c;
    if blk.len() < n * bp {
        return Err(format!("{name}: the image data is incomplete."));
    }
    let big = a.get("byteOrder").map(|s| s == "big").unwrap_or(false);
    let mut v: Vec<f32> = Vec::with_capacity(n);
    for ch in blk[..n * bp].chunks_exact(bp) {
        let x = match (fmt.as_str(), big) {
            ("UInt8", _) => ch[0] as f32 / 255.0,
            ("UInt16", false) => u16::from_le_bytes([ch[0], ch[1]]) as f32 / 65535.0,
            ("UInt16", true) => u16::from_be_bytes([ch[0], ch[1]]) as f32 / 65535.0,
            ("UInt32", false) => u32::from_le_bytes(ch.try_into().unwrap()) as f32 / u32::MAX as f32,
            ("UInt32", true) => u32::from_be_bytes(ch.try_into().unwrap()) as f32 / u32::MAX as f32,
            ("Float32", false) => f32::from_le_bytes(ch.try_into().unwrap()),
            ("Float32", true) => f32::from_be_bytes(ch.try_into().unwrap()),
            ("Float64", false) => f64::from_le_bytes(ch.try_into().unwrap()) as f32,
            _ => f64::from_be_bytes(ch.try_into().unwrap()) as f32,
        };
        v.push(x);
    }
    // Floats: scale by the declared bounds.
    if fmt.starts_with("Float") {
        if let Some(b) = a.get("bounds") {
            let p: Vec<f32> = b.split(':').map(|s| s.parse::<f32>().unwrap_or(f32::NAN)).collect();
            if p.len() == 2 && !(p[0] == 0.0 && p[1] == 1.0) && p[1] > p[0] {
                v.iter_mut().for_each(|x| *x = (*x - p[0]) / (p[1] - p[0]));
            }
        }
    }
    v.iter_mut().for_each(|x| {
        if !x.is_finite() {
            *x = 0.0
        }
    });
    // Planar: one channel after the other; normal: channels interleaved. Rows top first, x fastest.
    let normal = a.get("pixelStorage").map(|s| s == "Normal").unwrap_or(false);
    let plane = |k: usize| -> Gray {
        if normal {
            Gray::from_vec(h, w, (0..w * h).map(|i| v[i * c + k]).collect())
        } else {
            Gray::from_vec(h, w, v[k * w * h..(k + 1) * w * h].to_vec())
        }
    };
    let pat = body
        .find("<ColorFilterArray")
        .map(|s| {
            let rest = &body[s + 17..];
            let gt = rest.find('>').unwrap_or(rest.len());
            xml_attrs(&rest[..gt]).get("pattern").map(|p| p.to_uppercase()).unwrap_or_default()
        })
        .unwrap_or_default();
    if c == 1 && ["RGGB", "BGGR", "GRBG", "GBRG"].contains(&pat.as_str()) {
        return Ok(debayer(&plane(0), &pat, half));
    }
    if c >= 3 {
        let (r, g, b) = (plane(0), plane(1), plane(2));
        return Ok(Img::from_fn(h, w, |i, j| [r.at(i, j), g.at(i, j), b.at(i, j)]));
    }
    Ok(plane(0).map(|x| [x, x, x]))
}

fn xisf_decompress(blk: &[u8], comp: &str, subblocks: &str) -> Result<Vec<u8>, String> {
    if comp.is_empty() {
        return Ok(blk.to_vec());
    }
    let parts: Vec<&str> = comp.split(':').collect();
    let mut codec = parts[0].to_string();
    let usize_: usize = parts.get(1).and_then(|s| s.parse().ok()).ok_or("bad compression")?;
    let item: usize = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let shuffled = codec.ends_with("+sh");
    if shuffled {
        codec = codec.trim_end_matches("+sh").to_string();
    }
    let chunks: Vec<(usize, usize)> = if subblocks.is_empty() {
        vec![(blk.len(), usize_)]
    } else {
        subblocks
            .split(':')
            .map(|sb| {
                let c: Vec<usize> = sb.split(',').map(|x| x.parse().unwrap_or(0)).collect();
                (c[0], c[1])
            })
            .collect()
    };
    let mut out = Vec::new();
    let mut p = 0;
    for (cs, us) in chunks {
        let src = &blk[p..p + cs];
        p += cs;
        let part = match codec.as_str() {
            "zlib" => {
                let mut d = flate2::read::ZlibDecoder::new(src);
                let mut v = Vec::with_capacity(us);
                d.read_to_end(&mut v).map_err(|e| format!("zlib: damaged data ({e})"))?;
                v
            }
            "lz4" | "lz4hc" => lz4_block(src, us)?,
            "zstd" => zstd::bulk::decompress(src, us).map_err(|e| format!("zstd: damaged data ({e})"))?,
            other => return Err(format!("XISF compression “{other}” is not supported.")),
        };
        out.extend(part);
    }
    Ok(if shuffled { unshuffle(&out, item) } else { out })
}

/// Decode an LZ4 block (the format XISF uses for lz4 and lz4hc).
fn lz4_block(src: &[u8], n: usize) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; n];
    let (mut i, mut o) = (0usize, 0usize);
    let err = || "LZ4: damaged data".to_string();
    while i < src.len() {
        let tok = src[i];
        i += 1;
        let mut lit = (tok >> 4) as usize;
        if lit == 15 {
            loop {
                let b = *src.get(i).ok_or_else(err)?;
                i += 1;
                lit += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        if i + lit > src.len() || o + lit > n {
            return Err(err());
        }
        out[o..o + lit].copy_from_slice(&src[i..i + lit]);
        o += lit;
        i += lit;
        if i >= src.len() {
            break; // the last sequence has only literals
        }
        let off = src[i] as usize | ((*src.get(i + 1).ok_or_else(err)? as usize) << 8);
        i += 2;
        let mut ml = (tok & 0x0f) as usize;
        if ml == 15 {
            loop {
                let b = *src.get(i).ok_or_else(err)?;
                i += 1;
                ml += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        ml += 4;
        if off == 0 || off > o || o + ml > n {
            return Err(err());
        }
        for k in 0..ml {
            out[o + k] = out[o - off + k];
        }
        o += ml;
    }
    if o != n {
        return Err(err());
    }
    Ok(out)
}

/// Undo byte shuffling: byte j of every item was stored together.
fn unshuffle(b: &[u8], item: usize) -> Vec<u8> {
    if item <= 1 {
        return b.to_vec();
    }
    let n = b.len() / item;
    let mut out = vec![0u8; b.len()];
    for j in 0..item {
        for i in 0..n {
            out[i * item + j] = b[j * n + i];
        }
    }
    out[n * item..].copy_from_slice(&b[n * item..]);
    out
}

/// Write `img` as a 32-bit float XISF file (RGB, planar, uncompressed).
/// Negative values become 0; values above 1 are kept, with the bounds widened.
pub fn write_xisf(path: &str, img: &Img) -> Result<(), String> {
    let (h, w) = (img.h, img.w);
    let mut data: Vec<f32> = Vec::with_capacity(3 * h * w);
    for c in 0..3 {
        for p in &img.px {
            let v = p[c];
            data.push(if v.is_finite() { v.max(0.0) } else { 0.0 });
        }
    }
    let hi = data.iter().cloned().fold(1f32, f32::max);
    let stamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let header = |pos: usize| {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xisf version=\"1.0\" xmlns=\"http://www.pixinsight.com/xisf\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://www.pixinsight.com/xisf http://pixinsight.com/xisf/xisf-1.0.xsd\">\n<Image geometry=\"{w}:{h}:3\" sampleFormat=\"Float32\" bounds=\"0:{hi}\" colorSpace=\"RGB\" pixelStorage=\"Planar\" byteOrder=\"little\" location=\"attachment:{pos}:{}\"/>\n<Metadata>\n<Property id=\"XISF:CreationTime\" type=\"TimePoint\" value=\"{stamp}\"/>\n<Property id=\"XISF:CreatorApplication\" type=\"String\">PanoStitch</Property>\n</Metadata>\n</xisf>",
            4 * data.len()
        )
    };
    let mut pos = 4096; // the data starts on a 4 KiB boundary
    while 16 + header(pos).len() > pos {
        pos += 4096;
    }
    let xml = header(pos);
    let mut out: Vec<u8> = Vec::with_capacity(pos + 4 * data.len());
    out.extend_from_slice(b"XISF0100");
    out.extend_from_slice(&(xml.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(xml.as_bytes());
    out.extend(std::iter::repeat_n(0u8, pos - 16 - xml.len()));
    for v in &data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, out).map_err(|e| format!("Cannot save {}: {e}", basename(path)))
}

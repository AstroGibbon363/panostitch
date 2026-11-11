//! Using PanoStitch from your own Rust program (no window).
//!
//!   cargo run --release --example scripted -- a.NEF b.NEF c.NEF
//!
//! Stitches the given images into pano.tif (spherical projection).

use panostitch::io;
use panostitch::stitcher::{self, Projection, StitchParams};

fn main() -> Result<(), String> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let p = StitchParams { projection: Projection::Spherical, ..Default::default() };
    let pano = stitcher::stitch_files(&paths, &p, true, &mut |m| println!("{m}"))?;
    io::save_panorama("pano.tif", &pano)?;

    // A linear image (e.g. a FITS mosaic) can be stretched before saving it as TIFF:
    let (black, mid, white) = stitcher::auto_stretch(&pano, 0.25, 2.8);
    io::save_panorama("pano_stretched.tif", &stitcher::hist_stretch(&pano, black, mid, white))?;
    Ok(())
}

//! PanoStitch: opens the window (panorama stitching and star stacking).
//!
//!   panostitch [images …]

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = panostitch::gui::run(paths) {
        eprintln!("PanoStitch could not open its window: {e}");
        std::process::exit(1);
    }
}

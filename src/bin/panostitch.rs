//! PanoStitch: opens the window (panorama stitching and star stacking).
// No console window next to it on Windows (messages go to a dialog and the crash log).
#![cfg_attr(windows, windows_subsystem = "windows")]
//!
//!   panostitch [images …]

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = panostitch::gui::run(paths) {
        panostitch::gui::fatal_message(&format!(
            "PanoStitch could not open its window:\n{e}\n\nIt needs OpenGL 2.0 or newer: updating the graphics driver usually helps (on old PCs, the driver from the manufacturer of the graphics chip rather than the one Windows installs)."
        ));
        std::process::exit(1);
    }
}

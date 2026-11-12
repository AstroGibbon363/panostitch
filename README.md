# PanoStitch

Panorama stitching and star stacking for landscape astrophotography, written in
Rust. Joins overlapping photos (JPEG, TIFF, PNG, camera RAW, FITS, XISF) into a
panorama or mosaic, and stacks star images (aligned stars or star trails) with
calibration frames and sky/ground separation.

The Julia version is on the `main` branch. This branch (`v0.3`) is the Rust
rewrite: one small native program, no runtime to install. Downloads for Linux,
Windows and macOS are on the Releases page; `CHANGELOG.md` lists what changed.

## Running

```
panostitch [images or project files …]   # the window
panostack [options] LIGHT_FRAMES_OR_FOLDER # star stacking, no window (see --help)
```

The user manual is in `docs/` (English, Czech, German).

## Building

Needs Rust (https://rustup.rs). On Linux also the usual X11/Wayland and OpenGL
development libraries (present on any desktop install).

```
cargo build --release      # target/release/panostitch and panostack
cargo test --release       # engine, file formats, and both tabs of the window (headless)
```

Build on each target system (Windows, macOS, Linux); the result is a single
executable of about 15 MB that needs nothing else.

## Layout

| Path | Contents |
|---|---|
| `src/image.rs` | Float images, blur, resize, statistics |
| `src/io/` | Loading and saving: JPEG/PNG/TIFF/BMP, FITS, XISF, camera RAW |
| `src/stitcher/` | Features, matching, RANSAC, placement, bundle adjustment, projections, blending |
| `src/stacker/` | Calibration, star detection and alignment, combining, post-processing, the run |
| `src/gui/` | The window (egui): stitcher tab, stacking tab, file dialogs |
| `src/bin/` | `panostitch` (window) and `panostack` (command line) |
| `tests/` | Tests: stacking on synthetic night scenes, stitching, file formats, both tabs of the window |
| `examples/` | Using the library from your own Rust code |
| `docs/` | User manual in English, Czech and German (Markdown) |

Camera RAW files are decoded by a built-in pure-Rust decoder (`rawler`).
LibRaw's `dcraw_emu`, RawTherapee, darktable or dcraw are used too if installed
(choose under "RAW via" / "RAW conversion"). File dialogs use zenity, kdialog
or yad on Linux, AppleScript on macOS and PowerShell on Windows; without them,
type paths into the path box.

## License

PanoStitch is free software under the GNU General Public License, version 3
or later (see `LICENSE`). The DejaVu fonts in `assets/fonts` have their own
free license (`assets/fonts/LICENSE-DejaVu.txt`).

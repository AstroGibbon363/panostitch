# PanoStitch

Panorama stitching and star stacking for landscape astrophotography, written in
Rust. Joins overlapping photos (JPEG, TIFF, PNG, camera RAW, FITS, XISF) into a
panorama or mosaic, and stacks star images (aligned stars or star trails) with
calibration frames and sky/ground separation.

It is one small native program written in Rust, with no runtime to install.
The `main` branch holds the latest release; development happens on the
branch of the next version (e.g. `v0.5`). The first version, written in
Julia, is kept on the `julia` branch. Downloads for Linux
and Windows are on the Releases page (on macOS, build from source as below); `CHANGELOG.md` lists what changed.

## Running

```
panostitch [images or project files …]   # the window
panostack [options] LIGHT_FRAMES_OR_FOLDER # star stacking, no window (see --help)
```

The user manual is in `docs/` (English, Czech, German, Spanish, French, Polish) and inside the program (F1). The Timelapse tab
makes MP4 and WebM videos with ffmpeg if it is installed (optional; animated
GIF and numbered images work without it).

## Building

Needs Rust (https://rustup.rs). On Linux also the usual X11/Wayland and OpenGL
development libraries (present on any desktop install).

```
cargo build --release      # target/release/panostitch and panostack
cargo test --release       # engine, post-processing, time-lapse, file formats, and the window's tabs (headless)
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
| `src/finish.rs` | The Post-processing tab's engine: geometry, stretches (MTF, arcsinh, GHS), white balance, light, curves, colour, detail, sky and foreground looks |
| `src/timelapse.rs` | Time-lapse videos: shapes and sizes, camera move, deflicker, projects, GIF and numbered images, MP4 / WebM through ffmpeg |
| `src/gui/` | The window (egui): stitcher, stacking, post-processing and timelapse tabs, file dialogs, the built-in manual (`help.rs`), texts in six languages (`i18n.rs`, `i18n_*.rs`) |
| `src/bin/` | `panostitch` (window) and `panostack` (command line) |
| `tests/` | Tests: stacking on synthetic night scenes, stitching, post-processing, time-lapse videos, file formats, the window's tabs, translations and the manuals' links |
| `examples/` | Using the library from your own Rust code |
| `docs/` | User manual in English, Czech, German, Spanish, French and Polish (Markdown) |

Camera RAW files are decoded by a built-in pure-Rust decoder (`rawler`).
LibRaw's `dcraw_emu`, RawTherapee, darktable or dcraw are used too if installed
(choose under "RAW via" / "RAW conversion"). File dialogs use zenity, kdialog
or yad on Linux, AppleScript on macOS and PowerShell on Windows; without them,
type paths into the path box.

## License

PanoStitch is free software under the GNU General Public License, version 3
or later (see `LICENSE`). The DejaVu fonts in `assets/fonts` have their own
free license (`assets/fonts/LICENSE-DejaVu.txt`).

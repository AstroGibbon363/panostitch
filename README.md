# PanoStitch

Panorama stitching and star stacking for landscape astrophotography, written in
Julia. Joins overlapping photos (JPEG, TIFF, PNG, camera RAW, FITS) into a
panorama or mosaic, and stacks star images (aligned stars or star trails) with
calibration frames and sky/ground separation.

## Running

Needs Julia 1.13 or newer. Packages install automatically on the first start.

```
julia bin/panostitch.jl [images or project files ...]    # the window
julia bin/panostack.jl [options] STAR_IMAGES_OR_FOLDER   # star stacking, no window
```

The first start after installing or changing the code precompiles (a few
minutes, once). After that the window opens in about 5 seconds.

From a script, without graphics:

```julia
include("examples/scripted_stitch.jl")
pano = Stitcher.stitch(["a.NEF", "b.NEF", "c.NEF"]; projection = :cylindrical)
Stitcher.save_panorama("pano.tif", pano)
```

## Building

```
julia build/build_app.jl     # standalone program in build/PanoStitch (no Julia needed to run it)
julia docs/make_pdf.jl       # PDF manuals from docs/manual_*.md
```

The standalone build takes about half an hour and must be done on each target
system (Windows, macOS, Linux) separately. Both scripts install what they need
(PackageCompiler; pandoc and Typst) into their own environments.

## Layout

| Path | Contents |
|---|---|
| `src/` | The `PanoStitch` package: `Stitcher` (stitcher.jl) and `Stacker` (stacker.jl). No graphics. |
| `ext/PanoStitchGUIExt/` | The window (GLMakie), a package extension loaded when GLMakie is loaded. |
| `app/` | The program as a package (PanoStitchApp: PanoStitch + GLMakie, versions pinned in its Manifest), with the entry point of the standalone build. |
| `bin/` | Launchers: the window and the command-line stacker. |
| `examples/` | Using the package from your own scripts. |
| `docs/` | User manual in English, Czech and German (Markdown and PDF), and the PDF build script. |
| `build/` | Build script for the standalone program. |
| `assets/fonts/` | DejaVu fonts used by the window (license included). |

Camera RAW files are converted by LibRaw's `dcraw_emu`, which comes with the
program (LibRaw_jll) on Windows, macOS and Linux. RawTherapee, darktable or
dcraw are used too if installed. See the manual, section "RAW files".

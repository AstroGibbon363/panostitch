# Changes

## v0.4.0 (in development)

### Post-processing tab (new)
- The last steps on a stacked or stitched image in one place: turn and crop (any angle, empty corners cut away, crop to 3:2 / 16:9 / 1:1 …), stretch, white balance, light, curves, colour, noise, star reduction, sharpening, and saving at a chosen size (e.g. 2048 px for forums) with JPEG quality.
- **Stretches:** MTF (automatic, each colour on its own by default, or manual), Arcsinh and Generalized Hyperbolic (GHS), both also inverse; "Keep star colours"; black and symmetry point from the sky background.
- **White balance** on linear light, before the stretch: sample a grey rectangle on the preview, neutral background, temperature and tint.
- **Light:** exposure, contrast, highlights, midtones, shadows, whites, blacks. **Curves** for RGB and each colour.
- **Display modes** as in Siril (linear, log, square root, squared, asinh, AutoStretch, histogram equalisation): only the view changes.
- Live preview on a small copy; saving processes the full image with the values measured on the preview, so the file looks like the preview. Compare with the starting point (hold B).
- Results come in from the stacking tab ("Use in Post-processing") and the stitcher ("Post-process" after saving).
- **Undo / redo** (Ctrl+Z, Ctrl+Y), **crop on the image** (draw, resize or move a frame on the whole image), and **presets** that save the look (not the turn, crop or size) to apply to other images.
- **Sky and foreground:** looks of their own for the sky and the foreground (light, temperature, colour, detail) through a mask painted with a brush or found by "Detect sky", with a soft edge. The mask lies on the image, so it stays in place when turning or cropping.

### Timelapse tab (new)
- Makes a video from a sequence of landscape frames shot from a tripod.
- **Look** for the whole frame, the sky and the foreground (the Post-processing light and colour sliders, the same brush mask and sky detection, or a disc inside a fisheye's image circle with an optional radial gradient), shown on the selected frame. Optionally every frame starts from the Post-processing tab's look; the automatic stretch is measured once, so the video does not flicker.
- **Camera move:** a slider-like move across the frame (the view a few percent smaller, gliding left, right, up or down, with a gentle start and stop) and a slow zoom; the preview outlines where the video starts and ends.
- **Deflicker** (light or strong) of brightness and of each colour, region by region on a coarse grid, so large-scale changes in one part of the frame are evened out too; slow changes such as dusk stay.
- RAW frames are read with the fast built-in decoder by default; **RAW via** can choose RawTherapee (applies its `.pp3` edits, about ten times slower) or another installed decoder. The tab says how many frames have `.pp3` files.
- 16:9 crop, resolution up to 4K (never scaled up), frame rate, play forward then back, hold the last frame.
- **Output:** MP4 (H.264) and WebM (VP9) through ffmpeg when it is installed (or next to the program); animated GIF and numbered TIFF / PNG / JPEG images built in.
- Runs in the background with a progress bar (time elapsed and left) and Cancel, then a report: frames, video size, frame rate and length, time taken, file and its size.

### Speed and memory
- **Big panoramas in less memory:** the full-size panorama is rendered in bands of rows when it does not fit in one piece and kept in the file's own format (6 bytes per pixel for 16-bit TIFF instead of about 50); photos read again at full size are kept in half precision. Smart blending renders each band with overlap, so the bands join seamlessly.
- Star stacking loads the calibration frames in parallel (they were read one by one) and loads and measures as many light frames at once as the free memory allows (it was at most 4). 60 frames of 24 MP with darks, flats and bias: 118 s → 81 s on a 28-thread PC, the same result bit for bit.
- Long jobs use at most 85 % of the CPU cores, and memory checks leave 15 % of the memory free, so the PC stays responsive.

### Reports and times
- After stacking and after saving a panorama, a short report: photos / frames used, time taken, result size, file and its size, or what went wrong.
- The stacking progress bar shows the time elapsed next to the time left; the stitcher's status line shows the time elapsed while it works.

## v0.3.1

Stability, ease of use, and testing on real data.

### Panorama tab
- **Output size** (10–200 %) with quick buttons ¼, ½, full and 2×. A line under it shows how much memory and disk space the save needs and warns when it won't fit.
- RAW photos loaded at half size for speed are reread at full size when saving, so a 100 % panorama has the camera's full resolution.
- Finer **Yaw / Pitch / Roll** controls: Pitch and Roll sliders are fine near 0°, a drag box moves in 0.02° steps, and ◀ ▶ buttons step 0.1° (1° with Shift).
- The astro preset is 1 EV brighter and keeps more stars near the corners.

### Stacking tab
- **Save as** row: name suggestions, a format dropdown, the output folder, and a warning when the file already exists.
- Estimate of the space the frames need, kept in memory or on disk, with a warning when there isn't enough.
- **Presets**: Milky Way landscape, Star trails, Deep sky.
- Progress bar with time left, one Cancel button, readable explanations, "All sky" region.
- RAW frames are never rotated when stacking; noisy master flats are smoothed.

### Both tabs
- Output formats: TIFF, PNG, JPEG, FITS, XISF.
- Keyboard shortcuts: Ctrl+S save project, Ctrl+O open project, Ctrl+Enter run, F fit view (shown in tooltips; full list in the manual).
- Asks before closing when there is unsaved work or a job running.
- Side panels can be dragged wider; rows wrap instead of being cut off on small screens.
- The version is shown in the window title.

### Stability
- Long jobs run on separate low-priority threads, so the window keeps responding on slow PCs.
- When memory is nearly full, loading stops and the status line says which images were left out, instead of the PC freezing on swap. A save that won't fit is refused with a suggested output size.
- The output folder is checked before a long job starts, not at its end.
- Large images are shown within the graphics card's texture limit (older and built-in graphics).
- After a crash, `crash.log` in the settings folder has the details and the next start says where it is.
- Windows: no extra console window; a message box explains what to do when OpenGL is missing.

### Downloads
- Ready-made downloads for Linux and Windows only. macOS users build from source (see the README); GitHub's Mac build machines were too often unavailable.

## v0.3.0

Rust version: one native program, no Julia runtime. GPL-3.0 license. Builds for Linux, Windows and macOS.

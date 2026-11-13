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

### Timelapse tab (controls only)
- Frames with a preview of the selected one; kind (time-lapse, growing or fading star trails), deflicker, output format, frame rate, resolution and the video length.
- Look: the Post-processing light and colour sliders, separately for the whole frame, the sky and the foreground, with a brush mask painted on the frame (left = sky, right = foreground).
- Nothing is applied yet: making the video comes later.

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

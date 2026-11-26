# Changes

## v0.4.1 (in development)

### Fixes
- **Presets no longer change the exposure:** *astro / night landscape* added +1 EV on top of the Exposure slider; a preset now sets only how to match and project. Brightness stays with **Exposure (EV)** and **Auto exposure**.
- **Smaller screens (1280 × 720, 1366 × 768):** in the Panorama tab the memory estimate under *Output size* wraps instead of widening the right panel until the preview covered its left edge (cut-off labels, Czech and Spanish especially). A new test renders every tab in every language at these sizes and fails on any text cut off or panel overlap.
- **Untranslated texts translated:** the line above the panorama (projection, typical error, colour legend), the pair menu (inliers, rejected, none), *full* output size, *automatic* format, the model name in pair messages, the Post-processing summary (*In use: …*), its preset menu, the stacking result line and base frame label, the Timelapse *RAW via* tip. The stacking note under the post-processing options now follows a change of language.
- **Clearer controls:** *Save linear / stretched* are radio buttons (a highlighted label next to a plain one did not read as a choice); **Low** and **High** stand before their sliders; the *Simple* switch lost its ✓ (it looked ticked when off); zoom is **+** / **−** in the Panorama toolbar, so it takes fewer lines; the pair message is readable size; Post-processing's output size reads *full size* instead of *0 px*, and the output file field shows the whole path on hover.
- **Display modes:** the *Display* menu with **Low** / **High** above the preview (Panorama, Stacking and Post-processing tabs) no longer runs under the panel on the right when the window is narrow: it moves to the next line.

## v0.4.0

Released 2026-11-25. New since v0.3.2: the Post-processing and Timelapse tabs, night panoramas up to the zenith, star stacking with RBF background extraction and new stretches, wavelet noise reduction, six languages and in-app help.

### Star stacking
- **RBF background extraction** for frames that are all sky (deep sky): a flexible surface through a grid of sky samples that follows several glows or a bright corner where a polynomial cannot. Stars, galaxies, nebulae, the Milky Way and their surroundings are left out of the samples automatically, so a galaxy keeps its faint halo. **Samples per line**, **Grid tolerance** and **Smoothing** as in Siril; **Show the samples** marks the used and left-out ones on the result. Landscapes keep the polynomial.
- **Stretches:** the stretch is now called **MTF stretch** (automatic stretch kept), joined by **Arcsinh** (keeps star colours) and **GHS**, with their black point (and symmetry point) measured on the sky background.
- **Colour calibration** has two modes: **Auto** (as before) and **Manual**: drag a background area and a white reference on the result, then temperature and tint.
- **Save linear or stretched:** *Linear (for processing)* / *Stretched (for viewing)*, whatever the format (until chosen: FITS / XISF linear, others stretched). The old button that switched all processing off is gone.

### Noise reduction
- **Wavelet noise reduction** in place of the old smoothing: the image is split into layers of detail and the small wiggles of noise are shrunk in each, keeping stars and edges. The noise is measured area by area, so it works on linear and stretched images alike. **Colour noise** uses it on the colour layers only (the brightness stays exactly), in the Post-processing tab and in star stacking.

### All tabs
- **Linear or stretched** when saving the panorama and in Post-processing too (Post-processing saves only the linear steps then: turn and crop, background, colour calibration, white balance), each explained where it is chosen.

### Panorama tab
- **Crop in the Panorama tab:** **✂ Crop** above the view draws a frame on the panorama (drag, adjust edges and corners, move). Only that part is rendered and saved, so a big panorama needs less memory and no trip to Post-processing just to crop.
- **⬇ Save & post-process…** above the view saves the panorama and opens it in the Post-processing tab.
- **Post-processing memory check:** the memory needed to save was worked out for the whole image even when it was cropped, so a crop never got past "not enough memory". It now counts the cropped size, and the message says it means RAM (free disk space does not help).
- **Display modes as in Siril** (view only): Linear, Logarithm, Square root, Squared, Asinh, AutoStretch, Histogram equalisation, with **Low** and **High** cut-offs (applied exactly, so a narrow range of faint linear data is not posterised). In the Panorama tab and the Star stacking result (also *As processed*, the tab's own stretch) and in Post-processing.
- Memory: the undo history shares the matched points of the pairs instead of copying them at every step.

### Stability (star stacking)
- The frame report and *Save result…* no longer copy the whole stacking result (about 1.2 GB for 24 MP) at every redraw or save.
- *Save result…* reports correctly whether it saved linear or stretched data, and a crash while saving is reported instead of hanging.
- Histograms are counted once per image instead of at every redraw.

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
- Video shapes: as the frames, 16:9, 3:2, 4:3, 1:1 square (all-sky fisheye) and 9:16 vertical (phones); resolution up to 4K (never scaled up), frame rate, play forward then back, hold the last frame.
- **Output:** MP4 (H.264) and WebM (VP9) through ffmpeg when it is installed (or next to the program); animated GIF and numbered TIFF / PNG / JPEG images built in.
- Runs in the background with a progress bar (time elapsed and left) and Cancel, then a report: frames, video size, frame rate and length, time taken, file and its size.
- **Projects** (`.tlproj`): the frames, looks, mask and every video setting; frames are found again after the folder moved.
- Up to eight frames at a time, fewer when memory is short (120 fisheye RAW frames: 56 s → 46 s).

### Night panoramas
- **Sky-only frames match by their stars.** Where corner features fail (frames near the zenith show nothing but stars), pairs are matched by triangles of neighbouring stars, compared by the angles between them, which neither the camera's turning nor the sky's changes. A 120-frame all-sky panorama that left a hole at the zenith (48 frames linked) now links every frame up to the zenith (107 frames; only the nadir shots of grass stay out).
- **A zoom lens that crept** between shots (pointed up, a zoom ring turns by its own weight; 17 → 15 mm in that set): RAW files give each photo's focal length, features are found at the same scale in all of them, and the refinement fits a focal length per zoom setting.
- **The turning sky:** from the capture times, the refinement measures how the sky turned between rows shot minutes apart (it found 14.9°/hour; the true rate is 15.04°), so star pairs and ground pairs agree. When rendering, the stars of every photo are shown as at one moment while the ground and the sky's glow stay in place, so stars meet across rows instead of doubling.
- Pairs of cameras pointing far apart (a row looking up and one looking down) were refused as "folding the image", because the far corners of one photo lie behind the other camera. They are now judged on the part the matches cover.

### Speed and memory
- **Big panoramas in less memory:** the full-size panorama is rendered in bands of rows when it does not fit in one piece and kept in the file's own format (6 bytes per pixel for 16-bit TIFF instead of about 50); photos read again at full size are kept in half precision. Smart blending renders each band with overlap, so the bands join seamlessly.
- Star stacking loads the calibration frames in parallel (they were read one by one) and loads and measures as many light frames at once as the free memory allows (it was at most 4). 60 frames of 24 MP with darks, flats and bias: 118 s → 81 s on a 28-thread PC, the same result bit for bit.
- Long jobs use at most 85 % of the CPU cores, and memory checks leave 15 % of the memory free, so the PC stays responsive.

### Languages and help
- The window in **English, Czech, German, Spanish, French or Polish**, switched in the top bar (the first start follows the system's language). Messages from the engine (progress steps, the process log) are still in English.
- The manual in all six languages (Spanish, French and Polish are new).
- **Help** (F1 or **? Help**): the manual inside the program, in the window's language, with contents, search, links between chapters and Back; F1 opens the chapter of the tab on show.

### Comfort
- **Drag and drop** images or folders onto the window (to the tab on show); dropped project files open in their tab.
- **Recent projects** in the top bar, for all three kinds of project.
- The tab, the stacking settings, the timelapse's video settings and the stitcher's RAW settings are remembered for the next session (not files, masks or looks). File dialogs start in the folder used last.
- **Show in folder** after saving a panorama, a stack, a video or a post-processed image.
- Progress bars no longer run off narrow panels: the message and times are written under the bar.

### Stability
- **RAW white balance in stacking:** *Average of image* and *Daylight* gave wrong colours with the built-in decoder: the grey-world average could miss the blue pixels entirely, and Daylight applied no white balance at all (the camera colour matrix was read from a field the RAW library no longer fills). Both now match LibRaw.
- **Alignment:** a frame whose star match was degenerate (the same few stars matched over and over) could be accepted with a folded, mirrored transform and smeared across the whole stack, tinting it (e.g. red from car lights). Alignments that flip the frame or scale it beyond 0.6–1.6× are now refused, and the frame is reported as failed.
- **Post-processing white balance on stacks:** with the automatic stretch measuring each colour on its own (the default for linear data), temperature, tint and a sampled grey were measured away and had almost no effect. They now act on top of the stretch.
- Sorting star distances and fit errors cannot crash on an invalid number (NaN) any more.

### Reports and times
- After stacking and after saving a panorama, a short report: photos / frames used, time taken, result size, file and its size, or what went wrong.
- The stacking progress bar shows the time elapsed next to the time left; the stitcher's status line shows the time elapsed while it works.

## v0.3.2

Bug fixes for star stacking.

- **RAW white balance in stacking:** *Average of image* and *Daylight* gave wrong colours with the built-in RAW decoder. The grey-world average could miss the blue pixels entirely, and Daylight applied no white balance at all, because the camera colour matrix was read from a field the RAW library no longer fills. Both now match LibRaw. *Camera* falls back to the daylight balance for files without one.
- **Alignment:** a frame whose star match was degenerate (the same few stars matched over and over) could be accepted with a folded, mirrored transform and smeared across the whole stack, tinting it (for example red from car lights). Alignments that flip the frame or scale it beyond 0.6–1.6× are now refused, and the frame is reported as failed.
- Sorting star distances and fit errors cannot crash on an invalid number (NaN) any more.

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

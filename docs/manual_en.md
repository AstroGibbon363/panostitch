# Panorama Stitcher: User Manual

*Other languages: [Česky](manual_cs.md) · [Deutsch](manual_de.md)*

A desktop program that joins overlapping photos into one panorama or mosaic:
JPEG, TIFF, PNG, camera RAW and astronomical FITS images. Every step runs only
when you press its button, and you can inspect and correct each result before
going on. A second tab stacks star images into a low-noise night sky or star
trails ([Star stacking](#13-star-stacking)).

---

## Contents

1. [Requirements](#1-requirements)
2. [Starting the program](#2-starting-the-program)
3. [The window at a glance](#3-the-window-at-a-glance)
4. [Quick start](#4-quick-start)
5. [Step by step](#5-step-by-step)
6. [Navigating the view](#6-navigating-the-view)
7. [Projections](#7-projections)
8. [RAW files](#8-raw-files)
9. [FITS files (astronomy)](#9-fits-files-astronomy)
10. [Saving](#10-saving)
11. [Troubleshooting](#11-troubleshooting)
12. [Tips for good panoramas](#12-tips-for-good-panoramas)
13. [Star stacking](#13-star-stacking)
14. [Use without the window](#14-use-without-the-window)
15. [Files](#15-files)

---

## 1. Requirements

- **Nothing to install.** The program is one file, `panostitch`
  (`panostitch.exe` on Windows), plus `panostack` for star stacking from the
  command line. On Linux it needs a graphics driver with OpenGL and X11 or
  Wayland, as every desktop has.
- **Camera RAW files** need nothing extra: a RAW decoder is built in.
  LibRaw's `dcraw_emu`, RawTherapee, darktable or dcraw are used too if you
  have them installed (see [RAW files](#8-raw-files)).
- **FITS and XISF files** need nothing extra.
- **File dialogs** need nothing extra on Windows and macOS. On Linux they use
  `zenity`, `kdialog` or `yad`, whichever is installed (most desktops have
  one). Without any of them, you type paths into the path box instead.

To build the program from its source code you need Rust
(<https://rustup.rs>): `cargo build --release` makes
`target/release/panostitch` and `target/release/panostack`.

---

## 2. Starting the program

Double-click `panostitch`, or start it from a terminal:

```
panostitch
```

You can also list photos or a project file to open straight away:

```
panostitch ~/Pictures/pano/*.NEF
panostitch my_pano.panoproj
panostitch night.stackproj          # opens in the Star stacking tab
```

Notes:

- The program uses all CPU cores automatically.
- The window opens within a second or two; nothing has to be prepared first.
- If no window appears, start the program from a terminal: error messages
  are shown there.
- The appearance switches (Dark, Modern, Simple) are kept in `ui.toml` in the
  settings folder: `~/.config/panostitch` on Linux,
  `~/Library/Application Support/panostitch` on macOS,
  `%APPDATA%\panostitch` on Windows.

---

## 3. The window at a glance

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ [Open…] [Save proj.] │ [Image+keypoints] [Pair matches]     │ 3 · Projection      │
│ Images               │ [Panorama] [Zoom +] [Zoom -] [Fit]   │  projection menu    │
│  Add Remove Clear ▲▼ │ (o) overlays  [Undo] [Redo]          │  Focal, Yaw, Pitch, │
│  path box            │                                      │  Roll               │
│  image list          │                                      │ 4 · Compose         │
│  Reference           │            main view                 │  Blend, Preview size│
│  RAW via, half size  │                                      │  Exposure,Brightness│
│  Preset              │                                      │  show unlinked      │
│ 1 · Features         │                                      │  Auto crop, Uncrop  │
│                      │                                      │  Nudge ← ↑ ↓ →      │
│ 2 · Matching         │                                      │ 5 · Stretch         │
│  Pairs, Model, Pair  │                                      │  histogram, sliders │
│  pair details        │  status line (red text)              │ [Preview] [Save…]   │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

- **Left column:** open and save projects, choose the images, find features, match the images.
- **Middle:** the main view, with buttons to switch views, zoom, hide the
  overlays, and undo or redo.
- **Right column:** projection, exposure, blending, brightness, manual
  placement, histogram stretch, preview and saving.
- **Red status line** (under the view): what the program is doing, results,
  and error messages.

The switches at the top right change the appearance: **Dark**, **Modern** (rounder,
roomier controls) and **Simple**. Simple mode hides the detailed
controls (feature detection and matching settings, blending, stretch,
nudging) and keeps what most panoramas need: the images, a preset, **Run all
steps**, the projection with focal length, yaw, pitch and roll, exposure,
**Even out**, crop, preview and saving. The hidden settings keep their preset
values. The program remembers the switches.

---

## 4. Quick start

**Photo panorama:**

1. Leave **half size** on. It is faster, and plenty for checking the result.
2. Choose a **Preset** that fits your subject (see [Presets](#50-presets)).
3. Type the folder of your photos into the path box and press **Enter**, or
   click **Add…** and select the photos. **The order does not matter.**
4. Click **Run all steps** and wait until the status line says *Done*.
5. Choose a **projection** on the right: **cylindrical** for a single row,
   **spherical** for several rows or anything reaching far up or down. Click
   **Auto exposure** if it is too dark, and adjust **Pitch** if the horizon is
   bent. Click **Auto crop** to cut away the ragged edges.
6. Click **Save full resolution…**. Click **Save project…** if you want to come
   back to it later.

**Deep-sky mosaic from FITS files:**

1. Choose the **deep-sky mosaic** preset.
2. Load the FITS tiles (stacked or single frames). They are stretched
   automatically for viewing.
3. Click **Run all steps**, then **Auto stretch** to fit the stretch to the
   whole mosaic, and adjust it if you like.
4. Save as **`.fits`** to keep the linear data for further processing, or as
   **`.tif`** to get the stretched picture you see.

---

## 5. Step by step

**Typing values:** every slider has a number box next to it. Click the box,
type a value and press **Enter**. The value snaps to the slider's step and
range.

### 5.0 Presets

The **Preset** menu (left column, under *RAW via*) sets all matching and
composing settings for a type of subject at once. Every value can still be
changed afterwards.

| Preset | For | Main settings |
|---|---|---|
| **day landscape** | daylight panoramas, camera rotated in place | light denoising, homography, cylindrical |
| **astro / night landscape** | night skies, Milky Way over a landscape | strong denoising, looser matching, spherical, +2 EV (then **Auto exposure**) |
| **deep-sky mosaic** | telescope mosaics of star fields, e.g. FITS | little denoising (stars are tiny), similarity model (shift + field rotation), no refinement, rectilinear; use **Auto stretch** |
| **macro / microscope** | a slide or stage moved under a fixed camera | translation model (frames only shift), no refinement, rectilinear |
| **scan / flat document** | maps, paintings, documents photographed in parts | affine model, no refinement, rectilinear |

### 5.1 Images

| Control | What it does |
|---|---|
| **Add…** | Opens a file dialog. You can select several files (Ctrl/Shift-click). |
| **Path box** | Type a file or **folder** path and press Enter. A folder loads all images in it, sorted by name. Files named `panorama…` (earlier results) are skipped. |
| **Image list** | Click an image to select it and show it. **Ctrl-click** adds or removes one image from the selection, **Shift-click** selects a range, and **Ctrl+A** (with the mouse over the list) selects all. Scroll with the mouse wheel. |
| **Remove** | Removes the selected images. The **Delete** key does the same while the mouse is over the list. |
| **Clear all** | Removes all images. |
| **▲ / ▼** | Moves the selected images up or down in the list. Only the *sequential* and *grid* pair modes care about the order. |
| **Reference** | The image the others are placed around; it stays undistorted. **auto (centre)** picks the image in the middle of the group, which usually gives the most even result. |
| **RAW via** | Which RAW converter to use; see [RAW files](#8-raw-files). Applies to images added afterwards. |
| **half size** | Decodes RAW files and colour-camera FITS files at half resolution: about 4× faster and ¼ of the memory. Applies to images added afterwards. Turn it off before loading if you want a full-resolution result. |

The list marks each entry:

| Mark | Meaning |
|---|---|
| blue line | selected; the darker one is the current image (for viewing, brightness and moving) |
| `R` | reference image |
| `4345kp` | number of features (keypoints) found |
| `UNLINKED` | the image could not be connected to the panorama (see [5.3](#53-step-2-matching)) |

Matching results are kept when you reorder images or remove other images, so
you do not have to match again.

### 5.2 Step 1: Features

Features (keypoints) are distinctive spots, such as corners, edges and stars,
that can be recognised in two overlapping images.

| Slider | Default | Meaning |
|---|---|---|
| **Detection size (px)** | 2000 | The image is shrunk to this size (longest side) before searching. Larger is more precise but slower. |
| **Max keypoints** | 5000 | Upper limit per image. Keypoints are spread evenly over the whole image. |
| **Corner threshold** | 0.001 | Minimum corner strength. Lower finds weaker corners (useful for sky or fog); higher keeps only strong ones. |
| **Denoise (σ px)** | 1.5 | Blurs away sensor noise before searching. Raise it (2–3) for **night shots** or noisy high-ISO photos; lower it (0.5–1) for star fields and clean daylight photos with fine detail. |

Click **Detect features**. The view shows the current image with the keypoints
as **yellow circles**. They should be spread over the parts of the image that
overlap other images. Only images whose settings changed are processed again.
Dark and linear images (night shots, FITS) are handled: detection adjusts the
contrast internally.

### 5.3 Step 2: Matching

Matching finds the same keypoints in two images. Then **RANSAC** keeps only
the matches that agree with a single geometric model (the *inliers*).

**Which pairs are tried (Pairs menu):**

| Pairs | Use when |
|---|---|
| **all pairs** | Default. Every image is tried against every other one, so **the order does not matter**, and several rows work automatically. Takes longer for many images (74 photos ≈ 35 s). |
| **sequential** | One row, images in shooting order. Each image is matched only with the next one in the list. Fastest. |
| **grid** | Several rows in a known order: **Grid columns** images per row, row after row. Neighbours to the right, below and diagonally are tried. Switch on **snake** if every second row was shot in the opposite direction. |

**Settings:**

| Control | Default | Meaning |
|---|---|---|
| **Grid columns** | 5 | Images per row (grid mode only). |
| **Ratio test** | 0.8 | How much better the best match must be than the second best. Lower is stricter (fewer, more reliable matches); higher gives more matches. |
| **Max distance** | 0.35 | How different two keypoints may look and still match. |
| **RANSAC tol. (px)** | 5.0 | How far (in pixels) a match may be from the model and still count as an inlier. |
| **Min. inliers** | 20 | Fewest inliers for a pair to count as a real overlap. |
| **Model** | homography | How one image maps onto another, see below. |
| **refine** | on | Global refinement, see below. |

**When a pair counts as a real overlap:** it needs at least *Min. inliers*
inliers, a clear majority of its matches must agree, and the transform must be
plausible (it must not fold or extremely stretch the image). This filters out
false matches between images that do not actually overlap.

**Models:**

| Model | Use when |
|---|---|
| **homography** | Normal panoramas: camera rotated in place. The default. |
| **affine** | Scans, or photos of a flat object from far away. |
| **similarity** | Only shifting, rotating and uniform scaling, e.g. telescope mosaics with field rotation. Stable when there are few matches. |
| **translation** | Only shifting, e.g. a scanner or a microscope stage. Most robust, least flexible. |

**Placement and global refinement (refine):** the images are linked through
their strongest pairs, starting from the reference image. With **refine** on
(homography model only), all pairs are then solved together as one rotating
camera. That spreads small errors evenly instead of letting them add up, and
closes the loops between rows; this removes most ghosting. It also estimates
the lens's barrel distortion (strong in wide-angle lenses) and corrects it when
rendering. Pairs that do not fit the common solution are **rejected
automatically**, because they are usually false matches or something that
moved. The status line reports how many were rejected and the **typical pair
error** (1–3 px is good).

**Buttons:**

- **Match**: matches the pairs chosen in the Pairs menu (in parallel), then
  places and refines.
- **Re-match pair**: matches only the pair shown, with the current settings.
- **Discard pair**: stops using the pair shown (for a match you can see is
  wrong); the placement is redone without it.
- **Run all steps**: features, matching, placement and preview in one go.

The **Pair matches** view shows two images side by side:

- **green lines**: inliers (correct matches)
- **red lines**: outliers. Hide them with the **outliers** switch.

The **Pair** menu lists the accepted pairs with their number of inliers.
Pairs marked *rejected* failed one of the checks; the text below the menu says
why.

**What good looks like:** dozens of green lines or more, all describing the
same shift, and a mean error of 1–2 px.

**Unlinked images:** an image with no accepted pair to the rest is marked
`UNLINKED` in the list and **left out of the panorama**, so it cannot spoil it.
Switch on **show unlinked images** (right column) to show such images in a row
below the panorama and place them by hand with **Ctrl+drag**.

### 5.4 Step 3: Projection

See [Projections](#7-projections) for what each one looks like.

| Control | Meaning |
|---|---|
| **Projection menu** | 14 projections, see [Projections](#7-projections) |
| **Focal (× auto)** | The focal length is estimated from the matches. This slider scales it. If the panorama looks squeezed or stretched sideways, try values between 0.8 and 1.2. |
| **Yaw (°)** | Turns the whole panorama left or right: chooses which direction is in the centre. Useful for 360° panoramas (e.g. to put the Milky Way core in the middle instead of at the edge). |
| **Pitch (°)** | Tilts the whole panorama up or down. Use it to **straighten a curved horizon** (for example if the camera was pointed down). |
| **Roll (°)** | Rotates the whole panorama. Use it to level a sloping horizon. |
| **Reset focal / yaw / pitch / roll** | Returns these four sliders to their defaults. |

The view title shows the panorama size, the focal length in pixels, the field
of view of the reference image, and the typical alignment error.

These controls update the preview immediately (once a preview exists).

### 5.5 Step 4: Compose

| Control | Meaning |
|---|---|
| **Blend** | **smart** (default): multi-band blending with deghosting. Fine detail comes from one image per spot, so there are no double images of people, lights or close objects. Brightness differences are blended over a wide zone, so there are no hard joins. Wherever overlapping images disagree (something moved), that whole area is taken from one image. **feather**: plain smooth mixing; shows ghosts where things moved. **seam**: each pixel from the most central image; no ghosts, but hard joins. **average**: plain mean; mainly for checking alignment. |
| **Preview size (px)** | Size of the preview image (longest side). Larger is sharper but slower. Zooming in renders detail anyway (see [Navigating the view](#6-navigating-the-view)). |
| **Exposure (EV)** | Brightens or darkens the whole panorama, from −3 to +8 EV (+1 EV = twice as bright). Dark astro panoramas often need +1.5 to +4. |
| **Brightness (sel.)** | Brightness factor of the **selected** images. |
| **show unlinked images** | Also shows images that could not be linked, in a row below the panorama, for placing by hand. |
| **Auto exposure** | Sets **Exposure** from the current preview: typical brightness to middle grey, without clipping more than the brightest 0.5 % (stars, lamps). |
| **Even out** | Computes per-image brightness factors that make the overlaps match. |
| **Reset moves** | Undoes all manual moves. |
| **Auto crop** | Cuts the panorama to the largest rectangle that is completely covered by images, so the black, ragged edges disappear. The preview then shows the cropped result, and **Save full resolution…** saves it. The status line says how much of the panorama is kept. Straighten the horizon (Pitch, Roll) first; a bent panorama leaves only a thin rectangle. |
| **Uncrop** | Removes the crop. The crop is also removed automatically when you change anything that moves the images (projection, focal, yaw, pitch, roll, moves, matching). |
| **Nudge ← ↑ ↓ →** | Moves the current image by one preview pixel (see below). |

In the **Panorama** view, every image's outline is drawn and numbered:

- **orange**: the current image
- **light blue**: other selected images
- **cyan**: the reference image
- **white**: the other images
- **dashed yellow lines**: the links used to place the images

Switch these off with **overlays** in the toolbar to see the panorama itself.

**Click inside an outline** to select that image (also works with overlays off).

**Moving images by hand:** hold **Ctrl** and drag an image, or use the arrow
keys or the **Nudge** buttons. The image moves together with the images
attached to it further from the reference (along the yellow links). The
reference image does not move; use Pitch and Roll to move everything.

### 5.6 Step 5: Stretch

Astronomical images (and dark RAW renderings) are usually **linear**: the
interesting detail sits in the darkest few percent of the brightness range and
the image looks almost black. The histogram stretch makes it visible.

| Control | Meaning |
|---|---|
| **Histogram** | Brightness distribution of the panorama preview (in the Panorama view) or of the current image (in the other views), on a logarithmic scale. The **blue line** marks the black point, the **white line** the white point, and the **orange curve** shows how input brightness (left to right) maps to output brightness (bottom to top). |
| **Black point** | Everything at or below this brightness becomes black. Set it just below the sky background (the left edge of the histogram's main peak). |
| **Midtones** | Brightens (smaller values) or darkens (larger values) the middle tones without clipping. 0.5 is neutral. Deep-sky images typically need 0.005–0.05. |
| **White point** | Everything at or above this brightness becomes white. |
| **Auto stretch** | Sets all three from the current histogram: black point just below the sky background, midtones so the background ends up at about 25 % grey (the same idea as PixInsight's automatic screen stretch). |
| **Reset stretch** | Back to no stretch (0 / 0.5 / 1). |

- The stretch changes only how images are **shown** and how the panorama is
  saved as TIFF, PNG or JPEG. Matching works on the original data either way.
- **FITS files** get an automatic stretch for viewing when they are loaded,
  based on all loaded FITS images together.
- Changing the stretch is instant: nothing has to be recomputed.
- **Exposure** is applied before the stretch.

### 5.7 Undo, redo and projects

**Undo / redo:** **Undo** and **Redo** in the toolbar, or **Ctrl+Z** and
**Ctrl+Y** (also **Ctrl+Shift+Z**). The status line says what was undone, e.g.
*"Undone: Pitch (°) 0 → 10"*.

- Undoable: every slider, menu and switch of the settings; adding, removing
  and reordering images; matching results (Match, Re-match, Discard); the
  reference image; manual moves; brightness; auto crop.
- A slider drag counts as **one** step, however long you drag. Changes to two
  different controls are always two steps.
- Removed images are kept in memory for undo, so they come back instantly,
  together with their matching results.
- The last 100 steps are kept. Not undoable: view-only things (zoom, which
  view is shown, overlays), *RAW via* and *half size*, and saving files.

**Projects:** **Save project…** (top of the left column) writes a project file
with the extension **`.panoproj`**. **Open project…** loads one again. You can
also type the path of a `.panoproj` file into the path box and press Enter,
or start the program with it: `panostitch my_pano.panoproj`.

A project stores:

- the images (full path, and the path relative to the project file)
- all settings: sliders, menus, switches, including the stretch
- the matching results, so nothing has to be matched again
- manual moves, brightness factors, the reference image and the crop
- *RAW via* and *half size*, so the images are loaded the same way again

It does **not** contain the images themselves; they are loaded from their
files. If you move the photos together with the project file (e.g. the whole
folder), they are still found through the relative path. Missing files are
reported and left out.

The file is readable text (TOML format). Its size is mostly the stored match
points: about 50 kB per image.

---

## 6. Navigating the view

| Action | Mouse | Keyboard | Button |
|---|---|---|---|
| Pan | drag with **left** button | arrow keys (Image and Pair views) | |
| Zoom in | scroll up | `+` | **Zoom +** |
| Zoom out | scroll down | `-` | **Zoom -** |
| Reset zoom | | `0` or `Home` | **Fit** |
| Select image (Panorama view) | click inside its outline | | |
| Show / hide outlines, numbers, links | | | **overlays** switch |
| Move image (Panorama view) | **Ctrl + drag** | arrow keys (`Shift` = 10 steps) | **← ↑ ↓ →** |
| Undo | | `Ctrl+Z` | **Undo** |
| Redo | | `Ctrl+Y` or `Ctrl+Shift+Z` | **Redo** |

The **Image + keypoints**, **Pair matches** and **Panorama** buttons switch
between the three views.

**Sharp zoom:** the preview is a reduced image. When you zoom into the
panorama and pause for a moment, the visible area is re-rendered at screen
resolution from the original images, so you can check fine detail and joins.

---

## 7. Projections

A panorama covers more than a flat photo can show, so the program must
*project* the view onto a flat image. Each projection distorts something:

| Projection | Looks like | Best for | Limits |
|---|---|---|---|
| **rectilinear** | an ordinary photo: straight lines stay straight | narrow panoramas (up to ~100° wide), architecture, telescope mosaics | edges get extremely stretched as the view widens |
| **cylindrical** | horizontal straight lines bend, verticals stay vertical | most horizontal panoramas | not for tall views (top and bottom stretch) |
| **spherical** (equirectangular) | like cylindrical, with less vertical stretching | very wide or 360° panoramas, panorama viewers | straight lines curve |
| **mercator** | between cylindrical and spherical | wide panoramas with some height | stretches near top and bottom |
| **stereographic** | fisheye-like, keeps shapes round | very wide fields of view, creative shots | strong curvature |
| **transverse_cylindrical** | cylindrical turned on its side: vertical lines bend, horizontal lines stay straight | **vertical panoramas** (horizon to zenith), the Milky Way arch standing up | not for wide horizontal sweeps (they wrap over the top and bottom edges) |
| **transverse_mercator** | like transverse cylindrical, less stretched sideways | vertical panoramas with some width | as above |
| **fisheye** (equidistant) | round image, distance from the centre proportional to the angle | **all-sky views**: tilt with Pitch so the zenith is in the centre | edges compressed |
| **fisheye_equisolid** | round image that keeps areas in proportion | all-sky views, the way most fisheye lenses look | edges compressed more |
| **little_planet** | the ground in the centre, the sky around it as a ring | 360° panoramas with ground, creative | needs a panorama that reaches far down |
| **tunnel** | the sky in the centre, the horizon around it | 360° night skies, star trails, trees from below | needs a panorama that reaches far up |
| **orthographic** | a globe seen from outside | creative; shows one half of the sphere | the edges are very compressed |
| **hammer** (Hammer–Aitoff) | an oval map of the whole sphere with correct relative areas | full-sphere panoramas shown as one image, all-sky star maps | shapes distorted near the edges |
| **mollweide** | an oval equal-area map, flatter at the poles | as Hammer; the classic all-sky map of astronomy | shapes distorted near the edges |

If the horizon looks like a smile or a frown, adjust **Pitch**. If it is
tilted, adjust **Roll**. **Yaw** turns the view sideways. For the fisheye,
little planet and tunnel views, Pitch decides what is in the centre.

---

## 8. RAW files

Supported: **Canon CR2 and CR3, Nikon NEF and NRW, Sony ARW and SRF, DNG, Fuji
RAF, Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW**, and others. Exact
camera support depends on the decoder; the built-in one knows most cameras of
the last two decades, LibRaw (if installed) about 1250.

**RAW via** chooses the decoder:

| Option | Decoder | Character |
|---|---|---|
| **auto** | the first that works, in the order below; RawTherapee first if the photo has a `.pp3` file | recommended |
| **built-in** | the program's own decoder | always available, fast; neutral rendering with the camera's colours and white balance and no automatic brightening, so all frames keep the same exposure |
| **libraw** | `dcraw_emu` (LibRaw) | similar to built-in; for cameras the built-in decoder does not know |
| **rawtherapee** | `rawtherapee-cli` | **uses your RawTherapee edits** saved in `photo.NEF.pp3`; otherwise RawTherapee's default look |
| **darktable** | `darktable-cli` | darktable's default rendering (including lens correction where available); slowest |
| **dcraw** | `dcraw` | fallback; cannot read CR3 |

Only built-in is always available; the others appear when the program is
installed on your computer. Star stacking needs linear data, so it uses only
built-in, libraw or dcraw.

The external decoders write a 16-bit image into a temporary folder that is
deleted afterwards. Nothing is written next to your photos.

**Tip:** for consistent colours across the panorama, use the same edits (the
same `.pp3` profile) for all frames, or use **built-in** and correct the
colours afterwards.

---

## 9. FITS files (astronomy)

Files ending in **`.fits`, `.fit` or `.fts`** are read directly, with no extra
software. Supported:

| Content | Examples | How it is read |
|---|---|---|
| **8-, 16-, 32- and 64-bit integer, 32- and 64-bit float** | camera captures, stacking results | scaled to 0…1 (16-bit unsigned: 0…65535) |
| **Colour (3 planes)** | stacked results from Siril, PixInsight, DSS | as RGB |
| **Mono** | mono cameras, single filters | as grey |
| **Colour camera raw (Bayer)** | frames from ZWO, ToupTek, QHY… with a `BAYERPAT` header (RGGB, BGGR, GRBG, GBRG) | debayered; with **half size** one colour pixel per 2×2 cell (fast, clean), otherwise interpolated at full size |
| **Row order** | `ROWORDER` = `TOP-DOWN` or `BOTTOM-UP` (the FITS default) | the image is shown the right way up |

Not supported: **tile-compressed files (`.fz`)**. Decompress them first, e.g.
with `funpack`. Extra parts of a file (tables, ICC profiles) are ignored; the
first image is used.

**Notes for deep-sky mosaics:**

- Use the **deep-sky mosaic** preset. Telescope tiles only shift and rotate
  (field rotation), which the *similarity* model describes exactly.
- **Stacked, calibrated tiles** stitch best. Single short frames have few
  visible stars and a lot of noise.
- Raw colour-camera frames are not white-balanced, so they look green; the
  stretch does not change colour balance.
- Save as **FITS** to continue processing (background extraction, colour
  calibration, final stretch) in Siril or PixInsight. The FITS output keeps the
  linear values.

---

## 10. Saving

Click **Save full resolution…** and choose a file name. The file type follows
the extension:

| Extension | Result |
|---|---|
| `.tif` / `.tiff` | 16 bits per channel, best for further editing; saved **as shown** (with exposure and stretch) |
| `.png` | 16 bits per channel; as shown |
| `.jpg` | 8 bits per channel, smaller file; as shown |
| `.fits` / `.fit` / `.fts` | 32-bit float, three colour planes; **linear** (exposure and brightness factors applied, but **no stretch**, no clipping), for astronomy software |

When all loaded images are FITS, the suggested file name ends in `.fits`.

The panorama is rendered from the loaded images at their full size: half the
camera resolution if **half size** was on when you loaded them.

Panoramas wider or taller than 40,000 px are refused. This almost always means
that one pair is matched wrongly or that the projection is unsuitable; try
cylindrical or spherical.

---

## 11. Troubleshooting

| Problem | What to try |
|---|---|
| **No window appears** | Start the program from a terminal and read the error message there. On Linux it needs OpenGL: check that the graphics driver is installed. |
| **"Busy – wait…"** | A step is still running; wait for the status line to change. |
| **An image is skipped while loading** | The status line names the file and the reason. For RAW, try another converter in **RAW via**. For `.fz` FITS, decompress the file first. |
| **Panorama is mostly black** | Photos: click **Auto exposure** or raise **Exposure (EV)** (night shots often need +1.5 to +4), or edit the photos in RawTherapee (their `.pp3` files are then used). FITS and other linear data: click **Auto stretch**. |
| **FITS images look black or white after loading** | The automatic stretch on loading is based on all FITS images; one very over- or underexposed frame can throw it off. Remove such frames, then click **Auto stretch** (or **Reset stretch** and set it by hand). |
| **FITS images look green** | Raw colour-camera frames are not white-balanced. Stack and colour-calibrate them first, or correct the colour of the result in astronomy software. |
| **Few yellow keypoints** | Lower the **Corner threshold**; raise **Max keypoints**; raise **Detection size**. |
| **Keypoints mostly on grass, sky or noise** | Raise **Denoise** to 2–3. |
| **Images marked UNLINKED** | Look at them: do they really overlap another image? Raise **Ratio test** (0.85–0.9) and **Max distance** (0.4), lower **Min. inliers**, raise **Denoise**, then **Match** again. If it still fails, switch on **show unlinked images** and place them with **Ctrl+drag**. |
| **Clouds, water, or other moving things** | They change between shots, so they cannot be matched reliably (frames showing only clouds usually stay unlinked). Shoot such parts quickly one after another, or place them by hand. |
| **A wrong pair is used** (lines in all directions, images in the wrong place) | Select it in the **Pair** menu and click **Discard pair**. With **refine** on, most such pairs are rejected automatically. |
| **Ghosting (doubled objects)** | Keep **refine** on and use **Blend → smart** (default). Remaining problems come from things that changed between shots (e.g. a torch switched on in only one photo), or from **parallax**: close objects shift against the background when the camera is not rotated around its lens (the "nodal point"). |
| **Visible brightness steps** | **Even out**, then fine-tune per image with **Brightness (sel.)**. |
| **Horizon curved** | Adjust **Pitch** (and **Roll** if tilted). |
| **Strongly stretched top or bottom** | Use the **spherical** projection; the status line suggests it when the panorama reaches far up or down. |
| **Panorama squeezed or stretched** | Adjust **Focal (× auto)**. |
| **Huge, distorted panorama** | A wrong pair (check the pairs), or use **spherical** / **cylindrical** instead of rectilinear. |
| **Slow or out of memory** | Turn on **half size** before loading; lower **Preview size**. Full-resolution images need roughly 12 bytes per pixel each, about 300 MB for a 24 MP image. With many images, *all pairs* matching takes longer; *grid* is faster when you know the layout. |
| **Auto crop keeps only a thin strip** | The panorama is bent. Straighten it with **Pitch** and **Roll** (and choose a suitable projection) first, then **Auto crop** again. |
| **A project opens without some images** | The status line lists the missing files. They were moved or renamed, and not together with the project file. Put them back, or add them again and match. |
| **Undo does nothing** | Undo works on settled changes; wait a moment after the last change. While a step runs (*Busy*), undo waits. |
| **Everything is slow** | Turn on **half size** before loading (RAW and colour-camera FITS); lower **Preview size**. In the stacking tab, **Half size** under RAW conversion. |

---

## 12. Tips for good panoramas

- **Overlap** neighbouring shots by 30–50 %, also between rows. For telescope
  mosaics, 15–25 % is usually enough.
- **Rotate the camera** around the lens, not around yourself. Use a tripod if
  you can, ideally with a panoramic head.
- **Lock exposure, white balance and focus** (manual mode), so all frames match.
- Avoid moving objects in the overlap areas, or shoot them in one frame only.
- **Keep the camera level.** A tilted camera bends the horizon (fix with
  Pitch and Roll).
- For long series, check the result with **half size** on first.
- Do not save the result into the folder of the source images if you load
  images by folder. Files named `panorama…` are skipped, but other names are
  not.

---

## 13. Star stacking

The second tab, **Star stacking (landscape / trails)**, combines many
exposures of the same scene into one image: a low-noise sky with sharp stars,
or star trails. It is made for landscape astrophotography: the camera stands
on a tripod, the sky moves between the frames, the foreground does not.

It works like Sequator, with what Sequator lacks: bias and flat-dark frames,
32-bit linear output, a separate foreground image, star trails without gaps,
automatic sky detection, and no black box: every frame is measured and scored,
a chart shows the measures, and a log lists each step with what it did and how
long it took.

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ input list           │ [Base image] [Result] [Frame report] │ Post-processing     │
│  Star images, Base,  │ [Fit] [Open project…] [Save project…]│  Background extr.   │
│  calibration, Fore-  │                                      │  Color calibration  │
│  ground, Output      │                                      │                     │
│ Add Remove Clear     │   base image (draw the sky here),    │  Colour noise       │
│ Use as base          │   result, or frame report            │  Light pollution    │
│ settings list        │                                      │  Brightness, Tone   │
│ options of the       │                                      │  Color space        │
│ highlighted entry    │                                      │  [Save result…]     │
│                      │                                      │  [Use in Stitcher]  │
│ [Quick prev.] [Start]│  status line                         │                     │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

**Click an entry** in the input list or the settings list: its options and an
explanation appear below the lists.

### 13.1 Quick start

1. Highlight **Light frames**, click **Add…** and choose the star shots (or
   type a file or folder path into the path box and press Enter).
2. Highlight **Sky region**, choose **Boundary line** and drag a line along the
   horizon in the base image view, or click **Detect sky** and correct the
   mask with the brush. Stars are searched for only in the sky.
3. Click **Quick preview**: a small, fast result from up to 10 frames, to check
   the settings. Nothing is saved.
4. Highlight **Output** and choose where to save (default: `stacked.tif` next
   to the first star image), then click **Start**.
5. Adjust the post-processing on the right; the result updates at once. Click
   **Save result…** to save it again with the new settings.

### 13.2 Input files

| Entry | What to shoot |
|---|---|
| **Light frames** (*Star images* in simple mode) | The shots of the sky: same focal length, aperture and exposure. RAW is best; TIFF, FITS or XISF only if the data is still linear (not edited). **Unify exposure** evens out frames taken under different conditions. |
| **Base frame** (*Base image* in simple mode) | The frame the others are aligned to. Empty = the middle light frame. Choose another in the dropdown next to **Base frame**, or pick a light frame in its dropdown and click **Use as base**. |
| **Dark frames** (*Noise images* in simple mode) | Dark frames: lens cap on, same ISO and exposure time, taken just before or after the star images. Removes thermal noise and hot pixels. |
| **Bias frames** (*Bias images* in simple mode) | Shortest exposure, lens cap on, same ISO. |
| **Flat field frames** (*Vignetting images* in simple mode) | Flat frames: an evenly lit white surface, same focal length and aperture. Corrects darker corners and dust shadows. |
| **Flat-dark frames** (*Flat-dark images* in simple mode) | Darks for the flats: same ISO and exposure time as the vignetting images. |
| **Foreground image** | Optional: one shot of the ground from the same tripod position – at blue hour, a long exposure or low ISO. It replaces the (noisy, dark) stacked foreground below the sky region. **Match brightness** scales it to the stacked ground; **Exposure (EV)** brightens or darkens it from there. Needs a sky region. |
| **Output** | `.tif` (16-bit), `.jpg` (8-bit), or `.fits` / `.xisf` (32-bit float, linear, for further processing in Siril, PixInsight etc.). |

All calibration frames are optional. Even a few dark frames help a lot at
high ISO.

**The file lists:** each group shows its files in a dropdown next to its
name, so hundreds of light frames need no scrolling. Choose a file there to
see it in the preview; **Remove** and **Use as base** act on that file,
**Clear** empties the whole group.

**Noisy flats** (too few, or too dark in one colour, e.g. a blue light source)
are smoothed automatically; the process log says so, and how noisy they were.
Otherwise their noise would be stamped onto every frame and turn into fine
streaks as the stars drift.

**Temporary files:** none are written for intermediate images. The frames are
kept in memory, or, if they take more than half of the free memory, in one
temporary file in the output folder, which is deleted as soon as the run ends
(it is not even visible while it runs). The process log says which. If there
is not enough room for it, the run stops with a message before it starts:
switch on **Half size** (a quarter of the space) or save the output on a disk
with more free space. Masters (dark, bias, flat) are not saved either.

### 13.3 Settings

| Setting | What it does |
|---|---|
| **Composition** | **Align stars**: the frames are aligned on the stars and combined. **Accumulation** averages them; **Select best pixels** (default) drops outliers (planes, satellites); **Freeze ground** aligns only the sky, so sky and foreground both stay sharp (needs a sky region); **Align only** just writes the aligned frames. **Star trails**: no alignment, the sky keeps the brightest value of every pixel. **Trail fade**: **Fade in** (the trails start faint, Sequator's motion effect), **Fade out**, or **Both ends** (a comet-like taper). **Fill gaps between frames**: the short breaks of the camera's interval are filled along the measured star motion, so the trails are continuous lines instead of dashes. |
| **Sky region** | Where the sky is. **All sky**: the whole frame is sky (no foreground, e.g. telescope or deep-sky shots). **Boundary line**: drag a line along the horizon. **Gradient**: the line plus a soft transition. **Irregular mask**: paint the sky with the left mouse button, erase with the right one; **Brush size** (slider or mouse wheel). **Detect sky** paints a first mask from the brightness and the stars – approximate, so check it and correct it with the brush. **Mask feather** softens the mask's edge, inside the sky only, so the foreground gets none of the sky (no halos along the horizon). **Show detected stars** circles the stars the last run found in the shown frame. |
| **Sigma clipping** | How strictly outliers are removed (low / high × σ). 2–3 is usual; lower removes more but leaves more noise. Needs at least 4 frames, works best with 10+. |
| **Frame rejection** | Leaves out frames clearly worse than the typical one: clouds or dew (dimmer stars), few stars, blurred or elongated stars (focus, wind, a bumped tripod). |
| **RAW conversion** | White balance (camera, image average, daylight), demosaicing (AHD by default; DHT or AAHD often give less colour noise at high ISO) and **half size** (4× fewer pixels, much faster). |
| **Reduce dynamic noises** | Finds and removes hot pixels without dark frames. |
| **Reduce distor. effects** | Corrects lens distortion that leaves short trails in the corners of long sequences: **Lens**, **Complex** (ultra wide-angle lenses, refraction near the horizon) or **Auto**. |
| **Merge 4 pixels** | Adds up 2×2 pixels: brighter and less noisy, half the width and height. |
| **Time-lapse** | Stacks every N consecutive frames into one, giving a numbered sequence of low-noise frames in a folder next to the output. |

### 13.4 The views

- **Base image**: the frame the others are aligned to; the sky region is drawn
  here. Choose a file in a group's dropdown to see it.
- **Result**: the stack after post-processing. Drag to pan, mouse wheel to
  zoom, **Fit** to see all of it.
- **Frame report**: a chart of every measure per frame, each divided by its
  median, so a bad frame stands out (red = left out, blue = the base image);
  below it one line per star image: stars found, star size (FWHM),
  elongation, sky level, light (brightness of the stars compared with the
  base image), alignment error, whether the frame was used and why not.
  Under the table, the **process log**: every step of the run, what it did
  (calibration, sky region, frames used, distortion, gap filling, foreground,
  post-processing) and how long it took. **Click a frame** in the chart or
  the table to exclude it (or include it again), then click **Start** again.

### 13.5 Post-processing

Applied to the stacked result at once; **Start** saves with these settings,
**Save result…** saves again.

| Setting | What it does |
|---|---|
| **Background extraction** | Subtracts a smooth model of the sky background (gradients from light pollution or moonlight). |
| **Color calibration** | Makes the sky background neutral grey and the average star white. |
| **Reduce colour noise** | Smooths only the colours, removing the coloured speckles of high-ISO shots. |
| **Light pollution** | **Deep sky**: removes a smooth glow. **Uneven**: follows the glow tile by tile (domes of city light in wide-angle shots). **Strength** sets how much is removed. Correct the vignetting first (vignetting images), or it gets worse. |
| **Enhance star light** | Lifts the stars above the sky background without boosting its noise. |
| **Auto brightness** | Puts the sky at a pleasant level without clipping the brightest stars. |
| **High dynamic range** | Compresses bright stars and lights instead of clipping them. |
| **Tone** | **sRGB curve** (as a camera does), **Stretch** (black point, midtones, white point; automatic or by hand) or **Linear** (no curve). |
| **Color space** | sRGB or Adobe RGB for TIFF and JPEG output. |
| **Linear (for processing)** | Switches everything off and saves a linear 32-bit FITS: what Siril or PixInsight expect. |
| **Use in the Stitcher** | Adds the saved result to the Panorama stitcher tab, as one panel of a panorama (see 13.7). |

### 13.6 Projects

**Save project…** writes a `.stackproj` file with the file lists, the sky
region and every setting. **Open project…** (or `panostitch
night.stackproj`) brings it back. Files are found by their full path, or
relative to the project file if the folder was moved.

### 13.7 Panoramas of stacked panels

A Milky Way panorama with low noise: shoot several frames at each position of
the panorama (with overlap between the positions), then:

1. Stack the frames of the first position (Start), then click **Use in the
   Stitcher**.
2. Replace the star images with those of the next position, give the output a
   new name, Start, **Use in the Stitcher** again; and so on.
3. In the Panorama stitcher tab, **Run all steps** joins the stacked panels.

Save each panel as `.tif`; the same post-processing for all panels keeps their
brightness and colours alike.

### 13.8 Tips

- Shoot at least 10–20 star images with the same settings, plus 10–20 noise
  images right after, without moving the camera.
- Set the sky region carefully: trees and buildings that are counted as sky
  are blurred by the alignment. **Detect sky** is a quick start, not the
  last word.
- Look at the Frame report after a Quick preview: the chart shows at once
  which frames are worse (clouds, wind, dew).
- Use **Freeze ground** when the foreground matters, **Select best pixels**
  when it does not.
- For processing in other programs, save as `.fits` and switch on **Linear
  (for processing)**.
- From a terminal, `panostack` stacks without the window; `panostack --help`
  lists its options (see [Use without the window](#14-use-without-the-window)).

---

## 14. Use without the window

**Star stacking from a terminal:** `panostack` runs the same stacking as the
Star stacking tab. `panostack --help` lists all options. Examples:

```
panostack --mode freeze --horizon 0.62 --darks darks/ -o night.tif lights/
panostack --mode trails --fade in --fill-gaps --auto-sky -o trails.jpg lights/
panostack --preview lights/            # quick check, nothing saved
```

The frame report and the process log are printed at the end.

**Stitching from your own Rust program:** add PanoStitch as a dependency
(`panostitch = { path = "…/PanoStitch" }` in your `Cargo.toml`), then:

```rust
use panostitch::io;
use panostitch::stitcher::{self, Projection, StitchParams};

fn main() -> Result<(), String> {
    let paths = vec!["a.NEF".to_string(), "b.NEF".into(), "c.NEF".into()];
    let p = StitchParams { projection: Projection::Spherical, ..Default::default() };
    let pano = stitcher::stitch_files(&paths, &p, true, &mut |m| println!("{m}"))?;
    io::save_panorama("pano.tif", &pano)
}
```

`examples/scripted.rs` is a complete example
(`cargo run --release --example scripted -- a.NEF b.NEF c.NEF`).
`stitch_files(paths, params, raw_half, log)` loads and stitches;
`stitch(images, params, log)` stitches images already in memory.
The fields of `StitchParams`, with their defaults:

| Field | Default | Meaning |
|---|---|---|
| `layout` | Layout::All | which pairs to match: `All`, `Sequential`, `Grid` |
| `cols`, `snake` | 0 (all images), `false` | grid layout |
| `refine` | `true` | global refinement |
| `feature_scale` | `0.5` | detection scale |
| `nkeypoints` | `5000` |  |
| `denoise` | `1.5` |  |
| `ratio` | `0.8` |  |
| `max_distance` | `0.35` |  |
| `model` | `Model::Homography` | `Homography`, `Affine`, `Similarity`, `Translation` |
| `ransac_thresh` | `5.0` |  |
| `min_inliers` | `20` |  |
| `projection` | `Projection::Cylindrical` | any of [Projections](#7-projections), e.g. `Fisheye`, `LittlePlanet`, `Mollweide` |
| `blend` | `Blend::Smart` | `Smart`, `Feather`, `Seam`, `Average` |
| `sharpness` | `3.0` | feather mode: higher = narrower transitions |

**FITS mosaic with a stretched TIFF and a linear FITS result:**

```rust
use panostitch::stitcher::{Model, Projection, StitchParams};

let p = StitchParams { model: Model::Similarity, refine: false,
                       projection: Projection::Rectilinear, denoise: 0.75, ..Default::default() };
let mosaic = stitcher::stitch_files(&tiles, &p, false, &mut |_| {})?;
io::save_panorama("mosaic.fits", &mosaic)?;                          // linear
let (black, mid, white) = stitcher::auto_stretch(&mosaic, 0.25, 2.8);
io::save_panorama("mosaic.tif", &stitcher::hist_stretch(&mosaic, black, mid, white))?;
```

Star stacking from code: fill a `stacker::StackConfig` (start from
`StackConfig::default()`) and call `stacker::stack(&config, false, &progress, None)`.

---

## 15. Files

| File | Purpose |
|---|---|
| `panostitch` (`panostitch.exe`) | The program (the window). |
| `panostack` (`panostack.exe`) | Star stacking from the command line (`panostack --help`). |
| `*.panoproj`, `*.stackproj` | Your project files (wherever you save them). |
| `ui.toml` in the settings folder | The appearance switches (see [Starting the program](#2-starting-the-program)). |
| `docs/manual_en.md` | This manual (English). |
| `docs/manual_cs.md`, `docs/manual_de.md` | The manual in Czech and German. |

Source code (to build the program yourself):

| Path | Contents |
|---|---|
| `Cargo.toml` | The project and the libraries it uses. |
| `src/image.rs`, `src/io/` | Images; loading and saving (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Stitching: features, matching, placement, refinement, projections, blending. |
| `src/stacker/` | Star stacking. |
| `src/gui/` | The window: both tabs and the file dialogs. |
| `src/bin/` | The two programs, `panostitch` and `panostack`. |
| `tests/` | Tests: stacking on synthetic night scenes, stitching, file formats, and both tabs of the window. Run them with `cargo test --release`. |
| `examples/scripted.rs` | Using the stitcher from your own Rust code. |

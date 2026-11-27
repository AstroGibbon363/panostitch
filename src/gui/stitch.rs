//! The "Panorama stitcher" tab. Every step runs only when you press its
//! button, and each result can be inspected (keypoints, matches with inliers /
//! outliers, placed outlines and links) and overridden (pairs, reference
//! image, projection, horizon, nudges, brightness).
//!
//! View: left-drag pans, wheel zooms. Panorama view: click an image to select
//! it; Ctrl + drag or the arrow keys (Shift = ×10) move it with the images
//! attached beyond it.

use crate::gui::i18n::tr;
use super::dialogs::{self, Picked};
use super::{combo, image_view, slider, texture, View};
use crate::image::{luminance, quantiles, resize, Bits, HalfImg, Img, Source};
use crate::io::{self, basename, LoadOptions, RawDecoder, ShotInfo};
use crate::stitcher::*;
use egui::{Color32, Pos2, Stroke};
use rayon::prelude::*;
use crate::sys::{available_disk, available_memory, gb, memory_reserve};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

const DISP_MAX: usize = 1600;
/// In a panorama narrower than `AUTO_HORIZON_NARROW` (°), Auto horizon turns
/// the roll by at most `AUTO_HORIZON_MAX` (°): there the edge of the sky shows
/// a tilt only by its slope, and a hillside or a forest on a slope slopes too.
/// A wide one shows the tilt as the bow of its horizon, which a slope does not make.
const AUTO_HORIZON_MAX: f64 = 8.0;
const AUTO_HORIZON_NARROW: f64 = 90.0;
/// Marks images left out because the memory was almost full.
const LOWMEM: &str = "\u{1}lowmem:";

#[derive(Clone)]
pub struct Item {
    pub path: String,
    pub img: Arc<Img>,
    pub disp: Arc<Img>,
    pub f: f64,
    pub feats: Option<Arc<Features>>,
    pub featkey: Option<FeatKey>,
    pub gain: f64,
    pub offset: [f64; 2],
    /// Full (sensor) size ÷ loaded size: 2 for a RAW loaded at half size, else 1.
    pub native: f64,
    /// Focal length and capture time from the camera.
    pub shot: ShotInfo,
    /// Stars in the sky part (found when needed for matching by stars).
    pub stars: Option<Arc<Vec<Pt>>>,
    /// Where it shows sky, and its background (found when needed for rendering a turning sky).
    pub sky_map: Option<Arc<SkyMap>>,
    /// Source mask: where this image must be used (painted on the panorama).
    pub mask: Arc<Vec<MaskDab>>,
    /// Plate solving (photos of the sky only); None = not tried yet.
    pub solved: Option<Solved>,
    /// This photo's own corrections, set by hand.
    pub corr: Correction,
}

/// Corrections of one photo set by hand, on top of what the refinement found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Correction {
    /// Turn about the photo's own centre (°, clockwise).
    pub roll: f64,
    /// Added to the lens distortion found for it (k1·r² + k2·r⁴).
    pub k1: f64,
    pub k2: f64,
}

impl Correction {
    pub fn is_none(&self) -> bool {
        *self == Correction::default()
    }
}

/// What plate solving made of a photo.
#[derive(Clone, Debug)]
pub enum Solved {
    /// Too much landscape in it.
    NotSky,
    Failed(String),
    Found(Arc<solve::Solution>),
}

impl Item {
    /// The camera's record of the shot, with the focal length measured by plate solving.
    pub fn shot_info(&self) -> ShotInfo {
        let mut s = self.shot;
        s.focal_px = self.solved.as_ref().and_then(|s| s.found()).map(|s| s.f);
        s
    }
}

impl Solved {
    fn found(&self) -> Option<&Arc<solve::Solution>> {
        match self {
            Solved::Found(s) => Some(s),
            _ => None,
        }
    }
}

pub type FeatKey = (usize, usize, u64, u64);

#[derive(Clone)]
pub struct PairRes {
    pub i: usize,
    pub j: usize,
    /// Matched points (shared: the undo history keeps many copies of the pairs).
    pub p1: Arc<Vec<Pt>>,
    pub p2: Arc<Vec<Pt>>,
    pub h: M3,
    pub inl: Arc<Vec<usize>>,
    pub ok: bool,
    /// Shown in the Pair menu (accepted, or matched on purpose).
    pub listed: bool,
    pub msg: String,
    /// Matched by star patterns (the sky may have turned between the two shots).
    pub stars: bool,
    /// Control points placed by hand: kept by a new matching run and never
    /// rejected by the global refinement.
    pub manual: bool,
}

#[derive(Clone)]
pub struct Placement {
    pub parent: Vec<Option<usize>>,
    pub order: Vec<usize>,
    pub comp: Vec<usize>,
    pub ref_: usize,
    pub members: Vec<usize>,
    pub g: Vec<M3>,
    pub f: f64,
    /// Each image's lens distortion as the refinement found it.
    pub lenses: Vec<Lens>,
    pub typical: Option<f64>,
    /// Turning of the sky found from the star pairs (°/hour), if any.
    pub sky_rate: Option<f64>,
    /// The sky's turning, to show all stars as at one moment (None: not known).
    pub sky: Option<SkyMotion>,
}

/// How the sky turned while the panorama was shot.
#[derive(Clone, Debug)]
pub struct SkyMotion {
    /// Axis × rad/s, in the reference image's frame.
    pub w: [f64; 3],
    /// Each image's capture time minus the common moment (s); NaN: show it as shot.
    pub dt: Vec<f64>,
}

/// The sky alignment of a render, before the missing sky maps are found.
struct SkyJob {
    g: Vec<Option<M3>>,
    maps: Vec<Option<Arc<SkyMap>>>,
    /// (index among the rendered images, item index, image) whose map is still needed.
    todo: Vec<(usize, usize, Arc<Img>)>,
}

impl SkyJob {
    /// Find the missing maps (sent back to be kept with the images) and finish.
    fn finish(mut self, tx: &Sender<Msg>) -> Arc<SkyAlign> {
        let found: Vec<(usize, usize, Arc<SkyMap>)> = self.todo.par_iter().map(|(k, i, img)| (*k, *i, Arc::new(stars::sky_map(img)))).collect();
        if !found.is_empty() {
            let _ = tx.send(Msg::SkyMaps(found.iter().map(|(_, i, m)| (*i, m.clone())).collect()));
        }
        for (k, _, m) in found {
            self.maps[k] = Some(m);
        }
        Arc::new(SkyAlign { g: self.g, maps: self.maps })
    }
}

struct Preview {
    img: Img,
    tex: Option<egui::TextureHandle>,
    tex_key: (u64, u64, u64, u64),
    box_: (f64, f64, f64, f64),
    scale: f64,
    outlines: Vec<Vec<Pt>>,
    covered: Option<Bits>,
    /// The view it was rendered with: projection, focal length (px), yaw / pitch / roll (°).
    view: (Projection, f64, [f64; 3]),
}

/// The horizon found or marked, as directions in the reference camera's frame
/// (so it follows the view when pitch and roll change): the curve and its points.
struct HorizonLine {
    curve: Vec<nalgebra::Vector3<f64>>,
    points: Vec<nalgebra::Vector3<f64>>,
    ref_: usize,
}

#[derive(Clone, Debug)]
pub struct Params {
    pub det_px: usize,
    pub nkp: usize,
    pub thr: f64,
    pub denoise: f64,
    pub layout: Layout,
    pub cols: usize,
    pub snake: bool,
    pub ratio: f64,
    pub maxd: f64,
    pub tol: f64,
    pub mininl: usize,
    pub model: Model,
    pub refine: bool,
    /// What of the lens distortion the refinement fits.
    pub lens_fit: LensFit,
    /// Each image gets its own lens distortion in the refinement.
    pub lens_own: bool,
    pub proj: Projection,
    pub blend: Blend,
    pub focal: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    pub prev_px: usize,
    pub ev: f64,
    pub black: f64,
    pub mid: f64,
    pub white: f64,
    pub decoder: RawDecoder,
    pub half: bool,
    pub unlinked: bool,
    /// Find the photos of the sky on the star catalogue (plate solving) and link them by their stars.
    pub solve: bool,
    /// Fill the empty areas of the panorama with made-up content (auto-complete).
    pub autofill: bool,
    /// Size of the saved panorama in % of the photos' full (camera) resolution.
    pub out_pct: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            det_px: 2000, nkp: 5000, thr: 0.001, denoise: 1.5, layout: Layout::All, cols: 5, snake: false, ratio: 0.8, maxd: 0.35,
            tol: 5.0, mininl: 20, model: Model::Homography, refine: true, lens_fit: LensFit::Radial, lens_own: false, proj: Projection::Cylindrical, blend: Blend::Smart, focal: 1.0,
            yaw: 0.0, pitch: 0.0, roll: 0.0, prev_px: 1800, ev: 0.0, black: 0.0, mid: 0.5, white: 1.0, decoder: RawDecoder::Auto, half: true,
            unlinked: false, solve: true, autofill: false, out_pct: 100.0,
        }
    }
}

impl Params {
    fn preset(&mut self, name: &str) -> &'static str {
        let (det, nkp, thr, den, ratio, maxd, tol, mininl, model, refine, proj, note) = match name {
            "day landscape" => (2000, 5000, 0.001, 0.5, 0.8, 0.35, 4.0, 20, Model::Homography, true, Projection::Cylindrical, tr("Daylight panorama, camera rotated in place.")),
            "astro / night landscape" => (2000, 6000, 0.0035, 2.0, 0.85, 0.38, 5.0, 16, Model::Homography, true, Projection::Spherical, tr("Night or Milky Way panorama: more denoising, looser matching, spherical projection.")),
            "deep-sky mosaic" => (3000, 4000, 0.002, 0.75, 0.75, 0.3, 3.0, 12, Model::Similarity, false, Projection::Rectilinear, tr("Telescope mosaic: frames only shift and rotate. Use Auto stretch to see faint detail.")),
            "macro / microscope" => (2000, 5000, 0.001, 1.0, 0.8, 0.35, 3.0, 15, Model::Translation, false, Projection::Rectilinear, tr("Stage moved under a fixed camera: frames only shift.")),
            _ => (2500, 5000, 0.001, 0.5, 0.8, 0.35, 3.0, 20, Model::Affine, false, Projection::Rectilinear, tr("Flat object scanned or photographed in parts.")),
        };
        (self.det_px, self.nkp, self.thr, self.denoise, self.ratio, self.maxd, self.tol, self.mininl) = (det, nkp, thr, den, ratio, maxd, tol, mininl);
        // The exposure is left alone: a preset sets how to match and project, not
        // how bright the panorama is (that is the Exposure slider's, or Auto exposure's).
        (self.model, self.refine, self.proj, self.layout, self.blend) = (model, refine, proj, Layout::All, Blend::Smart);
        note
    }
    fn stretched(&self) -> bool {
        !(self.black == 0.0 && self.mid == 0.5 && self.white == 1.0)
    }
    /// Feature settings of an image detected at `scale`.
    fn featkey_at(&self, scale: f64) -> FeatKey {
        ((scale * 1e6).round() as usize, self.nkp, self.thr.to_bits(), self.denoise.to_bits())
    }
    /// Detection scale of each image: the detection size, and frames shot at
    /// another zoom resized so that every frame's pixels cover the same angle
    /// (otherwise their features do not compare).
    fn det_scales(&self, items: &[Item]) -> Vec<f64> {
        let rho = zoom(&items.iter().map(|it| it.shot_info()).collect::<Vec<_>>()).1;
        items.iter().zip(rho).map(|(it, r)| ((self.det_px as f64 / it.img.w.max(it.img.h) as f64).min(1.0) / r).min(2.0)).collect()
    }
}

const PRESETS: [&str; 5] = ["day landscape", "astro / night landscape", "deep-sky mosaic", "macro / microscope", "scan / flat document"]; // tr

/// How a linear image is shown (and saved, except as FITS / XISF): exposure gain, then the stretch.
pub fn look(a: &Img, gain: f64, p: &Params) -> Img {
    let g = gain as f32;
    let b = if g == 1.0 { a.clone() } else { a.map(move |c| [c[0] * g, c[1] * g, c[2] * g]) };
    if p.stretched() { hist_stretch(&b, p.black, p.mid, p.white) } else { b }
}

enum Msg {
    Status(String),
    Loaded(Vec<Option<Item>>, Vec<String>),
    Detail(u64, Img, (f64, f64, f64, f64), f64),
    DetailFailed,
    Detected(Vec<(usize, Arc<Features>, FeatKey)>),
    Matched { feats: Vec<(usize, Arc<Features>, FeatKey)>, stars: Vec<(usize, Arc<Vec<Pt>>)>, solved: Vec<(usize, Solved)>, pairs: Vec<PairRes>, layout: Option<Placement>, msg: String },
    Preview(Img, (f64, f64, f64, f64), f64, Vec<Vec<Pt>>, Option<Bits>),
    SkyMaps(Vec<(usize, Arc<SkyMap>)>),
    Done(String),
    Error(String),
}

#[derive(PartialEq, Clone, Copy)]
enum ViewKind {
    Image,
    Pair,
    Pano,
}

/// Fewest rows of a band when the panorama is rendered in bands to save memory.
const BAND_MIN: f64 = 1024.0;

/// Memory per pixel of the finished panorama kept for saving: 8-bit for JPEG,
/// 16-bit for TIFF / PNG, 32-bit float for FITS / XISF.
fn out_bytes_per_px(ext: &str) -> f64 {
    match ext {
        "jpg" | "jpeg" | "bmp" => 3.0,
        "fits" | "fit" | "fts" | "xisf" => 12.0,
        _ => 6.0,
    }
}

/// The finished panorama as it is saved.
enum OutBuf {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Img),
}

/// Working size (longest side) of filling the empty areas: when saving, in the preview.
const FILL_WORK: usize = 1600;
const FILL_WORK_PREVIEW: usize = 800;

/// The panorama being saved, as the fill of its empty areas sees it.
struct BufCanvas<'a> {
    buf: &'a mut OutBuf,
    w: usize,
    h: usize,
}

impl crate::fill::Canvas for BufCanvas<'_> {
    fn size(&self) -> (usize, usize) {
        (self.h, self.w)
    }
    fn get(&self, y: usize, x: usize) -> [f32; 3] {
        let k = (y * self.w + x) * 3;
        match &*self.buf {
            OutBuf::U8(v) => [v[k] as f32 / 255.0, v[k + 1] as f32 / 255.0, v[k + 2] as f32 / 255.0],
            OutBuf::U16(v) => [v[k] as f32 / 65535.0, v[k + 1] as f32 / 65535.0, v[k + 2] as f32 / 65535.0],
            OutBuf::F32(img) => img.px[y * self.w + x],
        }
    }
    fn set(&mut self, y: usize, x: usize, c: [f32; 3]) {
        let k = (y * self.w + x) * 3;
        match self.buf {
            OutBuf::U8(v) => {
                for i in 0..3 {
                    v[k + i] = (c[i].clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
            OutBuf::U16(v) => {
                for i in 0..3 {
                    v[k + i] = (c[i].clamp(0.0, 1.0) * 65535.0).round() as u16;
                }
            }
            OutBuf::F32(img) => img.px[y * self.w + x] = c,
        }
    }
}

/// What saving the panorama needs (bytes), for the estimate and the check before saving.
struct SaveNeeds {
    w: f64,
    h: f64,
    w0: f64,
    h0: f64,
    nat: f64,
    reread: f64,
    per_px: f64,
    mem: f64,
    file: f64,
}

impl SaveNeeds {
    /// Whether it fits in the free memory and the disk holding `dir`; else what to do instead.
    /// `fresh`: query the system now (otherwise a value at most a few seconds old).
    fn check(&self, dir: &std::path::Path, fresh: bool) -> Result<(), String> {
        let (mem, disk) = if fresh { (available_memory(), available_disk(dir)) } else { crate::sys::free_cached(dir) };
        let free = mem - memory_reserve();
        if self.mem > free {
            // The largest output size that fits: without reading the RAWs again (up to their loaded size), or with.
            let area = self.w0 * self.h0 * self.per_px;
            let fit_loaded = (free.max(0.0) / area).sqrt().min(1.0) / self.nat * 100.0;
            let fit_full = ((free - self.reread).max(0.0) / area).sqrt() / self.nat * 100.0;
            let best = if fit_full > 100.0 / self.nat { fit_full } else { fit_loaded };
            let pct = (best / 5.0).floor() * 5.0;
            return Err(crate::trf!("Not enough memory: saving {}×{} px needs about {}, but only {} is free (keeping {} for the system). {}", self.w, self.h, gb(self.mem), gb(free.max(0.0)), gb(memory_reserve()), if pct >= 5.0 { format!("Lower Output size to {pct:.0} % or less, or close other programs.") } else { "Close other programs, or stitch fewer images.".to_string() }));
        }
        if let Some(d) = disk {
            if self.file > d as f64 {
                return Err(crate::trf!("Not enough disk space: the file needs up to {}, but only {} is free in {}. Choose another folder, a smaller Output size or JPEG.", gb(self.file), gb(d as f64), dir.display()));
            }
        }
        Ok(())
    }
}

/// The crop frame being drawn on the panorama view, in output coordinates
/// (u0, u1, v0, v1), with the crop before and the drag going on (grip, frame at
/// its start, start point).
#[derive(Clone)]
struct CropEdit {
    frame: (f64, f64, f64, f64),
    before: Option<(f64, f64, f64, f64)>,
    drag: Option<(super::finish::CropGrip, (f64, f64, f64, f64), (f64, f64))>,
}

/// Control points placed by hand on two images (left `i`, right `j`), in each
/// image's pixels, as in PTGui: for pairs the matching missed or got wrong.
struct CpEdit {
    i: usize,
    j: usize,
    pts: Vec<(Pt, Pt)>,
    /// A point on one side waiting for its partner: (on the right, point).
    pending: Option<(bool, Pt)>,
    /// The points are stars (the sky may have turned between the shots).
    stars: bool,
    /// Clicks snap to the nearest star.
    snap: bool,
    va: View,
    vb: View,
}

/// Painting source masks: the image to use, the brush (screen px), and the
/// stroke being drawn (output coordinates; right button: erasing).
struct MaskEdit {
    src: usize,
    brush: f32,
    stroke: Vec<(f64, f64)>,
    erase: bool,
}

pub struct StitchTab {
    /// Saved panoramas handed to the Post-processing tab.
    pub to_finish: Vec<String>,
    /// The display mode of the views (Siril-like), or as processed (the stretch panel).
    disp_view: crate::finish::ViewStretch,
    disp_processed: bool,
    /// What the saved panorama holds: linear data or the stretched image (None: by the format).
    save_linear: Option<bool>,
    /// Hand the panorama being saved to the Post-processing tab when it is done.
    post_after: bool,
    /// The crop frame being drawn on the panorama view.
    crop_edit: Option<CropEdit>,
    /// Control points being placed by hand on two images.
    cp: Option<CpEdit>,
    /// Source masks being painted, and their tinted view (key, texture, numbers at their centres).
    mask_edit: Option<MaskEdit>,
    mask_tex: Option<(u64, Option<egui::TextureHandle>, Vec<(usize, f64, f64)>)>,
    /// The panorama being saved, and the last one saved.
    saving_to: Option<String>,
    last_pano: Option<String>,
    /// The running job: when it started and whether it is saving the panorama.
    job_t0: Option<std::time::Instant>,
    job_is_save: bool,
    /// Time spent loading, matching and previewing since the first image was added.
    work_secs: f64,
    /// What the panorama being saved is made of: images used, images loaded, cropped.
    save_meta: (usize, usize, bool),
    /// Summary of the last save.
    report: Option<super::JobReport>,
    items: Vec<Item>,
    pairs: Vec<PairRes>,
    layout: Option<Placement>,
    p: Params,
    sel: usize,
    ref_choice: Option<usize>,
    pairsel: usize,
    show_outliers: bool,
    overlays: bool,
    view: ViewKind,
    vstate: View,
    pano: Option<Preview>,
    crop: Option<(f64, f64, f64, f64)>,
    status: String,
    pathbox: String,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    busy: bool,
    preview_pending: bool,
    dialog: Option<(Receiver<Picked>, &'static str)>,
    tex_cache: HashMap<(String, u64), egui::TextureHandle>,
    drag: Option<(usize, (f64, f64), [f64; 2])>,
    pub simple: bool,
    /// File type of “Save full resolution…” (None = automatic).
    save_format: Option<&'static str>,
    /// Multi-selection in the image list (always contains `sel` when there are images).
    selected: std::collections::BTreeSet<usize>,
    /// The message to show when the preview being rendered arrives (instead of “Preview ready”).
    note_after: Option<String>,
    undo: Vec<(Snap, String)>,
    redo: Vec<(Snap, String)>,
    committed: Option<(u64, Snap)>,
    pending: Option<(u64, std::time::Instant)>,
    project: Option<String>,
    /// Fingerprint of the state last saved to (or opened from) a project file.
    saved_fp: Option<u64>,
    pending_project: Option<ProjData>,
    pano_version: u64,
    detail: Option<Detail>,
    detail_req: Option<(u64, std::time::Instant)>,
    detail_busy: bool,
    /// The view of the preview being rendered (see `Preview::view`).
    rendering_view: (Projection, f64, [f64; 3]),
    /// The horizon found by Auto horizon or marked by hand, drawn on the panorama.
    horizon: Option<HorizonLine>,
    /// Points of the horizon being marked by hand (output coordinates).
    horizon_edit: Option<Vec<Pt>>,
    /// Show the sky's coordinates (RA / Dec grid and names) on the panorama.
    sky_grid: bool,
}

struct Detail {
    key: u64,
    tex: egui::TextureHandle,
    tex_key: (u64, u64, u64, u64),
    img: Img,
    box_: (f64, f64, f64, f64),
    scale: f64,
}

/// The state that undo / redo restores (images are never modified, so it is small).
#[derive(Clone)]
struct Snap {
    items: Vec<Item>,
    pairs: Vec<PairRes>,
    ref_choice: Option<usize>,
    layout: Option<Placement>,
    crop: Option<(f64, f64, f64, f64)>,
    p: Params,
}

/// A project file being opened: applied once its images are loaded.
struct ProjData {
    path: String,
    offsets: Vec<([f64; 2], f64)>,
    corrs: Vec<Correction>,
    masks: Vec<Vec<MaskDab>>,
    missing: Vec<String>,
    pairs: Vec<PairRes>,
    reference: Option<usize>,
    crop: Option<(f64, f64, f64, f64)>,
}

pub const PROJECT_EXT: &str = ".panoproj";

/// The numeric settings by the names used in project files and undo labels.
fn sliders(p: &mut Params) -> Vec<(&'static str, f64, Box<dyn FnMut(&mut Params, f64)>)> {
    macro_rules! s {
        ($n:expr, $f:ident, $t:ty) => {
            ($n, p.$f as f64, Box::new(|q: &mut Params, v: f64| q.$f = v as $t) as Box<dyn FnMut(&mut Params, f64)>)
        };
    }
    vec![
        s!("Detection size (px)", det_px, usize), s!("Max keypoints", nkp, usize), s!("Corner threshold", thr, f64), // tr
        s!("Denoise (σ px)", denoise, f64), s!("Grid columns", cols, usize), s!("Ratio test", ratio, f64), // tr
        s!("Max distance", maxd, f64), s!("RANSAC tol. (px)", tol, f64), s!("Min. inliers", mininl, usize), // tr
        s!("Focal (× auto)", focal, f64), s!("Yaw (°)", yaw, f64), s!("Pitch (°)", pitch, f64), s!("Roll (°)", roll, f64), // tr
        s!("Preview size", prev_px, usize), s!("Exposure", ev, f64), s!("Black point", black, f64), s!("Midtones", mid, f64), // tr
        s!("White point", white, f64), s!("Output size (%)", out_pct, f64), // tr
    ]
}

fn layout_name(l: Layout) -> &'static str {
    match l {
        Layout::All => "all pairs",
        Layout::Sequential => "sequential",
        Layout::Grid => "grid",
    }
}

fn fmt_num(v: f64) -> String {
    let r = (v * 10000.0).round() / 10000.0;
    if r == r.round() { format!("{}", r as i64) } else { format!("{r}") }
}

fn hash_of(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// What changed from state a to state b (for the undo labels).
fn describe(a: &Snap, b: &Snap) -> String {
    let pa: Vec<&str> = a.items.iter().map(|i| i.path.as_str()).collect();
    let pb: Vec<&str> = b.items.iter().map(|i| i.path.as_str()).collect();
    if pa != pb {
        return if pb.len() > pa.len() { tr("add images") } else if pb.len() < pa.len() { tr("remove images") } else { tr("reorder images") }.into();
    }
    if a.items.iter().zip(&b.items).any(|(x, y)| x.offset != y.offset) {
        return tr("move images").into();
    }
    if a.items.iter().zip(&b.items).any(|(x, y)| x.gain != y.gain) {
        return tr("brightness").into();
    }
    if a.items.iter().zip(&b.items).any(|(x, y)| x.corr.roll != y.corr.roll) {
        return tr("turn an image").into();
    }
    if a.items.iter().zip(&b.items).any(|(x, y)| x.corr != y.corr) {
        return tr("an image's lens distortion").into();
    }
    if a.items.iter().zip(&b.items).any(|(x, y)| x.mask != y.mask) {
        return tr("source mask").into();
    }
    let (mut qa, mut qb) = (a.p.clone(), b.p.clone());
    for ((n, x, _), (_, y, _)) in sliders(&mut qa).into_iter().zip(sliders(&mut qb)) {
        if x != y {
            return format!("{} {} → {}", tr(n), fmt_num(x), fmt_num(y));
        }
    }
    let menus = [
        (tr("Pairs"), layout_name(a.p.layout), layout_name(b.p.layout)),
        (tr("Model"), a.p.model.name(), b.p.model.name()),
        (tr("Lens distortion"), a.p.lens_fit.name(), b.p.lens_fit.name()),
        (tr("Projection"), a.p.proj.name(), b.p.proj.name()),
        (tr("Blend"), a.p.blend.name(), b.p.blend.name()),
    ];
    for (n, x, y) in menus {
        if x != y {
            return format!("{n} → {}", tr(y));
        }
    }
    for (n, x, y) in [(tr("snake"), a.p.snake, b.p.snake), (tr("refine"), a.p.refine, b.p.refine), (tr("distortion per image"), a.p.lens_own, b.p.lens_own), (tr("plate solving"), a.p.solve, b.p.solve), (tr("fill empty areas"), a.p.autofill, b.p.autofill), (tr("show unlinked images"), a.p.unlinked, b.p.unlinked)] {
        if x != y {
            return format!("{n} {}", if y { tr("on") } else { tr("off") });
        }
    }
    if a.crop != b.crop {
        return if b.crop.is_none() { tr("uncrop") } else { tr("crop") }.into();
    }
    if a.ref_choice != b.ref_choice {
        return tr("reference image").into();
    }
    tr("matching / pairs").into()
}

impl Default for StitchTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            items: vec![], pairs: vec![], layout: None, p: Params::default(), sel: 0, ref_choice: None, pairsel: 0, show_outliers: true,
            overlays: true, view: ViewKind::Image, vstate: View::default(), pano: None, crop: None,
            status: tr("Add images to begin (any order).").into(), pathbox: String::new(), tx, rx, busy: false, preview_pending: false,
            dialog: None, tex_cache: HashMap::new(), drag: None, simple: false, save_format: None, selected: Default::default(), note_after: None, undo: vec![], redo: vec![], committed: None, pending: None, saved_fp: None, to_finish: vec![], disp_view: Default::default(), disp_processed: true, save_linear: None, post_after: false, crop_edit: None, cp: None, mask_edit: None, mask_tex: None, saving_to: None, last_pano: None, job_t0: None, job_is_save: false, work_secs: 0.0, save_meta: (0, 0, false), report: None,
            project: None, pending_project: None, pano_version: 0, detail: None, detail_req: None, detail_busy: false,
            rendering_view: (Projection::Cylindrical, 1.0, [0.0; 3]), horizon: None, horizon_edit: None, sky_grid: false,
        }
    }
}

/// Zoom groups (images shot at the same focal length; the most common one is
/// group 0) and each image's focal length ÷ that of group 0. Images without a
/// recorded focal length count as group 0.
pub fn zoom(shots: &[ShotInfo]) -> (Vec<usize>, Vec<f64>) {
    let key = |s: &ShotInfo| s.focal_mm.map(|f| (f * 10.0).round() as i64);
    let mut count: HashMap<i64, usize> = HashMap::new();
    for k in shots.iter().filter_map(key) {
        *count.entry(k).or_insert(0) += 1;
    }
    let Some(mode) = count.iter().max_by_key(|(k, c)| (**c, -**k)).map(|(k, _)| *k) else {
        let (mut grp, mut rho) = (vec![0; shots.len()], vec![1.0; shots.len()]);
        solved_focal(shots, &mut grp, &mut rho);
        return (grp, rho);
    };
    let mut keys: Vec<i64> = count.keys().copied().filter(|&k| k != mode).collect();
    keys.sort_unstable();
    let grp = shots.iter().map(|s| key(s).filter(|&k| k != mode).map(|k| 1 + keys.iter().position(|&x| x == k).unwrap()).unwrap_or(0)).collect();
    let mut rho: Vec<f64> = shots.iter().map(|s| key(s).map(|k| k as f64 / mode as f64).unwrap_or(1.0)).collect();
    let mut grp: Vec<usize> = grp;
    solved_focal(shots, &mut grp, &mut rho);
    (grp, rho)
}

/// Photos found on the star catalogue: their focal lengths are measured (a
/// zoom lens that crept between them, all tagged "16 mm"), so they form one
/// group of their own with those exact ratios.
fn solved_focal(shots: &[ShotInfo], grp: &mut [usize], rho: &mut [f64]) {
    let solved: Vec<usize> = (0..shots.len()).filter(|&i| shots[i].focal_px.is_some()).collect();
    if solved.is_empty() {
        return;
    }
    // Their scale in units of group 0's focal length (as the EXIF ratios have it).
    let base = |set: Vec<usize>| crate::image::median_f64(set.iter().map(|&i| shots[i].focal_px.unwrap() / rho[i]));
    let in0: Vec<usize> = solved.iter().copied().filter(|&i| grp[i] == 0).collect();
    let f0 = if in0.is_empty() { base(solved.clone()) } else { base(in0) };
    let sg = grp.iter().copied().max().unwrap_or(0) + 1;
    for &i in &solved {
        rho[i] = shots[i].focal_px.unwrap() / f0;
        grp[i] = sg;
    }
}

/// Focal length (px) of zoom group 0, from accepted pairs within one zoom group.
/// Pairs of photos of the sky from their plate solutions: every two solved
/// photos that share enough identified stars, linked by them.
fn catalogue_pairs(sols: &[Option<Arc<solve::Solution>>], sizes: &[(usize, usize)], f0: f64, rho: &[f64]) -> Vec<PairRes> {
    let n = sols.len();
    let mut out = Vec::new();
    for i in 0..n {
        let Some(a) = &sols[i] else { continue };
        for j in i + 1..n {
            let Some(b) = &sols[j] else { continue };
            let (p1, p2) = solve::common_stars(a, b);
            if p1.len() < stars::MIN_STARS {
                continue;
            }
            let Some(h) = stars::rotation_from_points(&p1, &p2, sizes[i], sizes[j], f0 * rho[i], f0 * rho[j]) else { continue };
            let nm = p1.len();
            let err = p1.iter().zip(&p2).map(|(x, y)| reproj_err(&h, *y, *x)).sum::<f64>() / nm as f64;
            let msg = crate::trf!("Pair {}–{} (star catalogue): {} stars identified in both, mean error {} px", i + 1, j + 1, nm, format!("{err:.2}"));
            out.push(PairRes { i, j, p1: Arc::new(p1), p2: Arc::new(p2), h, inl: Arc::new((0..nm).collect()), ok: true, listed: true, msg, stars: true, manual: false });
        }
    }
    out
}

fn group_focal(pairs: &[PairRes], sizes: &[(usize, usize)], grp: &[usize], rho: &[f64]) -> Option<f64> {
    let cs = |i: usize| [(sizes[i].1 as f64 + 1.0) / 2.0, (sizes[i].0 as f64 + 1.0) / 2.0];
    let est: Vec<f64> = pairs
        .iter()
        .filter(|q| q.ok && grp[q.i] == grp[q.j])
        .filter_map(|q| estimate_focal(&[(q.h, cs(q.i), cs(q.j))]).map(|f| f / rho[q.i]))
        .collect();
    (!est.is_empty()).then(|| crate::image::median_f64(est))
}

fn center(img: &Img) -> Pt {
    [(img.w as f64 + 1.0) / 2.0, (img.h as f64 + 1.0) / 2.0]
}

/// Placement from the accepted pairs: spanning tree, reference image, transforms
/// and (optionally) the global refinement. Pairs that do not fit the refined
/// solution are rejected and the placement is redone (up to 3 times).
pub fn relayout(sizes: &[(usize, usize)], shots: &[ShotInfo], pairs: &mut [PairRes], ref_choice: Option<usize>, p: &Params) -> (Option<Placement>, String) {
    let n = sizes.len();
    if n == 0 {
        return (None, String::new());
    }
    let shots: Vec<ShotInfo> = (0..n).map(|i| shots.get(i).copied().unwrap_or_default()).collect();
    let (grp, rho) = zoom(&shots);
    let cs: Vec<Pt> = sizes.iter().map(|&(h, w)| [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0]).collect();
    let mut nrej = 0;
    for pass in 0..4 {
        let e: Vec<usize> = (0..pairs.len()).filter(|&k| pairs[k].ok).collect();
        let adj = spanning_tree(n, &e.iter().map(|&k| (pairs[k].i, pairs[k].j, pairs[k].inl.len() as f64)).collect::<Vec<_>>());
        let ref_ = ref_choice.filter(|&r| r < n).unwrap_or(if e.is_empty() { n.div_ceil(2) - 1 } else { tree_center(&adj) });
        let roots: Vec<usize> = std::iter::once(ref_).chain(0..n).collect();
        let (parent, order, comp) = walk_tree(&adj, &roots);
        let members: Vec<usize> = (0..n).filter(|&i| comp[i] == comp[ref_]).collect();
        let hd: HashMap<(usize, usize), M3> = e.iter().map(|&k| ((pairs[k].i, pairs[k].j), pairs[k].h)).collect();
        // Focal length of the reference image (its zoom group's, from pairs within one group).
        let f = group_focal(pairs, sizes, &grp, &rho).map(|f| f * rho[ref_])
            .or_else(|| estimate_focal(&e.iter().map(|&k| (pairs[k].h, cs[pairs[k].i], cs[pairs[k].j])).collect::<Vec<_>>()))
            .unwrap_or(sizes[ref_].0.max(sizes[ref_].1) as f64);
        let mut g = place(&hd, &parent, &order, ref_, &sizes.iter().map(|s| s.1).collect::<Vec<_>>());
        let (mut f, mut typical, mut sky_rate, mut sky) = (f, None, None, None);
        let mut lenses: Vec<Lens> = (0..n).map(|i| Lens::new(0.0, 0.0, f * rho[i] / rho[ref_])).collect();
        if p.refine && p.model == Model::Homography && members.len() > 1 {
            let m: Vec<MatchSet> = e.iter().map(|&k| { let pr = &pairs[k]; (pr.i, pr.j, pr.inl.iter().map(|&q| pr.p1[q]).collect(), pr.inl.iter().map(|&q| pr.p2[q]).collect()) }).collect();
            let opts = BundleOpts {
                group: grp.clone(),
                focal: rho.iter().map(|r| r / rho[ref_]).collect(),
                time: shots.iter().map(|s| s.time.unwrap_or(f64::NAN)).collect(),
                sky: e.iter().map(|&k| pairs[k].stars).collect(),
                poly: p.lens_fit == LensFit::Poly,
                per_image: p.lens_own,
            };
            let b = bundle_adjust_with(&g, &cs, ref_, f, &members, &m, 40, p.tol, 60, p.lens_fit != LensFit::Off, &opts);
            let fin: Vec<f64> = b.med.iter().copied().filter(|v| v.is_finite()).collect();
            let typ = if fin.is_empty() { 0.0 } else { crate::image::median_f64(fin) };
            typical = Some(typ);
            // Reject pairs far off the common solution, or clearly worse than typical while resting on few inliers.
            let weak = 2 * p.mininl;
            let bad: Vec<usize> = (0..e.len())
                .filter(|&q| !pairs[e[q]].manual && b.med[q].is_finite() && (b.med[q] > (4.0 * p.tol).max(8.0) || (b.med[q] > (3.0 * typ).max(2.0 * p.tol) && pairs[e[q]].inl.len() < weak)))
                .collect();
            if pass < 3 && !bad.is_empty() {
                for &q in &bad {
                    let pr = &mut pairs[e[q]];
                    pr.ok = false;
                    pr.msg = crate::trf!("Pair {}–{} REJECTED by the global refinement: it is off by {} px from all other pairs, so it is probably a false match (or something moved). Re-match it to try again.", pr.i + 1, pr.j + 1, format!("{:.1}", b.med[q]));
                }
                nrej += bad.len();
                continue;
            }
            g = b.g;
            f = b.f;
            lenses = b.lens;
            // The sky turns 15.04°/hour; far from that, the fit did not find it (too few loops of star and ground pairs).
            let rate = |w: &[f64; 3]| nalgebra::Vector3::new(w[0], w[1], w[2]).norm().to_degrees() * 3600.0;
            let found = b.sky.filter(|w| (10.0..20.0).contains(&rate(w)));
            sky_rate = found.as_ref().map(rate);
            if let Some(w) = found {
                // Turn the stars of every image with star pairs back to the middle moment of those images.
                let mut starry = vec![false; n];
                for &k in &e {
                    if pairs[k].stars {
                        starry[pairs[k].i] = true;
                        starry[pairs[k].j] = true;
                    }
                }
                let ts: Vec<f64> = (0..n).filter(|&i| starry[i]).filter_map(|i| shots[i].time).collect();
                if !ts.is_empty() {
                    let t0 = crate::image::median_f64(ts);
                    let dt = (0..n).map(|i| if starry[i] { shots[i].time.map(|t| t - t0).unwrap_or(f64::NAN) } else { f64::NAN }).collect();
                    sky = Some(SkyMotion { w, dt });
                }
            }
        }
        let g = park_unlinked(g, &parent, &order, &comp, ref_, &members, f, &cs, sizes[ref_]);
        let mut note = if nrej == 0 { String::new() } else { crate::trf!(" Rejected {} pair(s) that did not fit the others.", nrej) };
        let mm: Vec<f64> = members.iter().filter_map(|&i| shots[i].focal_mm).collect();
        let (lo, hi) = (mm.iter().copied().fold(f64::MAX, f64::min), mm.iter().copied().fold(f64::MIN, f64::max));
        if hi > lo {
            note += &crate::trf!(" The zoom changed between the shots ({}–{} mm): each focal length was fitted on its own.", format!("{lo}"), format!("{hi}"));
        }
        if let Some(r) = sky_rate {
            note += &crate::trf!(" Star pairs used; the sky turned {}°/hour between the shots.", format!("{r:.1}"));
        }
        return (Some(Placement { parent, order, comp, ref_, members, g, f, lenses, typical, sky_rate, sky }), note);
    }
    (None, String::new())
}

/// Move each group of images not linked to the reference into a row below the panorama.
#[allow(clippy::too_many_arguments)]
fn park_unlinked(mut g: Vec<M3>, parent: &[Option<usize>], order: &[usize], comp: &[usize], ref_: usize, members: &[usize], f: f64, cs: &[Pt], refsize: (usize, usize)) -> Vec<M3> {
    let roots: Vec<usize> = order.iter().copied().filter(|&v| parent[v].is_none() && comp[v] != comp[ref_]).collect();
    if roots.is_empty() {
        return g;
    }
    let kr = inv(&kmat(f, cs[ref_]));
    let pitch = |g: &[M3], i: usize| { let d = kr * g[i] * nalgebra::Vector3::new(cs[i][0], cs[i][1], 1.0); d[1].atan2(d[0].hypot(d[2])) };
    let (h, w) = (refsize.0 as f64, refsize.1 as f64);
    let (vfov, hfov) = (2.0 * (h / 2.0 / f).atan(), 2.0 * (w / 2.0 / f).atan());
    let p0 = (members.iter().map(|&i| pitch(&g, i)).fold(f64::MIN, f64::max) + 1.1 * vfov).min(80f64.to_radians());
    for (k, &r) in roots.iter().enumerate() {
        let yaw = (k as f64 - (roots.len() as f64 - 1.0) / 2.0) * 1.1 * hfov;
        let m = rotation_h(yaw, p0, f, cs[ref_], 0.0) * inv(&g[r]);
        for &v in order {
            if comp[v] == comp[r] {
                let gv = m * g[v];
                g[v] = gv / gv.determinant().cbrt();
            }
        }
    }
    g
}

impl StitchTab {
    /// Show a message in the status line.
    pub fn notice(&mut self, s: String) {
        self.status = s;
    }

    pub fn busy(&self) -> bool {
        self.busy || self.dialog.is_some()
    }

    /// Run `f` in the background; false if another step is still running.
    fn spawn(&mut self, msg: &str, f: impl FnOnce(&Sender<Msg>) -> Result<Msg, String> + Send + 'static) -> bool {
        if self.busy {
            self.status = tr("Busy – wait for the current step to finish.").into();
            return false;
        }
        if self.items.is_empty() {
            self.work_secs = 0.0; // a new set of images
        }
        self.job_t0 = Some(std::time::Instant::now());
        self.job_is_save = false;
        self.busy = true;
        self.status = msg.into();
        let tx = self.tx.clone();
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&tx))).unwrap_or_else(|_| Err(tr("internal error (panic)").into()));
            let _ = tx.send(r.unwrap_or_else(Msg::Error));
        });
        true
    }

    /// Files dropped on the window.
    pub fn add_dropped(&mut self, ps: Vec<String>) {
        if self.busy {
            self.status = tr("Busy – wait for the current step to finish, then drop the files again.").into();
        } else {
            self.add_paths(ps);
        }
    }

    /// What is remembered between sessions: how RAW files are read.
    pub fn remembered(&self) -> toml::Table {
        let mut t = toml::Table::new();
        t.insert("raw_decoder".into(), self.p.decoder.name().into());
        t.insert("half_size".into(), self.p.half.into());
        t
    }
    pub fn restore_remembered(&mut self, t: &toml::Table) {
        if let Some(dec) = t.get("raw_decoder").and_then(|v| v.as_str()).and_then(RawDecoder::from_name) {
            if dec == RawDecoder::Auto || io::available_raw_decoders().contains(&dec) {
                self.p.decoder = dec;
            }
        }
        if let Some(h) = t.get("half_size").and_then(|v| v.as_bool()) {
            self.p.half = h;
        }
    }

    pub fn add_paths(&mut self, ps: Vec<String>) {
        let ps: Vec<String> = ps.into_iter().filter(|p| !p.is_empty()).collect();
        if !ps.is_empty() {
            self.add_paths_keep(ps);
        }
    }

    /// Load images in the background; the result keeps one entry per path ("" = skip).
    fn add_paths_keep(&mut self, ps: Vec<String>) {
        let opt = LoadOptions { decoder: self.p.decoder, half: self.p.half, ..Default::default() };
        let n = ps.len();
        self.spawn(&crate::trf!("Loading {} image(s)…", n), move |tx| {
            let done = std::sync::atomic::AtomicUsize::new(0);
            // Stop before the memory runs out: past that point the system swaps
            // and everything (not just this program) stops responding.
            let reserve = memory_reserve();
            let full = std::sync::atomic::AtomicBool::new(false);
            let res: Vec<Result<Item, String>> = ps
                .par_iter()
                .map(|p| {
                    if p.is_empty() {
                        return Err(String::new());
                    }
                    if full.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err(format!("{LOWMEM}{}", basename(p)));
                    }
                    let native = if opt.half && io::is_raw(p) { 2.0 } else { 1.0 };
                    let r = io::load_any(p, &opt).map(|img| {
                        let f = (DISP_MAX as f64 / img.w.max(img.h) as f64).min(1.0);
                        let disp = if f < 1.0 { resize(&img, f) } else { img.clone() };
                        Item { path: p.clone(), img: Arc::new(img), disp: Arc::new(disp), f, feats: None, featkey: None, gain: 1.0, offset: [0.0; 2], native, shot: io::shot_info(p), stars: None, sky_map: None, mask: Arc::new(vec![]), solved: None, corr: Correction::default() }
                    });
                    if available_memory() < reserve {
                        full.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let _ = tx.send(Msg::Status(crate::trf!("Loading {}/{}: {}", k, n, basename(p))));
                    r
                })
                .collect();
            let mut items = Vec::new();
            let mut failed = Vec::new();
            for r in res {
                match r {
                    Ok(i) => items.push(Some(i)),
                    Err(e) => {
                        failed.push(e);
                        items.push(None);
                    }
                }
            }
            Ok(Msg::Loaded(items, failed))
        });
    }

    /// The image list changed: `old_of[k]` is the former index of image k (None = new).
    /// Matching results of the images that stay are kept; the placement is redone.
    fn list_changed_from(&mut self, old_of: &[Option<usize>]) {
        let mut new_of: HashMap<usize, usize> = HashMap::new();
        for (k, o) in old_of.iter().enumerate() {
            if let Some(o) = o {
                new_of.insert(*o, k);
            }
        }
        self.pairs = std::mem::take(&mut self.pairs)
            .into_iter()
            .filter_map(|mut q| {
                (q.i, q.j) = (*new_of.get(&q.i)?, *new_of.get(&q.j)?);
                Some(q)
            })
            .collect();
        self.ref_choice = self.ref_choice.and_then(|r| new_of.get(&r).copied());
        self.pano = None;
        self.crop = None;
        self.detail = None;
        self.sel = self.sel.min(self.items.len().saturating_sub(1));
        self.selected.retain(|&i| i < self.items.len());
        if self.items.is_empty() {
            self.selected.clear();
        } else {
            self.selected.insert(self.sel);
        }
        self.pairsel = 0;
        if self.items.is_empty() {
            self.layout = None;
        } else {
            let note = self.relayout_now();
            if !note.is_empty() {
                self.status += &note;
            }
        }
    }

    /// Images added at the end (or the list cleared / replaced).
    fn list_changed(&mut self) {
        let n0 = self.layout.as_ref().map(|l| l.comp.len()).unwrap_or(0).min(self.items.len());
        let old_of: Vec<Option<usize>> = (0..self.items.len()).map(|k| if k < n0 { Some(k) } else { None }).collect();
        self.list_changed_from(&old_of);
    }

    fn targets(&self) -> Vec<usize> {
        if self.selected.is_empty() { vec![self.sel] } else { self.selected.iter().copied().collect() }
    }

    fn remove_selected(&mut self) {
        if self.busy || self.items.is_empty() {
            return;
        }
        let gone = self.targets();
        let old_of: Vec<Option<usize>> = (0..self.items.len()).filter(|i| !gone.contains(i)).map(Some).collect();
        let n = gone.len();
        let mut k = 0;
        self.items.retain(|_| {
            k += 1;
            !gone.contains(&(k - 1))
        });
        self.sel = gone[0].min(self.items.len().saturating_sub(1));
        self.selected.clear();
        self.status = crate::trf!("Removed {} image(s); {} left.", n, self.items.len());
        self.list_changed_from(&old_of);
    }

    /// Move the selected images one place up (-1) or down (+1).
    fn move_selected(&mut self, d: i64) {
        if self.busy || self.items.is_empty() {
            return;
        }
        let sel = self.targets();
        let n = self.items.len();
        if (d < 0 && sel[0] == 0) || (d > 0 && *sel.last().unwrap() == n - 1) {
            return;
        }
        let mut order: Vec<usize> = (0..n).collect();
        let iter: Vec<usize> = if d < 0 { sel.clone() } else { sel.iter().rev().copied().collect() };
        for i in iter {
            let j = (i as i64 + d) as usize;
            order.swap(i, j);
        }
        let mut items: Vec<Option<Item>> = std::mem::take(&mut self.items).into_iter().map(Some).collect();
        self.items = order.iter().map(|&o| items[o].take().unwrap()).collect();
        self.selected = sel.iter().map(|&i| (i as i64 + d) as usize).collect();
        self.sel = (self.sel as i64 + d).clamp(0, n as i64 - 1) as usize;
        let old_of: Vec<Option<usize>> = order.into_iter().map(Some).collect();
        self.list_changed_from(&old_of);
    }

    /// Only detect features (where the settings changed), to inspect them before matching.
    fn detect_only(&mut self) {
        if self.items.is_empty() {
            self.status = tr("Add images first.").into();
            return;
        }
        let p = self.p.clone();
        let dets = p.det_scales(&self.items);
        let todo: Vec<(usize, Arc<Img>, f64)> =
            self.items.iter().enumerate().filter(|(i, it)| it.featkey != Some(p.featkey_at(dets[*i]))).map(|(i, it)| (i, it.img.clone(), dets[i])).collect();
        self.spawn(&crate::trf!("Detecting features in {} image(s)…", todo.len()), move |_| {
            let feats: Vec<(usize, Arc<Features>, FeatKey)> = todo
                .par_iter()
                .map(|(i, img, det)| {
                    let dp = DetectParams { scale: *det, nkeypoints: p.nkp, threshold: p.thr, denoise: p.denoise, oriented: false };
                    (*i, Arc::new(detect(img, &dp)), p.featkey_at(*det))
                })
                .collect();
            Ok(Msg::Detected(feats))
        });
    }

    /// Detect (where needed), match `ij` (all candidate pairs if None) and place, then optionally preview.
    fn match_pairs(&mut self, ij: Option<Vec<(usize, usize)>>, then_preview: bool) {
        if self.items.len() < 2 {
            self.status = tr("Add at least two images.").into();
            return;
        }
        let p = self.p.clone();
        let shots: Vec<ShotInfo> = self.items.iter().map(|it| it.shot_info()).collect();
        let dets = p.det_scales(&self.items);
        let imgs: Vec<(Arc<Img>, Option<Arc<Features>>)> =
            self.items.iter().enumerate().map(|(i, it)| (it.img.clone(), if it.featkey == Some(p.featkey_at(dets[i])) { it.feats.clone() } else { None })).collect();
        let known_stars: Vec<Option<Arc<Vec<Pt>>>> = self.items.iter().map(|it| it.stars.clone()).collect();
        let known_solved: Vec<Option<Solved>> = self.items.iter().map(|it| it.solved.clone()).collect();
        let n = imgs.len();
        let given = ij.is_some();
        let mut ij = ij.unwrap_or_else(|| candidate_pairs(n, p.layout, p.cols, p.snake));
        // A full run keeps the pairs placed by hand (Re-match pair replaces one).
        let manual: Vec<(usize, usize)> = self.pairs.iter().filter(|q| q.manual && q.ok).map(|q| (q.i, q.j)).collect();
        if !given {
            ij.retain(|ij| !manual.contains(ij));
        }
        let mut old: Vec<PairRes> = self.pairs.iter().filter(|q| !ij.contains(&(q.i, q.j))).cloned().collect();
        // Pairs whose images got new features are stale (not those placed by hand).
        let stale: Vec<usize> = (0..n).filter(|&i| imgs[i].1.is_none()).collect();
        old.retain(|q| q.manual || (!stale.contains(&q.i) && !stale.contains(&q.j)));
        let ref_choice = self.ref_choice;
        let sizes: Vec<(usize, usize)> = self.items.iter().map(|it| (it.img.h, it.img.w)).collect();
        self.preview_pending = then_preview;
        let listed = given || p.layout != Layout::All;
        self.spawn(tr("Detecting features…"), move |tx| {
            let mut shots = shots;
            let feats: Vec<Arc<Features>> = imgs
                .par_iter()
                .enumerate()
                .map(|(i, (img, f))| f.clone().unwrap_or_else(|| {
                    let dp = DetectParams { scale: dets[i], nkeypoints: p.nkp, threshold: p.thr, denoise: p.denoise, oriented: false };
                    Arc::new(detect(img, &dp))
                }))
                .collect();
            let _ = tx.send(Msg::Status(crate::trf!("Matching {} pair(s)…", ij.len())));
            let mut res: Vec<PairRes> = ij
                .par_iter()
                .map(|&(i, j)| {
                    let (p1, p2) = match_pair(&feats[i], &feats[j], p.ratio, p.maxd);
                    let name = crate::trf!("Pair {}–{} ({})", i + 1, j + 1, tr(p.model.name()));
                    match ransac(&p2, &p1, p.model, 2000, p.tol) {
                        Ok((h, inl)) => {
                            let (ok, why) = accept_pair(p1.len(), inl.len(), &h, sizes[j].0, sizes[j].1, p.mininl, &inl.iter().map(|&k| p2[k]).collect::<Vec<_>>());
                            let err = inl.iter().map(|&k| reproj_err(&h, p2[k], p1[k])).sum::<f64>() / inl.len().max(1) as f64;
                            let msg = crate::trf!("{}: {} matches, {} inliers ({}%), mean error {} px{}", name, p1.len(), inl.len(), 100 * inl.len() / p1.len().max(1), format!("{err:.2}"), if ok { String::new() } else { format!("\nREJECTED: {why}.") });
                            PairRes { i, j, p1: Arc::new(p1), p2: Arc::new(p2), h, inl: Arc::new(inl), ok, listed: listed || ok, msg, stars: false, manual: false }
                        }
                        Err(e) => PairRes { i, j, p1: Arc::new(p1), p2: Arc::new(p2), h: M3::zeros(), inl: Arc::new(vec![]), ok: false, listed, msg: format!("{name} FAILED: {e}"), stars: false, manual: false },
                    }
                })
                .collect();
            // Night sky: pairs the features could not match may still match by their stars.
            let (grp, rho) = zoom(&shots);
            let failed: Vec<usize> = (0..res.len()).filter(|&k| !res[k].ok).collect();
            let mut found_stars: Vec<(usize, Arc<Vec<Pt>>)> = Vec::new();
            if !failed.is_empty() {
                let _ = tx.send(Msg::Status(tr("Matching the sky by its stars…").into()));
                let mut all = res.clone();
                all.extend(old.iter().cloned());
                let f0 = group_focal(&all, &sizes, &grp, &rho).unwrap_or(sizes[0].0.max(sizes[0].1) as f64);
                let mut want = vec![false; n];
                for &k in &failed {
                    want[res[k].i] = true;
                    want[res[k].j] = true;
                }
                let stars: Vec<Option<Arc<Vec<Pt>>>> = (0..n)
                    .into_par_iter()
                    .map(|i| known_stars[i].clone().or_else(|| want[i].then(|| Arc::new(stars::sky_stars(&imgs[i].0)))))
                    .collect();
                found_stars = (0..n).filter(|&i| known_stars[i].is_none()).filter_map(|i| stars[i].clone().map(|s| (i, s))).collect();
                let index: Vec<Option<stars::SkyIndex>> = (0..n)
                    .into_par_iter()
                    .map(|i| stars[i].as_ref().filter(|s| s.len() >= stars::MIN_STARS).map(|s| stars::SkyIndex::new(s, sizes[i].0, sizes[i].1, f0 * rho[i])))
                    .collect();
                let found: Vec<(usize, PairRes)> = failed
                    .par_iter()
                    .filter_map(|&k| {
                        let (i, j) = (res[k].i, res[k].j);
                        let (a, b) = (index[i].as_ref()?, index[j].as_ref()?);
                        let m = stars::match_stars(a, b, p.tol).ok()?;
                        let nm = m.p1.len();
                        let msg = crate::trf!("Pair {}–{} (stars): {} of {} stars in the overlap match, mean error {} px", i + 1, j + 1, nm, m.expected, format!("{:.2}", m.err));
                        Some((k, PairRes { i, j, p1: Arc::new(m.p1), p2: Arc::new(m.p2), h: m.h, inl: Arc::new((0..nm).collect()), ok: true, listed: true, msg, stars: true, manual: false }))
                    })
                    .collect();
                for (k, pr) in found {
                    res[k] = pr;
                }
            }
            let nacc = res.iter().filter(|r| r.ok).count();
            let ntried = res.len();
            res.extend(old);
            res.sort_by_key(|r| (r.i, r.j));
            // Photos of the sky: found on the star catalogue, two that overlap
            // share identified stars, which link them even where the matching
            // failed or took a wrong match.
            let mut solved_now: Vec<(usize, Solved)> = Vec::new();
            let mut cat_note = String::new();
            if p.solve {
                let todo: Vec<usize> = (0..n).filter(|&i| known_solved[i].is_none()).collect();
                if !todo.is_empty() {
                    let _ = tx.send(Msg::Status(tr("Finding the photos of the sky on the star catalogue…").into()));
                }
                let f0 = group_focal(&res, &sizes, &grp, &rho).unwrap_or(sizes[0].0.max(sizes[0].1) as f64);
                solved_now = todo
                    .par_iter()
                    .map(|&i| {
                        let img = &imgs[i].0;
                        let sol = if solve::sky_share(img) < solve::SKY_SHARE {
                            Solved::NotSky
                        } else {
                            match solve::solve_photo(img, f0 * rho[i]) {
                                Ok(s) => Solved::Found(Arc::new(s)),
                                Err(e) => Solved::Failed(e),
                            }
                        };
                        (i, sol)
                    })
                    .collect();
                let mut state: Vec<Option<Solved>> = known_solved.clone();
                for (i, s) in &solved_now {
                    state[*i] = Some(s.clone());
                }
                let sols: Vec<Option<Arc<solve::Solution>>> = state.iter().map(|s| s.as_ref().and_then(|s| s.found().cloned())).collect();
                let nsky = state.iter().filter(|s| matches!(s, Some(Solved::Found(_) | Solved::Failed(_)))).count();
                let nsolved = sols.iter().filter(|s| s.is_some()).count();
                let cat_pairs = catalogue_pairs(&sols, &sizes, f0, &rho);
                let (mut linked, mut corrected, mut added) = (0usize, 0usize, 0usize);
                for pr in cat_pairs {
                    match res.iter().position(|q| (q.i, q.j) == (pr.i, pr.j)) {
                        None => {
                            res.push(pr);
                            added += 1;
                        }
                        Some(k) if res[k].manual => {}
                        Some(k) if !res[k].ok => {
                            res[k] = pr;
                            linked += 1;
                        }
                        Some(k) => {
                            // A match the catalogue's stars disagree with is wrong.
                            let q = &res[k];
                            let e = pr.p1.iter().zip(pr.p2.iter()).map(|(a, b)| reproj_err(&q.h, *b, *a)).sum::<f64>() / pr.p1.len() as f64;
                            if e > (4.0 * p.tol).max(15.0) {
                                res[k] = pr;
                                corrected += 1;
                            }
                        }
                    }
                }
                res.sort_by_key(|r| (r.i, r.j));
                for (i, s) in state.iter().enumerate() {
                    shots[i].focal_px = s.as_ref().and_then(|s| s.found()).map(|s| s.f);
                }
                if nsky > 0 {
                    cat_note = crate::trf!(" Star catalogue: {} of {} photos of the sky found, {} pairs linked, {} corrected, {} added.", nsolved, nsky, linked, corrected, added);
                }
            }
            let _ = tx.send(Msg::Status(crate::trf!("Placing images{}", if p.refine { tr(" and refining globally…") } else { "…" })));
            let nstars = res.iter().filter(|r| r.ok && r.stars).count();
            let (layout, note) = relayout(&sizes, &shots, &mut res, ref_choice, &p);
            let mut msg = crate::trf!("{} of {} pairs accepted.{}", nacc, ntried, note);
            if nstars > 0 {
                msg = crate::trf!("{} of {} pairs accepted ({} of them by their stars).{}", nacc, ntried, nstars, note);
            }
            msg += &cat_note;
            if let Some(l) = &layout {
                if let Some(t) = l.typical {
                    msg += &crate::trf!(" Typical pair error after refinement: {} px.", format!("{t:.1}"));
                }
                let unl: Vec<String> = (0..n).filter(|&i| l.comp[i] != l.comp[l.ref_]).map(|i| (i + 1).to_string()).collect();
                msg += &if unl.is_empty() { tr(" All images are linked.").to_string() } else { crate::trf!(" NOT linked: images {} – hidden; switch on “show unlinked images” to place them by hand, or try other matching settings.", unl.join(", ")) };
            }
            if let Some(l) = &layout {
                // Suggest the spherical projection for panoramas that reach far up or down.
                if p.proj != Projection::Spherical {
                    let cs: Vec<Pt> = sizes.iter().map(|&(h, w)| [(w as f64 + 1.0) / 2.0, (h as f64 + 1.0) / 2.0]).collect();
                    let kr = inv(&kmat(l.f, cs[l.ref_]));
                    let steep = l.members.iter().any(|&i| {
                        let d = kr * l.g[i] * nalgebra::Vector3::new(cs[i][0], cs[i][1], 1.0);
                        d[1].atan2(d[0].hypot(d[2])).abs() > 45f64.to_radians()
                    });
                    if steep {
                        msg += tr(" Tip: the panorama reaches far up/down; the spherical projection suits it better.");
                    }
                }
            }
            Ok(Msg::Matched { feats: feats.into_iter().enumerate().map(|(i, f)| (i, f, p.featkey_at(dets[i]))).collect(), stars: found_stars, solved: solved_now, pairs: res, layout, msg })
        });
    }

    fn relayout_now(&mut self) -> String {
        let sizes: Vec<(usize, usize)> = self.items.iter().map(|it| (it.img.h, it.img.w)).collect();
        let shots: Vec<ShotInfo> = self.items.iter().map(|it| it.shot_info()).collect();
        let (l, note) = relayout(&sizes, &shots, &mut self.pairs, self.ref_choice, &self.p);
        self.layout = l;
        note
    }

    fn visible(&self) -> Vec<usize> {
        let Some(l) = &self.layout else { return vec![] };
        (0..self.items.len()).filter(|&i| self.p.unlinked || l.comp[i] == l.comp[l.ref_]).collect()
    }

    /// Transform of every image into the (rotated) reference plane, including nudges.
    fn placements(&self, f: f64, c: Pt) -> Vec<M3> {
        let l = self.layout.as_ref().unwrap();
        let nud = |i: usize| rotation_h(self.items[i].offset[0] / f, self.items[i].offset[1] / f, f, c, 0.0);
        let mut nm = vec![M3::identity(); self.items.len()];
        for &v in &l.order {
            if v == l.ref_ {
                continue;
            }
            nm[v] = match l.parent[v] { None => nud(v), Some(p) => nm[p] * nud(v) };
        }
        let vrot = rotation_h(self.p.yaw.to_radians(), self.p.pitch.to_radians(), f, c, self.p.roll.to_radians());
        (0..self.items.len()).map(|i| vrot * nm[i] * l.g[i] * self.spin(i)).collect()
    }

    /// Image i turned about its own centre by its roll correction (in its own pixels).
    fn spin(&self, i: usize) -> M3 {
        let r = self.items[i].corr.roll.to_radians();
        if r == 0.0 {
            return M3::identity();
        }
        let c = center(&self.items[i].img);
        let (s, k) = r.sin_cos();
        translation(c[0], c[1]) * M3::new(k, -s, 0.0, s, k, 0.0, 0.0, 0.0, 1.0) * translation(-c[0], -c[1])
    }

    /// Lens distortion of every image: the refinement's, plus its own correction.
    fn lenses(&self) -> Vec<Lens> {
        let l = self.layout.as_ref();
        (0..self.items.len())
            .map(|i| {
                let base = l.and_then(|l| l.lenses.get(i).copied()).unwrap_or_else(|| Lens::new(0.0, 0.0, l.map(|l| l.f).unwrap_or(1.0)));
                let c = self.items[i].corr;
                Lens::new(base.k1 + c.k1, base.k2 + c.k2, base.fc)
            })
            .collect()
    }

    fn geometry(&self) -> Option<(f64, Pt, Vec<M3>, Vec<usize>, Vec<Lens>)> {
        let l = self.layout.as_ref()?;
        let f = l.f * self.p.focal;
        let c = center(&self.items[l.ref_].img);
        Some((f, c, self.placements(f, c), self.visible(), self.lenses()))
    }

    fn request_preview(&mut self) {
        if self.busy {
            self.preview_pending = true;
            return;
        }
        let Some((f, c, g, vis, lens)) = self.geometry() else {
            self.status = tr("Match the images first.").into();
            return;
        };
        self.rendering_view = (self.p.proj, f, [self.p.yaw, self.p.pitch, self.p.roll]);
        let p = self.p.clone();
        let outlines: Vec<Vec<Pt>> = (0..self.items.len()).map(|i| if vis.contains(&i) { outline(&g[i], self.items[i].img.h, self.items[i].img.w, p.proj, f, c, lens[i], 24) } else { vec![] }).collect();
        let lenses: Vec<Lens> = vis.iter().map(|&i| lens[i]).collect();
        let full = bounds(&vis.iter().map(|&i| outlines[i].clone()).collect::<Vec<_>>());
        // While the crop frame is drawn, the whole panorama is shown.
        let crop = if self.crop_edit.is_some() { None } else { self.crop };
        let box_ = crop.unwrap_or(full);
        let side = (box_.1 - box_.0).max(box_.3 - box_.2) + 1.0;
        let scale = (p.prev_px as f64 / side).min(1.0);
        let (imgs, scales, sizes, gains, gv) = self.compose_inputs(&vis, &g, scale);
        let sky = self.sky_job(&vis, &g);
        let force = self.force_job(&vis);
        let uncropped = crop.is_none();
        self.spawn(tr("Rendering preview…"), move |tx| {
            let sky = sky.map(|j| j.finish(tx));
            let refs: Vec<&Img> = imgs.iter().map(|a| a.as_ref()).collect();
            let cp = ComposeParams { scale, projection: p.proj, f, c, blend: p.blend, lenses, sharpness: 3.0, deghost: 0.12, levels: None, sky, force };
            let (mut img, cov) = compose(&refs, &gv, box_, &cp, &gains, &scales, &sizes, uncropped || p.autofill);
            if p.autofill {
                if let Some(c) = &cov {
                    let _ = tx.send(Msg::Status(tr("Filling the empty areas…").into()));
                    crate::fill::complete_img(&mut img, &crate::fill::Cover::from_bits(c), FILL_WORK_PREVIEW);
                }
            }
            Ok(Msg::Preview(img, box_, scale, outlines, if uncropped { cov } else { None }))
        });
    }

    /// Images (display copies if they are sharp enough for `scale`), their scales, sizes, gains and transforms.
    #[allow(clippy::type_complexity)]
    fn compose_inputs(&self, vis: &[usize], g: &[M3], scale: f64) -> (Vec<Arc<Img>>, Vec<f64>, Vec<(usize, usize)>, Vec<f64>, Vec<M3>) {
        let use_disp = vis.iter().all(|&i| scale <= self.items[i].f);
        let imgs = vis.iter().map(|&i| if use_disp { self.items[i].disp.clone() } else { self.items[i].img.clone() }).collect();
        let scales = vis.iter().map(|&i| if use_disp { self.items[i].f } else { 1.0 }).collect();
        let sizes = vis.iter().map(|&i| (self.items[i].img.h, self.items[i].img.w)).collect();
        let gains = vis.iter().map(|&i| self.items[i].gain * 2f64.powf(self.p.ev)).collect();
        let gv = vis.iter().map(|&i| g[i]).collect();
        (imgs, scales, sizes, gains, gv)
    }

    /// The sky alignment for rendering images `vis` placed by `g` (None without a turning sky).
    fn sky_job(&self, vis: &[usize], g: &[M3]) -> Option<SkyJob> {
        let l = self.layout.as_ref()?;
        let sm = l.sky.as_ref()?;
        let k = kmat(l.f, center(&self.items[l.ref_].img));
        let w = nalgebra::Vector3::new(sm.w[0], sm.w[1], sm.w[2]);
        let mut job = SkyJob { g: vec![None; vis.len()], maps: vec![None; vis.len()], todo: vec![] };
        for (kk, &i) in vis.iter().enumerate() {
            let Some(&dt) = sm.dt.get(i).filter(|d| d.is_finite()) else { continue };
            // Back in time by dt: the reference plane turned with the sky, then the image's own placement.
            let turn = k * rodrigues(&(w * -dt)) * inv(&k);
            let spin = self.spin(i);
            job.g[kk] = Some(g[i] * inv(&spin) * inv(&l.g[i]) * turn * l.g[i] * spin);
            match &self.items[i].sky_map {
                Some(m) => job.maps[kk] = Some(m.clone()),
                None => job.todo.push((kk, i, self.items[i].img.clone())),
            }
        }
        job.g.iter().any(|x| x.is_some()).then_some(job)
    }

    /// Re-render the visible part of the panorama at screen resolution (in the background).
    fn render_detail(&mut self, key: u64, box_: (f64, f64, f64, f64), scale: f64) {
        let Some((f, c, g, vis, lens)) = self.geometry() else { return };
        let (imgs, scales, sizes, gains, gv) = self.compose_inputs(&vis, &g, scale);
        let sky = self.sky_job(&vis, &g);
        let force = self.force_job(&vis);
        let lenses: Vec<Lens> = vis.iter().map(|&i| lens[i]).collect();
        let (proj, blend, tx) = (self.p.proj, self.p.blend, self.tx.clone());
        self.detail_busy = true;
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(|| {
                let sky = sky.map(|j| j.finish(&tx));
                let refs: Vec<&Img> = imgs.iter().map(|a| a.as_ref()).collect();
                let cp = ComposeParams { scale, projection: proj, f, c, blend, lenses, sharpness: 3.0, deghost: 0.12, levels: None, sky, force };
                compose(&refs, &gv, box_, &cp, &gains, &scales, &sizes, false).0
            });
            let _ = tx.send(match r {
                Ok(img) => Msg::Detail(key, img, box_, scale),
                Err(_) => Msg::DetailFailed,
            });
        });
    }

    fn snap(&self) -> Snap {
        Snap { items: self.items.clone(), pairs: self.pairs.clone(), ref_choice: self.ref_choice, layout: self.layout.clone(), crop: self.crop, p: self.p.clone() }
    }

    fn fingerprint(&self) -> u64 {
        let items: Vec<(&str, [f64; 2], f64, usize, f64, Correction)> = self.items.iter().map(|i| (i.path.as_str(), i.offset, i.gain, i.mask.len(), i.mask.iter().map(|d| d.x + d.y + d.r).sum::<f64>(), i.corr)).collect();
        let pairs: Vec<(usize, usize, bool, usize)> = self.pairs.iter().map(|q| (q.i, q.j, q.ok, q.inl.len())).collect();
        hash_of(&format!("{items:?}{pairs:?}{:?}{:?}{:?}{}", self.ref_choice, self.crop, self.p, self.layout.is_some()))
    }

    /// Record the state when it has changed and then stayed the same for a moment (a slider drag counts once).
    fn check_undo(&mut self) {
        if self.busy || self.pending_project.is_some() {
            return;
        }
        let fp = self.fingerprint();
        let Some((cfp, _)) = &self.committed else {
            self.committed = Some((fp, self.snap()));
            return;
        };
        if *cfp == fp {
            self.pending = None;
            return;
        }
        match self.pending {
            Some((pf, t)) if pf == fp => {
                if t.elapsed() > std::time::Duration::from_millis(400) {
                    self.commit(fp);
                }
            }
            _ => self.pending = Some((fp, std::time::Instant::now())),
        }
    }

    fn commit(&mut self, fp: u64) {
        let new = self.snap();
        if let Some((_, old)) = self.committed.take() {
            let label = describe(&old, &new);
            self.undo.push((old, label));
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.committed = Some((fp, new));
        self.pending = None;
    }

    fn restore(&mut self, s: Snap) {
        self.items = s.items;
        self.pairs = s.pairs;
        self.ref_choice = s.ref_choice;
        self.layout = s.layout;
        self.crop = s.crop;
        self.p = s.p;
        self.sel = self.sel.min(self.items.len().saturating_sub(1));
        self.selected.retain(|&i| i < self.items.len());
        self.committed = Some((self.fingerprint(), self.snap()));
        self.pending = None;
        if self.layout.is_some() && self.pairs.iter().any(|q| q.ok) && self.pano.is_some() {
            self.request_preview();
        } else {
            self.pano = None;
            if self.view == ViewKind::Pano {
                self.view = ViewKind::Image;
            }
        }
    }

    fn step(&mut self, redo: bool) {
        if self.busy {
            return;
        }
        self.crop_edit = None;
        self.cp = None;
        self.mask_tex = None;
        // A change still settling counts first.
        let fp = self.fingerprint();
        if self.committed.as_ref().is_some_and(|c| c.0 != fp) {
            self.commit(fp);
        }
        let from = if redo { &mut self.redo } else { &mut self.undo };
        let Some((state, label)) = from.pop() else { return };
        let cur = self.committed.take().map(|c| c.1).unwrap_or_else(|| self.snap());
        if redo { self.undo.push((cur, label.clone())) } else { self.redo.push((cur, label.clone())) }
        self.restore(state);
        let msg = crate::trf!("{}: {}. ({} undo / {} redo steps left)", if redo { tr("Redone") } else { tr("Undone") }, label, self.undo.len(), self.redo.len());
        if self.busy || self.preview_pending {
            self.note_after = Some(msg);
        } else {
            self.status = msg;
        }
    }

    // ---------------- projects ----------------

    pub fn save_project(&mut self, path: &str) -> Result<String, String> {
        use toml::{Table, Value};
        let mut path = path.to_string();
        if !path.to_lowercase().ends_with(PROJECT_EXT) {
            path += PROJECT_EXT;
        }
        let abs = std::path::absolute(&path).map_err(|e| e.to_string())?;
        let dir = abs.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
        let mut d = Table::new();
        d.insert("format".into(), "PanoStitch project".into());
        d.insert("version".into(), 1.into());
        let images: Vec<Value> = self
            .items
            .iter()
            .map(|it| {
                let mut t = Table::new();
                let ap = std::path::absolute(&it.path).unwrap_or_else(|_| it.path.clone().into());
                t.insert("path".into(), ap.to_string_lossy().to_string().into());
                t.insert("relative".into(), super::relpath(&ap, &dir).into());
                t.insert("offset".into(), Value::Array(vec![it.offset[0].into(), it.offset[1].into()]));
                t.insert("gain".into(), it.gain.into());
                if !it.corr.is_none() {
                    // Corrections by hand: turn (°, clockwise) and lens distortion added (k1, k2).
                    t.insert("turn".into(), it.corr.roll.into());
                    t.insert("lens_k1".into(), it.corr.k1.into());
                    t.insert("lens_k2".into(), it.corr.k2.into());
                }
                if !it.mask.is_empty() {
                    // Source mask: dabs [x, y, radius, erase (0/1)] in the image's pixels.
                    t.insert("mask".into(), Value::Array(it.mask.iter().map(|d| Value::Array([d.x, d.y, d.r, if d.erase { 1.0 } else { 0.0 }].into_iter().map(Value::from).collect())).collect()));
                }
                Value::Table(t)
            })
            .collect();
        d.insert("images".into(), Value::Array(images));
        let mut l = Table::new();
        l.insert("raw_decoder".into(), self.p.decoder.name().into());
        l.insert("half_size".into(), self.p.half.into());
        d.insert("loading".into(), Value::Table(l));
        let mut q = self.p.clone();
        d.insert("sliders".into(), Value::Table(sliders(&mut q).into_iter().map(|(n, v, _)| (n.to_string(), Value::from(v))).collect()));
        let mut m = Table::new();
        m.insert("pairs".into(), layout_name(self.p.layout).into());
        m.insert("model".into(), self.p.model.name().into());
        m.insert("lens distortion".into(), self.p.lens_fit.name().into());
        m.insert("projection".into(), self.p.proj.name().into());
        m.insert("blend".into(), self.p.blend.name().into());
        d.insert("menus".into(), Value::Table(m));
        let mut t = Table::new();
        t.insert("snake".into(), self.p.snake.into());
        t.insert("refine".into(), self.p.refine.into());
        t.insert("distortion per image".into(), self.p.lens_own.into());
        t.insert("plate_solve".into(), self.p.solve.into());
        t.insert("fill_empty".into(), self.p.autofill.into());
        t.insert("show unlinked images".into(), self.p.unlinked.into());
        t.insert("overlays".into(), self.overlays.into());
        d.insert("toggles".into(), Value::Table(t));
        d.insert("reference".into(), (self.ref_choice.map(|r| r + 1).unwrap_or(0) as i64).into());
        let arr = |v: Vec<f64>| Value::Array(v.into_iter().map(Value::from).collect());
        let pairs: Vec<Value> = self
            .pairs
            .iter()
            .map(|pr| {
                let mut t = Table::new();
                t.insert("a".into(), ((pr.i + 1) as i64).into());
                t.insert("b".into(), ((pr.j + 1) as i64).into());
                t.insert("H".into(), Value::Array((0..3).map(|r| arr((0..3).map(|c| pr.h[(r, c)]).collect())).collect()));
                t.insert("inliers".into(), Value::Array(pr.inl.iter().map(|&k| Value::from((k + 1) as i64)).collect()));
                t.insert("x1".into(), arr(pr.p1.iter().map(|q| q[0]).collect()));
                t.insert("y1".into(), arr(pr.p1.iter().map(|q| q[1]).collect()));
                t.insert("x2".into(), arr(pr.p2.iter().map(|q| q[0]).collect()));
                t.insert("y2".into(), arr(pr.p2.iter().map(|q| q[1]).collect()));
                t.insert("accepted".into(), pr.ok.into());
                t.insert("listed".into(), pr.listed.into());
                t.insert("message".into(), pr.msg.clone().into());
                if pr.stars {
                    t.insert("stars".into(), true.into());
                }
                if pr.manual {
                    t.insert("manual".into(), true.into());
                }
                Value::Table(t)
            })
            .collect();
        d.insert("pairs".into(), Value::Array(pairs));
        if let Some(c) = self.crop {
            d.insert("crop".into(), arr(vec![c.0, c.1, c.2, c.3]));
        }
        std::fs::write(&path, toml::to_string(&d).map_err(|e| e.to_string())?).map_err(|e| crate::trf!("Cannot save {}: {}", path, e))?;
        self.project = Some(path.clone());
        self.saved_fp = Some(self.fingerprint());
        super::remember_project(&path);
        Ok(path)
    }

    pub fn open_project(&mut self, path: &str) {
        if self.busy {
            self.status = tr("Busy – wait for the current step to finish.").into();
            return;
        }
        match self.parse_project(path) {
            Ok((paths, pd)) => {
                super::remember_project(path);
                self.pending_project = Some(pd);
                self.items.clear();
                self.list_changed();
                self.add_paths_keep(paths);
            }
            Err(e) => self.status = crate::trf!("Error: {}", e),
        }
    }

    fn parse_project(&mut self, path: &str) -> Result<(Vec<String>, ProjData), String> {
        use toml::Value;
        let text = std::fs::read_to_string(path).map_err(|e| crate::trf!("Project file not found: {} ({})", path, e))?;
        let d: toml::Table = text.parse().map_err(|e| crate::trf!("Cannot read {}: {}", basename(path), e))?;
        if d.get("format").and_then(|v| v.as_str()) != Some("PanoStitch project") {
            return Err(crate::trf!("{} is not a PanoStitch project.", basename(path)));
        }
        let dir = std::path::absolute(path).map_err(|e| e.to_string())?.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let num = |v: &Value| v.as_float().or_else(|| v.as_integer().map(|i| i as f64));
        let tbl = |k: &str| d.get(k).and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let l = tbl("loading");
        if let Some(dec) = l.get("raw_decoder").and_then(|v| v.as_str()).and_then(RawDecoder::from_name) {
            self.p.decoder = if dec == RawDecoder::Auto || io::available_raw_decoders().contains(&dec) { dec } else { RawDecoder::Auto };
        }
        if let Some(h) = l.get("half_size").and_then(|v| v.as_bool()) {
            self.p.half = h;
        }
        let sl = tbl("sliders");
        let mut q = self.p.clone();
        for (n, _, mut set) in sliders(&mut q) {
            if let Some(v) = sl.get(n).and_then(num) {
                set(&mut self.p, v);
            }
        }
        let m = tbl("menus");
        let ms = |k: &str| m.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if let Some(l) = [Layout::All, Layout::Sequential, Layout::Grid].into_iter().find(|l| layout_name(*l) == ms("pairs")) {
            self.p.layout = l;
        }
        if let Some(v) = Model::from_name(&ms("model")) {
            self.p.model = v;
        }
        if let Some(v) = LensFit::from_name(&ms("lens distortion")) {
            self.p.lens_fit = v;
        }
        if let Some(v) = Projection::from_name(&ms("projection")) {
            self.p.proj = v;
        }
        if let Some(v) = Blend::from_name(&ms("blend")) {
            self.p.blend = v;
        }
        let t = tbl("toggles");
        let tb = |k: &str| t.get(k).and_then(|v| v.as_bool());
        self.p.snake = tb("snake").unwrap_or(self.p.snake);
        self.p.refine = tb("refine").unwrap_or(self.p.refine);
        self.p.lens_own = tb("distortion per image").unwrap_or(self.p.lens_own);
        self.p.solve = tb("plate_solve").unwrap_or(self.p.solve);
        self.p.autofill = tb("fill_empty").unwrap_or(self.p.autofill);
        self.p.unlinked = tb("show unlinked images").unwrap_or(self.p.unlinked);
        self.overlays = tb("overlays").unwrap_or(self.overlays);
        // Find the images: stored absolute path, else relative to the project file.
        let recs = d.get("images").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let mut paths = Vec::new();
        let mut offsets = Vec::new();
        let mut corrs = Vec::new();
        let mut masks = Vec::new();
        let mut missing = Vec::new();
        for r in &recs {
            let ap = r.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let rel = dir.join(r.get("relative").and_then(|v| v.as_str()).unwrap_or(""));
            let found = if std::path::Path::new(ap).is_file() { Some(ap.to_string()) } else if rel.is_file() { Some(rel.to_string_lossy().to_string()) } else { None };
            let off = r.get("offset").and_then(|v| v.as_array()).map(|a| [a.first().and_then(num).unwrap_or(0.0), a.get(1).and_then(num).unwrap_or(0.0)]).unwrap_or([0.0; 2]);
            offsets.push((off, r.get("gain").and_then(num).unwrap_or(1.0)));
            let rn = |k: &str| r.get(k).and_then(num).filter(|v| v.is_finite()).unwrap_or(0.0);
            corrs.push(Correction { roll: rn("turn"), k1: rn("lens_k1"), k2: rn("lens_k2") });
            masks.push(
                r.get("mask")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|d| d.as_array()).filter(|d| d.len() == 4).filter_map(|d| Some(MaskDab { x: num(&d[0])?, y: num(&d[1])?, r: num(&d[2])?, erase: num(&d[3])? > 0.5 })).collect())
                    .unwrap_or_default(),
            );
            match found {
                Some(p) => paths.push(p),
                None => {
                    missing.push(basename(ap));
                    paths.push(String::new());
                }
            }
        }
        let fv = |v: Option<&Value>| -> Vec<f64> { v.and_then(|a| a.as_array()).map(|a| a.iter().filter_map(num).collect()).unwrap_or_default() };
        let mut pairs = Vec::new();
        for r in d.get("pairs").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            let (Some(a), Some(b)) = (r.get("a").and_then(|v| v.as_integer()), r.get("b").and_then(|v| v.as_integer())) else { continue };
            let rows: Vec<Vec<f64>> = r.get("H").and_then(|v| v.as_array()).map(|a| a.iter().map(|row| fv(Some(row))).collect()).unwrap_or_default();
            if rows.len() != 3 || rows.iter().any(|r| r.len() != 3) {
                continue;
            }
            let h = M3::new(rows[0][0], rows[0][1], rows[0][2], rows[1][0], rows[1][1], rows[1][2], rows[2][0], rows[2][1], rows[2][2]);
            let pts = |x: &str, y: &str| fv(r.get(x)).into_iter().zip(fv(r.get(y))).map(|(a, b)| [a, b]).collect::<Vec<Pt>>();
            let (p1, p2) = (pts("x1", "y1"), pts("x2", "y2"));
            let inl: Vec<usize> = fv(r.get("inliers")).into_iter().map(|k| k as usize - 1).filter(|&k| k < p1.len()).collect();
            pairs.push(PairRes {
                i: a as usize - 1,
                j: b as usize - 1,
                p1: Arc::new(p1),
                p2: Arc::new(p2),
                h,
                inl: Arc::new(inl),
                ok: r.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false),
                listed: r.get("listed").and_then(|v| v.as_bool()).unwrap_or(true),
                msg: r.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                stars: r.get("stars").and_then(|v| v.as_bool()).unwrap_or(false),
                manual: r.get("manual").and_then(|v| v.as_bool()).unwrap_or(false),
            });
        }
        let reference = d.get("reference").and_then(|v| v.as_integer()).filter(|&r| r > 0).map(|r| r as usize - 1);
        let crop = d.get("crop").map(|v| fv(Some(v))).filter(|c| c.len() == 4).map(|c| (c[0], c[1], c[2], c[3]));
        Ok((paths, ProjData { path: path.to_string(), offsets, corrs, masks, missing, pairs, reference, crop }))
    }

    /// Images of an opened project, placed as saved. `items[k]` belongs to record k.
    fn apply_project(&mut self, items: Vec<Option<Item>>, failed: Vec<String>, pd: ProjData) {
        let mut newidx = vec![None; items.len()];
        self.items.clear();
        for (k, it) in items.into_iter().enumerate() {
            if let Some(mut it) = it {
                (it.offset, it.gain) = pd.offsets[k];
                it.corr = pd.corrs.get(k).copied().unwrap_or_default();
                it.mask = Arc::new(pd.masks.get(k).cloned().unwrap_or_default());
                newidx[k] = Some(self.items.len());
                self.items.push(it);
            }
        }
        self.pairs = pd
            .pairs
            .into_iter()
            .filter_map(|mut q| {
                let (i, j) = (newidx.get(q.i).copied().flatten()?, newidx.get(q.j).copied().flatten()?);
                (q.i, q.j) = (i, j);
                Some(q)
            })
            .collect();
        self.ref_choice = pd.reference.and_then(|r| newidx.get(r).copied().flatten());
        self.sel = 0;
        self.selected = if self.items.is_empty() { Default::default() } else { [0].into_iter().collect() };
        self.pano = None;
        let note = self.relayout_now();
        self.crop = pd.crop;
        self.undo.clear();
        self.redo.clear();
        self.committed = None;
        self.project = Some(pd.path.clone());
        self.saved_fp = Some(self.fingerprint());
        let failed: Vec<String> = failed.into_iter().filter(|f| !f.is_empty()).collect();
        self.status = crate::trf!("Opened {}: {} images, {} pairs.{}{}{}", basename(&pd.path), self.items.len(), self.pairs.iter().filter(|q| q.ok).count(), note, if pd.missing.is_empty() { String::new() } else { format!(" Missing files: {}.", pd.missing.join(", ")) }, if failed.is_empty() { String::new() } else { format!(" Could not load: {}.", failed.join("; ")) });
        if self.pairs.iter().any(|q| q.ok) {
            self.vstate.fit = true;
            self.note_after = Some(self.status.clone());
            self.request_preview();
        } else {
            self.view = ViewKind::Image;
        }
    }

    /// Estimated memory and disk space for saving the area `box_` as a file of type `ext`.
    fn save_needs(&self, box_: (f64, f64, f64, f64), ext: &str) -> SaveNeeds {
        let scale = self.out_scale();
        let nat = self.layout.as_ref().and_then(|l| self.items.get(l.ref_)).map(|it| it.native).unwrap_or(1.0);
        let (w0, h0) = (box_.1 - box_.0 + 1.0, box_.3 - box_.2 + 1.0);
        // Photos read again at full size are kept in half precision (6 bytes per pixel).
        let reread = if scale > 1.0 { self.visible().iter().map(|&i| &self.items[i]).filter(|it| it.native > 1.0).map(|it| (it.img.w * it.img.h) as f64 * it.native.powi(2) * 6.0).sum() } else { 0.0 };
        let (w, h) = ((w0 * scale).round().max(1.0), (h0 * scale).round().max(1.0));
        // The panorama is kept in the file's own format and rendered in bands of
        // at least BAND_MIN rows (the smart blend keeps several layers per pixel).
        let work = if self.p.blend == Blend::Smart { 64.0 } else { 32.0 };
        let per_px = out_bytes_per_px(ext) + work * (BAND_MIN / h).min(1.0);
        let bpp = match ext {
            "jpg" | "jpeg" => 1.0,
            "fits" | "fit" | "fts" | "xisf" => 12.0,
            _ => 6.0, // 16-bit TIFF / PNG, uncompressed at most
        };
        SaveNeeds { w, h, w0, h0, nat, reread, per_px, mem: reread + w * h * per_px, file: w * h * bpp + 50e6 }
    }

    /// Render scale of the saved panorama (output px per loaded px of the reference photo).
    fn out_scale(&self) -> f64 {
        let nat = self.layout.as_ref().and_then(|l| self.items.get(l.ref_)).map(|it| it.native).unwrap_or(1.0);
        self.p.out_pct / 100.0 * nat
    }

    /// Size (w, h) in px the saved panorama will have, from the last preview's area.
    fn out_size(&self) -> Option<(usize, usize)> {
        let p = self.pano.as_ref()?;
        let s = self.out_scale();
        let (w, h) = (p.img.w as f64 / p.scale, p.img.h as f64 / p.scale);
        Some((((w * s).round() as usize).max(1), ((h * s).round() as usize).max(1)))
    }

    fn save_full(&mut self, out: String) {
        let Some((f, c, g, vis, lens)) = self.geometry() else { return };
        let p = self.p.clone();
        let o: Vec<Vec<Pt>> = vis.iter().map(|&i| outline(&g[i], self.items[i].img.h, self.items[i].img.w, p.proj, f, c, lens[i], 24)).collect();
        let lenses: Vec<Lens> = vis.iter().map(|&i| lens[i]).collect();
        let box_ = self.crop.unwrap_or_else(|| bounds(&o));
        let scale = self.out_scale();
        let (w, h) = (((box_.1 - box_.0 + 1.0) * scale).round().max(1.0), ((box_.3 - box_.2 + 1.0) * scale).round().max(1.0));
        if w.max(h) > 60_000.0 {
            self.status = crate::trf!("The panorama would be {}×{} px; lower the output size, check the pairs or use another projection.", w, h);
            return;
        }
        let (imgs, mut scales, sizes, gains, gv) = self.compose_inputs(&vis, &g, scale);
        let sky = self.sky_job(&vis, &g);
        let force = self.force_job(&vis);
        let linear = self.save_linear.unwrap_or_else(|| io::is_astro(&out));
        // Above the loaded resolution, read RAWs loaded at half size again at full size.
        let reread: Vec<(usize, String)> = vis.iter().enumerate().filter(|(_, &i)| self.items[i].native > 1.0 && scale > 1.0).map(|(k, &i)| (k, self.items[i].path.clone())).collect();
        if let Err(e) = crate::sys::check_writable(&out) {
            self.status = e;
            return;
        }
        // Enough memory and disk space? Running out would make the whole system stop responding.
        let dir = std::path::Path::new(&out).parent().filter(|d| !d.as_os_str().is_empty()).map(|d| d.to_path_buf()).unwrap_or_else(|| ".".into());
        let ext = io::ext_of(&out);
        let nd = self.save_needs(box_, &ext);
        if let Err(e) = nd.check(&dir, true) {
            self.status = e;
            return;
        }
        let opt = LoadOptions { decoder: p.decoder, half: false, ..Default::default() };
        let cropped = self.crop.is_some();
        let meta = (vis.len(), self.items.len(), cropped);
        let out_name = out.clone();
        let started = self.spawn(tr("Rendering the panorama…"), move |tx| {
            let mut srcs: Vec<Source> = imgs.into_iter().map(Source::Full).collect();
            let nre = reread.len();
            for (n, (k, path)) in reread.iter().enumerate() {
                let _ = tx.send(Msg::Status(crate::trf!("Reading {}/{} at full size: {}", n + 1, nre, basename(path))));
                if let Ok(full) = io::load_any(path, &opt) {
                    let (h0, w0) = sizes[*k];
                    let (rx, ry) = (full.w as f64 / w0 as f64, full.h as f64 / h0 as f64);
                    // Only if it is the same picture, just larger (same orientation and aspect).
                    if rx > 1.0 && (rx - ry).abs() < 0.02 * rx {
                        srcs[*k] = Source::Half(HalfImg::from_img(&full));
                        scales[*k] = rx;
                    }
                }
            }
            let refs: Vec<&Source> = srcs.iter().collect();
            let sky = sky.map(|j| j.finish(tx));
            let cp = ComposeParams { scale, projection: p.proj, f, c, blend: p.blend, lenses, sharpness: 3.0, deghost: 0.12, levels: None, sky, force };
            let (wu, hu) = (w as usize, h as usize);
            let astro = io::is_astro(&out);
            // The panorama in the file's own format, filled band by band.
            let mut buf = match out_bytes_per_px(&io::ext_of(&out)) as usize {
                3 => OutBuf::U8(vec![0; wu * hu * 3]),
                6 => OutBuf::U16(vec![0; wu * hu * 3]),
                _ => OutBuf::F32(Img::new(hu, wu)),
            };
            // Bands as large as half the free memory allows (one band = the whole panorama when it fits).
            let work = if p.blend == Blend::Smart { 64.0 } else { 32.0 };
            let free = (crate::sys::available_memory() - crate::sys::memory_reserve()).max(0.0);
            let band_rows = ((0.5 * free / (work * wu as f64)) as usize).max(BAND_MIN as usize);
            let bands = hu.div_ceil(band_rows.min(hu).max(1));
            let mut done_rows = 0usize;
            let mut cover = p.autofill.then(|| crate::fill::Cover::new(hu, wu));
            let (wc, hc) = compose_bands(&refs, &gv, box_, &cp, &gains, &scales, &sizes, band_rows, p.autofill, &mut |y0, img, cov| {
                if let (Some(all), Some(c)) = (cover.as_mut(), cov) {
                    all.set_rows(y0, &c.v);
                }
                let img = if linear { img } else { look(&img, 1.0, &p) };
                if img.w != wu || y0 + img.h > hu {
                    return Err(crate::trf!("internal error: band {}×{} at row {} of {}×{}", img.w, img.h, y0, wu, hu));
                }
                let (a, b) = (y0 * wu * 3, (y0 + img.h) * wu * 3);
                match &mut buf {
                    OutBuf::U8(v) => v[a..b].copy_from_slice(&io::to_rgb8(&img)),
                    OutBuf::U16(v) => v[a..b].copy_from_slice(&io::to_rgb16(&img)),
                    OutBuf::F32(full) => full.px[y0 * wu..(y0 + img.h) * wu].copy_from_slice(&img.px),
                }
                done_rows += img.h;
                if bands > 1 {
                    let _ = tx.send(Msg::Status(crate::trf!("Rendering the panorama ({}×{} px) in {} bands: {} %", w, h, bands, 100 * done_rows / hu)));
                }
                Ok(())
            })?;
            if (wc, hc) != (wu, hu) {
                return Err(crate::trf!("internal error: rendered {}×{} px, expected {}×{}", wc, hc, wu, hu));
            }
            let mut filled = 0usize;
            if let Some(cover) = &cover {
                let status = |_: &str| {
                    let _ = tx.send(Msg::Status(tr("Filling the empty areas…").into()));
                };
                let mut canvas = BufCanvas { buf: &mut buf, w: wu, h: hu };
                filled = crate::fill::complete(&mut canvas, cover, FILL_WORK, &status);
            }
            let _ = tx.send(Msg::Status(crate::trf!("Writing {}…", basename(&out))));
            match buf {
                OutBuf::U8(v) => io::save_rgb8(&out, wu, hu, v, 95)?,
                OutBuf::U16(v) => io::save_rgb16(&out, wu, hu, v)?,
                OutBuf::F32(img) => io::save_panorama(&out, &img)?,
            }
            let fill_note = if filled > 0 { crate::trf!(", {} % made up to fill the empty areas", format!("{:.1}", 100.0 * filled as f64 / (wu * hu) as f64)) } else { String::new() };
            Ok(Msg::Done(crate::trf!("Saved {} ({}×{} px, {} images{}{}{}).", out, wu, hu, refs.len(), if cropped { tr(", cropped") } else { "" }, if linear { if astro { tr(", linear 32-bit") } else { tr(", linear") } } else if p.stretched() { ", stretched as shown" } else { "" }, fill_note)))
        });
        if started {
            self.job_is_save = true;
            self.saving_to = Some(out_name);
            self.save_meta = meta;
        }
    }

    /// The summary shown when saving a panorama has finished or failed.
    fn save_report(&self, ok: Result<&str, &str>) -> super::JobReport {
        let (used, total, cropped) = self.save_meta;
        let secs = self.job_t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
        let mut r = match ok {
            Ok(_) => super::JobReport::new(tr("Panorama saved"), true),
            Err(e) => super::JobReport::new(tr("Panorama not saved"), false).row(tr("Error"), e.to_string()),
        };
        r = r.row(tr("Photos"), if used == total { format!("{used}") } else { crate::trf!("{} of {} (the others did not match)", used, total) });
        if self.work_secs > 0.0 {
            r = r.row(tr("Stitching"), crate::trf!("{} (loading, matching, previews)", super::duration(self.work_secs)));
        }
        r = r.row(tr("Saving"), super::duration(secs));
        if let (Ok(msg), Some(p)) = (ok, &self.saving_to) {
            // "Saved <path> (W×H px, …)": the size from the message, the file from disk.
            if let Some(dims) = msg.rsplit_once(" (").and_then(|(_, t)| t.split(" px").next()) {
                let mp = dims.split_once('×').and_then(|(a, b)| Some(a.parse::<f64>().ok()? * b.parse::<f64>().ok()? / 1e6));
                r = r.row(tr("Size"), format!("{dims} px{}{}", mp.map(|m| format!(" ({m:.1} MP)")).unwrap_or_default(), if cropped { ", cropped" } else { "" }));
            }
            r = r.row(tr("File"), format!("{} ({})", basename(p), super::file_size(p))).file(p);
        }
        r
    }

    fn poll(&mut self, ctx: &egui::Context) {
        self.poll_messages(ctx);
        // A job has ended: count its time (saving is reported on its own).
        if !self.busy {
            if let Some(t0) = self.job_t0.take() {
                if !self.job_is_save {
                    self.work_secs += t0.elapsed().as_secs_f64();
                }
                self.job_is_save = false;
            }
        }
    }

    fn poll_messages(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Status(s) => self.status = s,
                Msg::Error(e) => {
                    if self.job_is_save {
                        self.report = Some(self.save_report(Err(&e)));
                        self.saving_to = None;
                        self.post_after = false;
                    }
                    self.status = crate::trf!("Error: {}", e);
                    self.busy = false;
                    self.preview_pending = false;
                    self.note_after = None;
                }
                Msg::Done(s) => {
                    if self.job_is_save {
                        if let Some(p) = self.saving_to.clone() {
                            if s.starts_with(&format!("Saved {p} ")) {
                                self.report = Some(self.save_report(Ok(&s)));
                                if std::mem::take(&mut self.post_after) {
                                    self.to_finish.push(p.clone());
                                }
                                self.last_pano = Some(p);
                            }
                        }
                        self.saving_to = None;
                    }
                    self.status = s;
                    self.busy = false;
                }
                Msg::Detail(key, img, box_, scale) => {
                    self.detail_busy = false;
                    if self.pano.is_some() {
                        let tex = texture(ctx, "detail", &self.shown_img(&img, 1.0, self.pano.as_ref().map(|p| &p.img)));
                        let tk = (self.p.black.to_bits(), self.p.mid.to_bits(), self.p.white.to_bits(), self.disp_key());
                        self.detail = Some(Detail { key, tex, tex_key: tk, img, box_, scale });
                    }
                }
                Msg::DetailFailed => self.detail_busy = false,
                Msg::SkyMaps(ms) => {
                    for (i, m) in ms {
                        if let Some(it) = self.items.get_mut(i) {
                            it.sky_map = Some(m);
                        }
                    }
                }
                Msg::Loaded(items, failed) if self.pending_project.is_some() => {
                    self.busy = false;
                    let pd = self.pending_project.take().unwrap();
                    self.apply_project(items, failed, pd);
                }
                Msg::Loaded(items, failed) => {
                    self.busy = false;
                    let items: Vec<Item> = items.into_iter().flatten().collect();
                    let (lowmem, failed): (Vec<String>, Vec<String>) = failed.into_iter().filter(|f| !f.is_empty()).partition(|f| f.starts_with(LOWMEM));
                    let lowmem_note = if lowmem.is_empty() {
                        String::new()
                    } else {
                        crate::trf!("\nMemory is almost full: {} image(s) were not loaded ({}). {}Close other programs, or stitch fewer images at a time.", lowmem.len(), lowmem.iter().map(|f| f.trim_start_matches(LOWMEM)).collect::<Vec<_>>().join(", "), if self.p.half { "" } else { "Switch on RAW at half size (a quarter of the memory) and add them again. " })
                    };
                    let astro = items.iter().any(|it| io::is_astro(&it.path));
                    let n = items.len();
                    let n0 = self.items.len();
                    self.items.extend(items);
                    self.sel = self.items.len().saturating_sub(1);
                    self.selected = [self.sel].into_iter().collect();
                    let old_of: Vec<Option<usize>> = (0..self.items.len()).map(|k| (k < n0).then_some(k)).collect();
                    self.list_changed_from(&old_of);
                    self.view = ViewKind::Image;
                    self.vstate.fit = true;
                    let mut note = String::new();
                    if astro && !self.p.stretched() {
                        // FITS / XISF data is usually linear: stretch for viewing.
                        let all: Vec<[f32; 3]> = self.items.iter().filter(|it| io::is_astro(&it.path)).flat_map(|it| it.disp.px.iter().step_by(7).copied().collect::<Vec<_>>()).collect();
                        let (b, m, w) = auto_stretch(&Img::from_vec(1, all.len(), all), 0.25, 2.8);
                        (self.p.black, self.p.mid, self.p.white) = (b, m, w);
                        note = tr(" Auto stretch applied for viewing (FITS / XISF data is linear).").into();
                    }
                    self.status = format!("Loaded {n} image(s). Next: Run all steps (or detect, match, preview).{note}{}{lowmem_note}", if failed.is_empty() { String::new() } else { format!("\nSkipped {}:\n{}", failed.len(), failed.join("\n")) });
                }
                Msg::Detected(feats) => {
                    self.busy = false;
                    let changed: Vec<usize> = feats.iter().map(|f| f.0).collect();
                    for (i, f, key) in feats {
                        if let Some(it) = self.items.get_mut(i) {
                            it.feats = Some(f);
                            it.featkey = Some(key);
                        }
                    }
                    // Pairs of images with new features are stale.
                    self.pairs.retain(|q| !changed.contains(&q.i) && !changed.contains(&q.j));
                    if !changed.is_empty() {
                        let _ = self.relayout_now();
                    }
                    self.view = ViewKind::Image;
                    self.status = tr("Features detected. Look for keypoints spread over the overlap areas; if too few, lower the corner threshold or raise Denoise. Then match.").into();
                }
                Msg::Matched { feats, stars, solved, pairs, layout, msg } => {
                    self.busy = false;
                    for (i, f, key) in feats {
                        if let Some(it) = self.items.get_mut(i) {
                            it.feats = Some(f);
                            it.featkey = Some(key);
                        }
                    }
                    for (i, s) in stars {
                        if let Some(it) = self.items.get_mut(i) {
                            it.stars = Some(s);
                        }
                    }
                    for (i, s) in solved {
                        if let Some(it) = self.items.get_mut(i) {
                            it.solved = Some(s);
                        }
                    }
                    self.pairs = pairs;
                    self.layout = layout;
                    self.crop = None;
                    self.status = crate::trf!("{} Inspect pairs in the Pair menu, then preview.", msg);
                    if self.preview_pending {
                        self.note_after = Some(crate::trf!("Done. {}", msg));
                    }
                    if !self.preview_pending {
                        self.view = ViewKind::Pair;
                        self.vstate.fit = true;
                    }
                }
                Msg::Preview(img, box_, scale, outlines, covered) => {
                    self.busy = false;
                    let fit = self.view != ViewKind::Pano || self.pano.is_none();
                    self.pano_version += 1;
                    self.detail = None;
                    self.pano = Some(Preview { img, tex: None, tex_key: (0, 0, 0, 0), box_, scale, outlines, covered, view: self.rendering_view });
                    self.view = ViewKind::Pano;
                    if fit {
                        self.vstate.fit = true;
                    }
                    self.status = self.note_after.take().unwrap_or_else(|| tr("Preview ready. Drag to pan, wheel to zoom (detail sharpens). Click an image to select it; Ctrl+drag or arrow keys move it.").into());
                }
            }
        }
        if !self.busy && self.preview_pending {
            self.preview_pending = false;
            self.request_preview();
        }
        if let Some((rx, what)) = &self.dialog {
            if let Ok(r) = rx.try_recv() {
                let what = *what;
                self.dialog = None;
                match r {
                    Picked::NoTool => self.status = dialogs::NO_DIALOG.into(),
                    Picked::Save(Some(p)) if what == "save" => self.save_full(p),
                    Picked::Save(Some(p)) if what == "saveproj" => match self.save_project(&p) {
                        Ok(p) => self.status = crate::trf!("Saved project {}.", p),
                        Err(e) => self.status = crate::trf!("Error: {}", e),
                    },
                    Picked::Files(ps) if what == "openproj" => {
                        if let Some(p) = ps.first() {
                            self.open_project(&p.clone());
                        }
                    }
                    Picked::Files(ps) => self.add_paths(ps),
                    Picked::Save(_) => {}
                }
            }
        }
    }

    fn tex_for(&mut self, ctx: &egui::Context, i: usize) -> egui::TextureHandle {
        let key = (self.items[i].path.clone(), self.p.ev.to_bits() ^ self.p.black.to_bits().rotate_left(7) ^ self.p.mid.to_bits().rotate_left(13) ^ self.p.white.to_bits().rotate_left(19) ^ self.disp_key().rotate_left(29));
        if let Some(t) = self.tex_cache.get(&key) {
            return t.clone();
        }
        if self.tex_cache.len() > 64 {
            self.tex_cache.clear();
        }
        let t = texture(ctx, &key.0, &self.shown_img(&self.items[i].disp, 2f64.powf(self.p.ev), None));
        self.tex_cache.insert(key, t.clone());
        t
    }

    /// The lens distortion the refinement found, in words.
    fn lens_summary(&self) -> String {
        let Some(l) = &self.layout else { return String::new() };
        if !self.p.refine || self.p.lens_fit == LensFit::Off {
            return tr("Lens distortion not fitted.").into();
        }
        let ks: Vec<(f64, f64)> = l.members.iter().filter_map(|&i| l.lenses.get(i)).map(|x| (x.k1, x.k2)).collect();
        let range = |v: Vec<f64>| {
            let (lo, hi) = (v.iter().copied().fold(f64::MAX, f64::min), v.iter().copied().fold(f64::MIN, f64::max));
            if hi - lo < 5e-4 { format!("{lo:.3}") } else { format!("{lo:.3} … {hi:.3}") }
        };
        let k1 = range(ks.iter().map(|k| k.0).collect());
        if self.p.lens_fit == LensFit::Poly {
            crate::trf!("Lens distortion: k1 = {}, k2 = {}.", k1, range(ks.iter().map(|k| k.1).collect()))
        } else {
            crate::trf!("Lens distortion: k1 = {}.", k1)
        }
    }

    /// Turn and lens distortion of the selected images, by hand. True when changed.
    fn correction_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(it) = self.items.get(self.sel) else {
            ui.label(tr("Add images first."));
            return false;
        };
        let mut c = it.corr;
        let found = self.layout.as_ref().and_then(|l| l.lenses.get(self.sel).copied());
        let mut ch = false;
        egui::Grid::new("corr_grid").num_columns(2).show(ui, |ui| {
            ch |= angle(ui, tr("Turn (°)"), &mut c.roll, 20.0, true);
            ch |= slider(ui, tr("Distortion k1 +"), &mut c.k1, -0.3..=0.3, 0.0005);
            ch |= slider(ui, tr("Distortion k2 +"), &mut c.k2, -0.3..=0.3, 0.0005);
        });
        if let Some(fl) = found {
            ui.label(egui::RichText::new(crate::trf!("Found by the refinement: k1 = {}, k2 = {}; with the correction k1 = {}, k2 = {}.", format!("{:.4}", fl.k1), format!("{:.4}", fl.k2), format!("{:.4}", fl.k1 + c.k1), format!("{:.4}", fl.k2 + c.k2))).weak().size(12.0));
        }
        ui.label(egui::RichText::new(tr("Turn: for a photo that sits a little rotated against its neighbours (about its own centre). k1 bends straight lines: − barrel, + pincushion. k2 acts mostly in the corners: with the opposite sign of k1 it corrects the wavy \"moustache\" distortion. Both are added to what the refinement found; they apply to all selected images.")).weak().size(12.0));
        let targets = self.targets();
        if ch {
            for &i in &targets {
                self.items[i].corr = c;
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Reset")).on_hover_text(tr("No turn, distortion as found (the selected images)")).clicked() {
                for &i in &targets {
                    self.items[i].corr = Correction::default();
                }
                ch = true;
            }
            if ui.button(tr("Distortion to all images")).on_hover_text(tr("The same k1 and k2 correction for every image (one lens)")).clicked() {
                for it in self.items.iter_mut() {
                    (it.corr.k1, it.corr.k2) = (c.k1, c.k2);
                }
                ch = true;
            }
        });
        ch
    }

    fn nudge(&mut self, dx: f64, dy: f64) {
        let Some(l) = &self.layout else { return };
        if self.sel == l.ref_ {
            self.status = tr("The reference image stays fixed; select another image to nudge (or use Pitch / Roll).").into();
            return;
        }
        let s = self.pano.as_ref().map(|p| p.scale).unwrap_or(1.0);
        self.items[self.sel].offset[0] += dx / s;
        self.items[self.sel].offset[1] += dy / s;
        self.request_preview();
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.poll(ctx);
        self.check_undo();
        let mut redo_preview = false;
        egui::Panel::left("stitch_left").default_size(400.0).size_range(380.0..=700.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("sl").show(ui, |ui| self.left_panel(ui));
        });
        egui::Panel::right("stitch_right").default_size(350.0).size_range(320.0..=600.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("sr").show(ui, |ui| redo_preview = self.right_panel(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.center(ui, ctx));
        if redo_preview && self.pano.is_some() {
            self.request_preview();
        }
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("⇪ Open project…")).on_hover_text(tr("Ctrl+O")).clicked() {
                self.ask_open_project();
            }
            if ui.button(tr("⬇ Save project…")).on_hover_text(tr("Ctrl+S saves to the open project at once")).clicked() {
                self.ask_save_project(false);
            }
        });
        ui.heading(tr("Images"));
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Add…")).clicked() && self.dialog.is_none() {
                self.dialog = Some((dialogs::pick_images(), "add"));
            }
            if ui.button(tr("Remove")).clicked() {
                self.remove_selected();
            }
            if ui.button(if self.simple { tr("Clear") } else { tr("Clear all") }).clicked() && !self.busy {
                self.selected = (0..self.items.len()).collect();
                self.remove_selected();
            }
            if ui.button("▲").clicked() {
                self.move_selected(-1);
            }
            if ui.button("▼").clicked() {
                self.move_selected(1);
            }
        });
        let r = ui.add(egui::TextEdit::singleline(&mut self.pathbox).hint_text(tr("…or type a file/folder path + Enter")).desired_width(ui.available_width().min(420.0)));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.pathbox.trim().is_empty() {
            let t = self.pathbox.trim().to_string();
            self.pathbox.clear();
            if t.to_lowercase().ends_with(PROJECT_EXT) {
                self.open_project(&dialogs::expand_paths(&t)[0]);
            } else {
                self.add_paths(dialogs::expand_paths(&t));
            }
        }
        let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
        let mut clicked: Option<usize> = None;
        let list = egui::ScrollArea::vertical().id_salt("list").max_height(160.0).show(ui, |ui| {
            if self.items.is_empty() {
                ui.weak(tr("(no images)"));
            }
            let refi = self.layout.as_ref().map(|l| l.ref_);
            for (i, it) in self.items.iter().enumerate() {
                let kp = it.feats.as_ref().map(|f| format!(" {}kp", f.pts.len())).unwrap_or_default();
                let unl = self.layout.as_ref().is_some_and(|l| self.pairs.iter().any(|p| p.ok) && l.comp[i] != l.comp[l.ref_]);
                let star = if it.solved.as_ref().and_then(|s| s.found()).is_some() { " ★" } else { "" };
                let txt = format!("{}{:>3} {}{kp}{star}{}", if Some(i) == refi { "R" } else { " " }, i + 1, basename(&it.path), if unl { "  UNLINKED" } else { "" });
                let on = self.selected.contains(&i) || i == self.sel;
                let mut rt = egui::RichText::new(txt).monospace();
                if i == self.sel {
                    rt = rt.strong();
                }
                if ui.selectable_label(on, rt).clicked() {
                    clicked = Some(i);
                }
            }
        });
        if let Some(k) = clicked {
            // Click selects, Ctrl-click adds / removes one, Shift-click selects a range.
            if shift && !self.items.is_empty() {
                let (a, b) = (self.sel.min(k), self.sel.max(k));
                self.selected.extend(a..=b);
            } else if ctrl {
                if !self.selected.remove(&k) {
                    self.selected.insert(k);
                }
            } else {
                self.selected = [k].into_iter().collect();
            }
            self.sel = k;
            if self.view != ViewKind::Pano {
                self.view = ViewKind::Image;
                self.vstate.fit = true;
            }
        }
        // Delete removes, Ctrl+A selects all, while the mouse is over the list.
        let over = ui.rect_contains_pointer(list.inner_rect);
        if over && !ui.ctx().egui_wants_keyboard_input() {
            let (del, all) = ui.input(|i| (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace), i.modifiers.command && i.key_pressed(egui::Key::A)));
            if del {
                self.remove_selected();
            } else if all {
                self.selected = (0..self.items.len()).collect();
            }
        }
        egui::Grid::new("load").num_columns(2).show(ui, |ui| {
            let simple = self.simple;
            if !simple {
            ui.label(tr("Reference"));
            let names: Vec<String> = self.items.iter().enumerate().map(|(i, it)| format!("{}: {}", i + 1, basename(&it.path))).collect();
            let cur = self.ref_choice.map(|r| names.get(r).cloned().unwrap_or_default()).unwrap_or_else(|| tr("auto (centre)").into());
            let before = self.ref_choice;
            egui::ComboBox::from_id_salt("ref").selected_text(cur).width(250.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.ref_choice, None, tr("auto (centre)"));
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut self.ref_choice, Some(i), n);
                }
            });
            ui.end_row();
            if before != self.ref_choice && !self.pairs.is_empty() {
                let note = self.relayout_now();
                self.status = crate::trf!("Reference changed.{}", note);
                if self.pano.is_some() {
                    self.request_preview();
                }
            }
            let mut decs = vec![RawDecoder::Auto];
            decs.extend(io::available_raw_decoders());
            combo(ui, tr("RAW via"), &mut self.p.decoder, &decs, |d| d.name());
            ui.label("");
            ui.checkbox(&mut self.p.half, tr("RAW at half size"));
            ui.end_row();
            }
            ui.label(egui::RichText::new(tr("Preset")).strong());
            egui::ComboBox::from_id_salt("preset").selected_text(tr("choose…")).show_ui(ui, |ui| {
                for name in PRESETS {
                    if ui.selectable_label(false, tr(name)).clicked() {
                        let note = self.p.preset(name);
                        self.status = crate::trf!("Preset “{}”: {} Next: Run all steps.", tr(name), note);
                    }
                }
            });
            ui.end_row();
        });
        ui.separator();
        if self.simple {
            if ui.add(egui::Button::new(egui::RichText::new(tr("▶ Run all steps")).strong())).on_hover_text(tr("Ctrl+Enter")).clicked() {
                self.match_pairs(None, true);
            }
            return;
        }
        ui.heading(tr("1 · Features"));
        egui::Grid::new("feat").num_columns(2).show(ui, |ui| {
            slider(ui, tr("Detection size (px)"), &mut self.p.det_px, 500..=6000, 100.0);
            slider(ui, tr("Max keypoints"), &mut self.p.nkp, 500..=15000, 500.0);
            slider(ui, tr("Corner threshold"), &mut self.p.thr, 0.0..=0.02, 0.0001);
            slider(ui, tr("Denoise (σ px)"), &mut self.p.denoise, 0.0..=4.0, 0.05);
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Detect features")).clicked() {
                self.detect_only();
            }
            if ui.add(egui::Button::new(egui::RichText::new(tr("▶ Run all steps")).strong())).on_hover_text(tr("Ctrl+Enter")).clicked() {
                self.match_pairs(None, true);
            }
        });
        ui.separator();
        ui.heading(tr("2 · Matching"));
        egui::Grid::new("match").num_columns(2).show(ui, |ui| {
            combo(ui, tr("Pairs"), &mut self.p.layout, &[Layout::All, Layout::Sequential, Layout::Grid], |l| match l { Layout::All => "all pairs", Layout::Sequential => "sequential", Layout::Grid => "grid" }); // tr
            if self.p.layout == Layout::Grid {
                slider(ui, tr("Grid columns"), &mut self.p.cols, 1..=60, 1.0);
                ui.label("");
                ui.checkbox(&mut self.p.snake, tr("snake (every 2nd row reversed)"));
                ui.end_row();
            }
            slider(ui, tr("Ratio test"), &mut self.p.ratio, 0.5..=0.95, 0.005);
            slider(ui, tr("Max distance"), &mut self.p.maxd, 0.1..=0.5, 0.005);
            slider(ui, tr("RANSAC tol. (px)"), &mut self.p.tol, 0.5..=20.0, 0.1);
            slider(ui, tr("Min. inliers"), &mut self.p.mininl, 6..=100, 1.0);
            combo(ui, tr("Model"), &mut self.p.model, &Model::ALL, |m| m.name());
            ui.label("");
            if ui.checkbox(&mut self.p.refine, tr("global refinement")).changed() && !self.pairs.is_empty() {
                let note = self.relayout_now();
                self.status = crate::trf!("Global refinement {}.{}", if self.p.refine { tr("on") } else { tr("off") }, note);
            }
            ui.end_row();
            if self.p.refine && self.p.model == Model::Homography {
                let before = (self.p.lens_fit, self.p.lens_own);
                combo(ui, tr("Lens distortion"), &mut self.p.lens_fit, &LensFit::ALL, |m| m.name());
                if self.p.lens_fit != LensFit::Off {
                    ui.label("");
                    ui.checkbox(&mut self.p.lens_own, tr("each image its own distortion")).on_hover_text(tr("Off: one distortion for all photos (one lens at one focal length). On: each photo gets its own, kept close to the common one where its points say little: for a zoom that changed between the shots, or photos from different lenses. Each photo's rotation is always refined on its own."));
                    ui.end_row();
                }
                if (self.p.lens_fit, self.p.lens_own) != before && !self.pairs.is_empty() {
                    let note = self.relayout_now();
                    self.status = format!("{}{note}", self.lens_summary());
                }
            }
            ui.label("");
            ui.checkbox(&mut self.p.solve, tr("plate solve photos of the sky")).on_hover_text(tr("Photos that are (nearly) all sky are found on a star catalogue (Hipparcos and Tycho-2, to magnitude 8). Two that overlap are linked by the stars identified in both: pairs the matching missed are linked, and a wrong match is corrected. Photos with landscape are left to the other matching. Takes under a second per photo."));
            ui.end_row();
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Match")).clicked() {
                self.match_pairs(None, false);
            }
            let cur = self.pairs.get(self.pairsel).map(|q| (q.i, q.j));
            if ui.button(tr("Re-match pair")).clicked() {
                match cur {
                    Some(ij) => self.match_pairs(Some(vec![ij]), false),
                    None => self.status = tr("Choose a pair first.").into(),
                }
            }
            if ui.button(tr("✚ Control points…")).on_hover_text(tr("Place matching points by hand on two images, as in PTGui: for pairs the program could not match or matched wrongly")).clicked() {
                self.open_cp();
            }
            if ui.button(tr("Discard pair")).clicked() && !self.busy {
                if let Some(q) = self.pairs.get_mut(self.pairsel) {
                    q.ok = false;
                    q.msg = tr("Discarded by you. Use Re-match pair to try it again.").into();
                    let note = self.relayout_now();
                    self.status = crate::trf!("Pair discarded; placement updated.{}", note);
                    if self.pano.is_some() {
                        self.request_preview();
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Pair"));
            let label = |q: &PairRes| format!("{}–{}  {}", q.i + 1, q.j + 1, if q.ok { crate::trf!("{} inliers", q.inl.len()) } else { tr("rejected").into() });
            let cur = self.pairs.get(self.pairsel).filter(|q| q.listed).map(label).unwrap_or_else(|| tr("(none)").into());
            egui::ComboBox::from_id_salt("pair").selected_text(cur).width(150.0).show_ui(ui, |ui| {
                for (k, q) in self.pairs.iter().enumerate().filter(|(_, q)| q.listed) {
                    if ui.selectable_label(k == self.pairsel, label(q)).clicked() {
                        self.pairsel = k;
                        self.view = ViewKind::Pair;
                        self.vstate.fit = true;
                    }
                }
            });
            ui.checkbox(&mut self.show_outliers, tr("outliers"));
        });
        if let Some(q) = self.pairs.get(self.pairsel) {
            ui.label(egui::RichText::new(&q.msg).size(12.5));
        }
    }

    /// Returns whether the preview should be re-rendered.
    fn right_panel(&mut self, ui: &mut egui::Ui) -> bool {
        let mut redo = false;
        ui.heading(tr("3 · Projection"));
        egui::Grid::new("proj").num_columns(2).show(ui, |ui| {
            if combo(ui, tr("Projection"), &mut self.p.proj, &Projection::ALL, |p| p.name()) {
                self.crop = None;
                self.vstate.fit = true;
                redo = true;
            }
            redo |= slider(ui, tr("Focal (× auto)"), &mut self.p.focal, 0.3..=3.0, 0.005);
            redo |= angle(ui, tr("Yaw (°)"), &mut self.p.yaw, 180.0, false);
            redo |= angle(ui, tr("Pitch (°)"), &mut self.p.pitch, 90.0, true);
            redo |= angle(ui, tr("Roll (°)"), &mut self.p.roll, 45.0, true);
        });
        ui.horizontal_wrapped(|ui| {
            let ok = horizon::supported(self.p.proj);
            let off = tr("Not for this projection: it bends every level line. For rectilinear, cylindrical, spherical, Mercator, Hammer and Mollweide.");
            if ui.add_enabled(ok, egui::Button::new(tr("⟷ Auto horizon"))).on_hover_text(tr("Find the horizon in the preview, smooth it with a curve of the 4th order (trees and hills count less) and set pitch and roll so that it is straight; yaw stays. The green line shows the horizon found.")).on_disabled_hover_text(off).clicked() {
                redo |= self.auto_horizon();
            }
            if ui.add_enabled(ok, egui::Button::selectable(self.horizon_edit.is_some(), tr("✎ Horizon by hand"))).on_hover_text(tr("Click 4 or more points along the horizon on the panorama; a curve is drawn through them. Then ✔ Level horizon sets pitch and roll so that it is straight.")).on_disabled_hover_text(off).clicked() {
                self.toggle_horizon_edit();
            }
        });
        if ui.button(tr("Reset focal / yaw / pitch / roll")).clicked() {
            (self.p.focal, self.p.yaw, self.p.pitch, self.p.roll) = (1.0, 0.0, 0.0, 0.0);
            redo = true;
        }
        if redo {
            self.crop = None;
        }
        ui.separator();
        ui.heading(tr("4 · Compose"));
        let simple = self.simple;
        egui::Grid::new("comp").num_columns(2).show(ui, |ui| {
            if !simple {
                redo |= combo(ui, tr("Blend"), &mut self.p.blend, &Blend::ALL, |b| b.name());
                slider(ui, tr("Preview size (px)"), &mut self.p.prev_px, 500..=4000, 100.0);
            }
            redo |= slider(ui, tr("Exposure (EV)"), &mut self.p.ev, -3.0..=8.0, 0.05);
            if !simple && self.sel < self.items.len() {
                // Brightness applies to all selected images.
                let mut g = self.items[self.sel].gain;
                if slider(ui, tr("Brightness (sel.)"), &mut g, 0.3..=3.0, 0.005) {
                    for i in self.targets() {
                        self.items[i].gain = g;
                    }
                    redo = true;
                }
            }
        });
        if !simple && ui.checkbox(&mut self.p.unlinked, tr("show unlinked images")).changed() {
            self.crop = None;
            redo = true;
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Auto exposure")).clicked() {
                // Middle grey for the typical brightness, without clipping more than the brightest 0.5 %.
                match &self.pano {
                    Some(p) => {
                        let mut lum: Vec<f32> = p.img.px.iter().map(|c| luminance(*c)).filter(|l| *l > 0.0).collect();
                        if !lum.is_empty() {
                            let q = quantiles(&mut lum, &[0.5, 0.995]);
                            let d = ((0.18 / q[0] as f64).log2()).min((1.0 / q[1] as f64).log2()).clamp(-6.0, 10.0);
                            self.p.ev = (self.p.ev + d).clamp(-3.0, 8.0);
                            self.status = crate::trf!("Exposure set to {} EV.", format!("{:.1}", self.p.ev));
                            redo = true;
                        }
                    }
                    None => self.status = tr("Preview the panorama first.").into(),
                }
            }
            if ui.button(tr("Even out")).clicked() {
                match &self.layout {
                    Some(l) if self.pairs.iter().any(|q| q.ok) => {
                        let mut pts = HashMap::new();
                        for &v in &l.order {
                            let Some(p) = l.parent[v] else { continue };
                            if let Some(q) = self.pairs.iter().find(|q| q.ok && ((q.i, q.j) == (p, v) || (q.i, q.j) == (v, p))) {
                                let (a, b): (Vec<Pt>, Vec<Pt>) = (q.inl.iter().map(|&k| q.p1[k]).collect(), q.inl.iter().map(|&k| q.p2[k]).collect());
                                pts.insert((p, v), if q.i == p { (a, b) } else { (b, a) });
                            }
                        }
                        let imgs: Vec<&Img> = self.items.iter().map(|it| it.img.as_ref()).collect();
                        let g = estimate_gains(&imgs, &l.parent, &l.order, &pts);
                        for (it, gi) in self.items.iter_mut().zip(g.iter()) {
                            it.gain = gi.clamp(0.3, 3.0);
                        }
                        self.status = format!("Brightness: {}", g.iter().enumerate().map(|(i, g)| format!("{}: {g:.2}", i + 1)).collect::<Vec<_>>().join(", "));
                        redo = true;
                    }
                    _ => self.status = tr("Match the images first.").into(),
                }
            }
            if ui.button(tr("Reset moves")).clicked() {
                self.items.iter_mut().for_each(|it| it.offset = [0.0; 2]);
                redo = true;
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Auto crop")).clicked() {
                self.auto_crop();
            }
            if ui.button(tr("Uncrop")).clicked() && self.crop.is_some() {
                self.crop = None;
                self.vstate.fit = true;
                redo = true;
            }
        });
        if !simple {
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Nudge selected:"));
            for (l, d) in [("←", (-1.0, 0.0)), ("↑", (0.0, -1.0)), ("↓", (0.0, 1.0)), ("→", (1.0, 0.0))] {
                if ui.button(l).clicked() {
                    self.nudge(d.0, d.1);
                }
            }
        });
        egui::CollapsingHeader::new(tr("Correct selected image")).id_salt("corr").show(ui, |ui| redo |= self.correction_ui(ui));
        ui.separator();
        ui.heading(tr("5 · Stretch"));
        // The histogram of what is shown, without copying it (the Auto stretch button copies once).
        let stretch = Some((self.p.black, self.p.mid, self.p.white));
        if self.view == ViewKind::Pano && self.pano.is_some() {
            histogram_of(ui, &self.pano.as_ref().unwrap().img, 1.0, stretch);
        } else if let Some(it) = self.items.get(self.sel) {
            histogram_of(ui, &it.disp, 2f32.powf(self.p.ev as f32), stretch);
        }
        egui::Grid::new("str").num_columns(2).show(ui, |ui| {
            slider(ui, tr("Black point"), &mut self.p.black, 0.0..=0.5, 0.0005);
            slider(ui, tr("Midtones"), &mut self.p.mid, 0.0005..=0.9995, 0.0005);
            slider(ui, tr("White point"), &mut self.p.white, 0.5..=1.0, 0.0005);
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Auto stretch")).clicked() {
                if let Some(src) = self.hist_source() {
                    (self.p.black, self.p.mid, self.p.white) = auto_stretch(&src, 0.25, 2.8);
                }
            }
            if ui.button(tr("Reset stretch")).clicked() {
                (self.p.black, self.p.mid, self.p.white) = (0.0, 0.5, 1.0);
            }
        });
        }
        ui.separator();
        let bw = ui.available_width().min(310.0);
        let big = |t: &str| egui::Button::new(egui::RichText::new(t).strong().size(15.0)).min_size(egui::vec2(bw, 36.0));
        if ui.add(big(tr("◐ Preview panorama"))).on_hover_text("P").clicked() {
            self.request_preview();
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Format"));
            let cur = self.save_format.and_then(|e| io::OUTPUT_FORMATS.iter().find(|f| f.0 == e)).map(|f| f.1).unwrap_or(tr("automatic"));
            egui::ComboBox::from_id_salt("savefmt").selected_text(cur).width(240.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.save_format, None, tr("automatic")).on_hover_text(tr("TIFF; FITS or XISF when all images are"));
                for (e, d) in io::OUTPUT_FORMATS {
                    ui.selectable_value(&mut self.save_format, Some(e), d);
                }
            });
        });
        egui::Grid::new("outsize").num_columns(2).show(ui, |ui| {
            ui.label(tr("Output size"));
            ui.add(egui::Slider::new(&mut self.p.out_pct, 10.0..=200.0).step_by(5.0).suffix(" %"))
                .on_hover_text(tr("Size of the saved panorama: 100 % keeps the photos' full camera resolution (RAWs loaded at half size are read again at full size when saving), 50 % halves width and height, above 100 % enlarges."));
            ui.end_row();
            ui.label("");
            match self.out_size() {
                Some((w, h)) => ui.weak(format!("→ {w} × {h} px ({:.1} MP)", (w * h) as f64 / 1e6)),
                None => ui.weak(tr("(preview to see the size in pixels)")),
            };
            ui.end_row();
        });
        // Live estimate of what saving needs, against what is free (wrapped:
        // a long line would widen the panel over the preview).
        {
            if let (Some(pv), Some(first)) = (&self.pano, self.items.first()) {
                let auto = if self.items.iter().all(|it| io::is_fits(&it.path)) { "fits" } else if self.items.iter().all(|it| io::is_astro(&it.path)) { "xisf" } else { "tif" };
                let nd = self.save_needs(pv.box_, self.save_format.unwrap_or(auto));
                let dir = std::path::Path::new(&first.path).parent().filter(|d| !d.as_os_str().is_empty()).map(|d| d.to_path_buf()).unwrap_or_else(|| ".".into());
                let (mem, disk) = crate::sys::free_cached(&dir);
                let text = crate::trf!("needs ≈ {} memory ({} free), file ≤ {}{}", gb(nd.mem), gb((mem - memory_reserve()).max(0.0)), gb(nd.file), disk.map(|d| crate::trf!(" ({} free here)", gb(d as f64))).unwrap_or_default());
                match nd.check(&dir, false) {
                    Ok(()) => ui.add(egui::Label::new(egui::RichText::new(text).weak()).wrap()),
                    Err(e) => ui.add(egui::Label::new(egui::RichText::new(format!("⚠ {text}")).color(Color32::from_rgb(220, 120, 0))).wrap()).on_hover_text(e),
                };
            }
        }
        ui.horizontal_wrapped(|ui| {
            for (t, pct) in [("¼", 25.0), ("½", 50.0), (tr("full"), 100.0), ("2×", 200.0)] {
                if ui.selectable_label(self.p.out_pct == pct, t).clicked() {
                    self.p.out_pct = pct;
                }
            }
        });
        {
            let auto = if self.items.iter().all(|it| io::is_fits(&it.path)) { "fits" } else if self.items.iter().all(|it| io::is_astro(&it.path)) { "xisf" } else { "tif" };
            let ext = self.save_format.unwrap_or(auto);
            let linear = self.save_linear.unwrap_or_else(|| matches!(ext, "fits" | "fit" | "xisf"));
            if let Some(v) = super::save_kind_ui(ui, linear, ext) {
                self.save_linear = Some(v);
            }
        }
        if ui.add(big(tr("⬇ Save panorama…"))).on_hover_text(tr("Ctrl+Shift+S")).clicked() {
            self.ask_save_pano();
        }
        if let Some(p) = self.last_pano.clone() {
            if ui.button(tr("✎ Post-process")).on_hover_text(crate::trf!("Open {} in the Post-processing tab: crop, curves, colour, noise", basename(&p))).clicked() {
                self.to_finish.push(p);
            }
        }
        if let Some(r) = &self.report {
            ui.add_space(4.0);
            if r.ui(ui) {
                self.report = None;
            }
        }
        redo
    }

    fn ask_open_project(&mut self) {
        if self.dialog.is_none() {
            self.dialog = Some((dialogs::pick_open(tr("Open project"), tr("PanoStitch projects"), PROJECT_EXT), "openproj"));
        }
    }

    /// Save the project: `quick` saves straight to the open project file, if there is one.
    pub fn ask_save_project(&mut self, quick: bool) {
        if self.items.is_empty() {
            self.status = tr("Nothing to save yet.").into();
            return;
        }
        if quick {
            if let Some(p) = self.project.clone() {
                self.status = match self.save_project(&p) {
                    Ok(p) => crate::trf!("Saved project {}.", p),
                    Err(e) => crate::trf!("Error: {}", e),
                };
                return;
            }
        }
        if self.dialog.is_none() {
            let default = self.project.clone().unwrap_or_else(|| {
                std::path::Path::new(&self.items[0].path).parent().unwrap_or(std::path::Path::new(".")).join(format!("project{PROJECT_EXT}")).to_string_lossy().to_string()
            });
            self.dialog = Some((dialogs::pick_save(tr("Save project"), &default), "saveproj"));
        }
    }

    /// Whether there are images whose state is not saved in a project file.
    pub fn unsaved(&self) -> bool {
        !self.items.is_empty() && self.saved_fp != Some(self.fingerprint())
    }

    fn ask_save_pano(&mut self) {
        self.post_after = false;
        if self.crop_edit.is_some() {
            self.toggle_crop();
        }
        if self.layout.is_none() {
            self.status = tr("Match the images first (Run all steps).").into();
            return;
        }
        if self.dialog.is_none() {
            let auto = if self.items.iter().all(|it| io::is_fits(&it.path)) { "fits" } else if self.items.iter().all(|it| io::is_astro(&it.path)) { "xisf" } else { "tif" };
            let ext = self.save_format.unwrap_or(auto);
            let dir = std::path::Path::new(&self.items[0].path).parent().map(|d| d.to_path_buf()).unwrap_or_default();
            let default = dir.join(format!("panorama.{ext}")).to_string_lossy().to_string();
            self.dialog = Some((dialogs::pick_save(tr("Save panorama"), &default), "save"));
        }
    }

    fn hist_source(&self) -> Option<Img> {
        if self.view == ViewKind::Pano {
            if let Some(p) = &self.pano {
                return Some(p.img.clone());
            }
        }
        let it = self.items.get(self.sel)?;
        let g = 2f32.powf(self.p.ev as f32);
        Some(it.disp.map(move |c| [c[0] * g, c[1] * g, c[2] * g]))
    }

    fn auto_crop(&mut self) {
        let Some(p) = &self.pano else {
            self.status = tr("Preview the panorama first.").into();
            return;
        };
        let Some(cov) = &p.covered else {
            self.status = tr("Uncrop and preview first.").into();
            return;
        };
        let Some(((r0, r1), (c0, c1))) = largest_rectangle(cov) else {
            self.status = tr("Nothing to crop.").into();
            return;
        };
        // Output coordinates, one preview pixel inside on every side.
        let pu = |c: f64| (c - 0.5) / p.scale + p.box_.0 - 0.5;
        let pv = |r: f64| (r - 0.5) / p.scale + p.box_.2 - 0.5;
        let b = (pu(c0 as f64 + 2.0).ceil(), pu(c1 as f64).floor(), pv(r0 as f64 + 2.0).ceil(), pv(r1 as f64).floor());
        if b.1 <= b.0 || b.3 <= b.2 {
            self.status = tr("The covered area is too small to crop.").into();
            return;
        }
        let full = (p.box_.1 - p.box_.0 + 1.0) * (p.box_.3 - p.box_.2 + 1.0);
        let area = (b.1 - b.0 + 1.0) * (b.3 - b.2 + 1.0);
        self.crop = Some(b);
        self.vstate.fit = true;
        self.note_after = Some(crate::trf!("Cropped to {}×{} px ({} % of the panorama). The crop is used when saving; it is removed when you change the geometry.", b.1 - b.0 + 1.0, b.3 - b.2 + 1.0, (100.0 * area / full).round()));
        self.request_preview();
    }

    /// Open the control-point editor on the chosen pair (or the selected image and the next one).
    fn open_cp(&mut self) {
        let n = self.items.len();
        if n < 2 {
            self.status = tr("Add at least two images first.").into();
            return;
        }
        let (i, j) = self.pairs.get(self.pairsel).map(|q| (q.i, q.j)).unwrap_or((self.sel.min(n - 2), self.sel.min(n - 2) + 1));
        self.cp = Some(self.cp_for(i, j, true));
        self.view = ViewKind::Pair;
    }

    /// The editor on images `i` (left) and `j` (right), with the points placed by hand on them before.
    fn cp_for(&self, i: usize, j: usize, snap: bool) -> CpEdit {
        let q = self.pairs.iter().find(|q| q.manual && ((q.i, q.j) == (i, j) || (q.i, q.j) == (j, i)));
        let pts = q.map(|q| q.p1.iter().zip(q.p2.iter()).map(|(a, b)| if q.i == i { (*a, *b) } else { (*b, *a) }).collect()).unwrap_or_default();
        let stars = self.pairs.iter().find(|q| (q.i, q.j) == (i.min(j), i.max(j))).is_some_and(|q| q.stars);
        CpEdit { i, j, pts, pending: None, stars, snap, va: View::default(), vb: View::default() }
    }

    /// Focal length (px) of image `i`: from the placement, else from the pairs, else a guess.
    fn focal_guess(&self, i: usize) -> f64 {
        let shots: Vec<ShotInfo> = self.items.iter().map(|it| it.shot_info()).collect();
        let (grp, rho) = zoom(&shots);
        if let Some(l) = &self.layout {
            return l.f * rho[i] / rho[l.ref_];
        }
        let sizes: Vec<(usize, usize)> = self.items.iter().map(|it| (it.img.h, it.img.w)).collect();
        group_focal(&self.pairs, &sizes, &grp, &rho).map(|f| f * rho[i]).unwrap_or(sizes[i].0.max(sizes[i].1) as f64)
    }

    /// Fewest control points the matching model needs.
    fn cp_needed(&self) -> usize {
        match self.p.model {
            Model::Translation => 1,
            Model::Homography | Model::Similarity => 2,
            Model::Affine => 3,
        }
    }

    /// The pair's transform from the control points (pixels of the right image
    /// into the left one) and the mean error (px).
    fn cp_fit(&self, cp: &CpEdit) -> Option<(M3, f64)> {
        if cp.pts.len() < self.cp_needed() {
            return None;
        }
        let p1: Vec<Pt> = cp.pts.iter().map(|p| p.0).collect();
        let p2: Vec<Pt> = cp.pts.iter().map(|p| p.1).collect();
        let (a, b) = (&self.items[cp.i].img, &self.items[cp.j].img);
        let h = if self.p.model == Model::Homography {
            stars::rotation_from_points(&p1, &p2, (a.h, a.w), (b.h, b.w), self.focal_guess(cp.i), self.focal_guess(cp.j))?
        } else {
            fit_model(self.p.model, &p2, &p1).ok()?
        };
        let err = p1.iter().zip(&p2).map(|(x, y)| reproj_err(&h, *y, *x)).sum::<f64>() / p1.len() as f64;
        err.is_finite().then_some((h, err))
    }

    /// Use the control points: the pair placed by hand replaces any other result for these two images.
    fn cp_apply(&mut self) {
        let Some(cp) = &self.cp else { return };
        let Some((h, err)) = self.cp_fit(cp) else {
            self.status = crate::trf!("Place at least {} points in both images.", self.cp_needed());
            return;
        };
        let (i, j, pts, h) = if cp.i < cp.j { (cp.i, cp.j, cp.pts.clone(), h) } else { (cp.j, cp.i, cp.pts.iter().map(|p| (p.1, p.0)).collect(), inv(&h)) };
        let n = pts.len();
        let msg = crate::trf!("Pair {}–{} (control points): {} points placed by hand, mean error {} px", i + 1, j + 1, n, format!("{err:.2}"));
        let pr = PairRes { i, j, p1: Arc::new(pts.iter().map(|p| p.0).collect()), p2: Arc::new(pts.iter().map(|p| p.1).collect()), h, inl: Arc::new((0..n).collect()), ok: true, listed: true, msg, stars: cp.stars, manual: true };
        match self.pairs.iter().position(|q| (q.i, q.j) == (i, j)) {
            Some(k) => self.pairs[k] = pr,
            None => {
                self.pairs.push(pr);
                self.pairs.sort_by_key(|q| (q.i, q.j));
            }
        }
        self.pairsel = self.pairs.iter().position(|q| (q.i, q.j) == (i, j)).unwrap_or(0);
        let note = self.relayout_now();
        self.crop = None;
        self.status = crate::trf!("Control points used for pair {}–{}; placement updated.{}", i + 1, j + 1, note);
        if self.layout.is_some() {
            // Shown again once the preview is ready (it reports in the status line too).
            self.note_after = Some(self.status.clone());
            self.request_preview();
        }
    }

    /// The control-point editor: the two images side by side, each with its own zoom.
    fn show_cp(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let n = self.items.len();
        let Some(mut cp) = self.cp.take() else { return };
        if cp.i >= n || cp.j >= n || n < 2 {
            return;
        }
        let mut close = false;
        let mut apply = false;
        ui.horizontal_wrapped(|ui| {
            let (mut i, mut j) = (cp.i, cp.j);
            let num = |v: f64, _: std::ops::RangeInclusive<usize>| format!("{}", v as usize + 1);
            let parse = |s: &str| s.trim().parse::<f64>().ok().map(|v| v - 1.0);
            ui.label(tr("Left"));
            ui.add(egui::DragValue::new(&mut i).range(0..=n - 1).custom_formatter(num).custom_parser(parse));
            ui.label(tr("Right"));
            ui.add(egui::DragValue::new(&mut j).range(0..=n - 1).custom_formatter(num).custom_parser(parse));
            if (i, j) != (cp.i, cp.j) && i != j {
                cp = self.cp_for(i, j, cp.snap);
            }
            ui.checkbox(&mut cp.snap, tr("snap to stars")).on_hover_text(tr("A click lands on the centre of the nearest star; elsewhere (the ground) it stays where you clicked"));
            ui.checkbox(&mut cp.stars, tr("the points are stars")).on_hover_text(tr("The sky turns between shots minutes apart: star points let the refinement measure it. Leave off for points on the ground"));
            let need = self.cp_needed();
            if ui.add_enabled(cp.pts.len() >= need, egui::Button::new(egui::RichText::new(tr("✔ Use the points")).strong())).on_disabled_hover_text(crate::trf!("Place at least {} points in both images.", need)).clicked() {
                apply = true;
            }
            if ui.add_enabled(!cp.pts.is_empty(), egui::Button::new(tr("Clear"))).clicked() {
                cp.pts.clear();
                cp.pending = None;
            }
            if ui.button(tr("Close")).clicked() {
                close = true;
            }
        });
        let fit = self.cp_fit(&cp);
        ui.label(match &fit {
            Some((_, e)) => crate::trf!("{} points · mean error {} px. Right-click a point to remove it.", cp.pts.len(), format!("{e:.2}")),
            None => crate::trf!("Click a point in one image, then the same point in the other (at least {}; spread them over the overlap). Scroll to zoom, drag to pan.", self.cp_needed()),
        });
        let (ta, tb) = (self.tex_for(ctx, cp.i), self.tex_for(ctx, cp.j));
        let sizes = [(self.items[cp.i].img.w as f64, self.items[cp.i].img.h as f64), (self.items[cp.j].img.w as f64, self.items[cp.j].img.h as f64)];
        let palette = [Color32::from_rgb(255, 80, 80), Color32::from_rgb(80, 220, 80), Color32::from_rgb(90, 160, 255), Color32::from_rgb(255, 200, 40), Color32::from_rgb(230, 90, 230), Color32::from_rgb(60, 220, 220)];
        let mut clicked: Option<(bool, Pt, f32)> = None;
        let mut removed: Option<usize> = None;
        let avail = ui.available_size();
        let (pts, pending) = (cp.pts.clone(), cp.pending);
        ui.horizontal(|ui| {
            for right in [false, true] {
                let (w, h) = sizes[right as usize];
                let tex = if right { &tb } else { &ta };
                let view = if right { &mut cp.vb } else { &mut cp.va };
                let label = if right { cp.j + 1 } else { cp.i + 1 };
                ui.allocate_ui(egui::vec2((avail.x - 8.0) / 2.0, avail.y), |ui| {
                    let s = image_view(ui, view, (w, h), &[(tex, [0.5, w + 0.5, 0.5, h + 0.5])], true);
                    let p = ui.painter().with_clip_rect(s.rect);
                    p.text(s.rect.left_top() + egui::vec2(6.0, 4.0), egui::Align2::LEFT_TOP, label.to_string(), egui::FontId::proportional(20.0), Color32::WHITE);
                    for (k, pt) in pts.iter().enumerate() {
                        let q = if right { pt.1 } else { pt.0 };
                        let c = palette[k % palette.len()];
                        let at = s.to_screen(q[0], q[1]);
                        p.circle_stroke(at, 7.0, Stroke::new(2.0, c));
                        p.text(at + egui::vec2(9.0, -9.0), egui::Align2::LEFT_BOTTOM, (k + 1).to_string(), egui::FontId::proportional(15.0), c);
                        // Where the fit puts this point: a long line means a wrong point.
                        if let Some((hh, _)) = &fit {
                            let (x, y) = if right { apply_h(&inv(hh), pt.0[0], pt.0[1]) } else { apply_h(hh, pt.1[0], pt.1[1]) };
                            p.line_segment([at, s.to_screen(x, y)], Stroke::new(1.0, c));
                        }
                    }
                    if let Some((on_right, q)) = pending {
                        if on_right == right {
                            p.circle_stroke(s.to_screen(q[0], q[1]), 9.0, Stroke::new(2.5, Color32::YELLOW));
                        } else if let Some((hh, _)) = &fit {
                            // The partner's likely place in this image.
                            let (x, y) = if right { apply_h(&inv(hh), q[0], q[1]) } else { apply_h(hh, q[0], q[1]) };
                            let at = s.to_screen(x, y);
                            let st = Stroke::new(1.5, Color32::YELLOW);
                            p.line_segment([at - egui::vec2(12.0, 0.0), at + egui::vec2(12.0, 0.0)], st);
                            p.line_segment([at - egui::vec2(0.0, 12.0), at + egui::vec2(0.0, 12.0)], st);
                        }
                    }
                    if let Some(pos) = s.resp.interact_pointer_pos() {
                        if s.resp.clicked() {
                            let (x, y) = s.to_image(pos);
                            if x >= 0.5 && x <= w + 0.5 && y >= 0.5 && y <= h + 0.5 {
                                clicked = Some((right, [x, y], s.zoom));
                            }
                        } else if s.resp.secondary_clicked() {
                            removed = pts
                                .iter()
                                .enumerate()
                                .map(|(k, pt)| {
                                    let q = if right { pt.1 } else { pt.0 };
                                    (k, s.to_screen(q[0], q[1]).distance(pos))
                                })
                                .filter(|(_, d)| *d < 14.0)
                                .min_by(|a, b| a.1.total_cmp(&b.1))
                                .map(|(k, _)| k);
                        }
                    }
                });
            }
        });
        if let Some(k) = removed {
            cp.pts.remove(k);
        }
        if let Some((right, q, zoom)) = clicked {
            let img = &self.items[if right { cp.j } else { cp.i }].img;
            let q = if cp.snap { stars::snap_to_star(img, q[0], q[1], (8.0 / zoom as f64).clamp(3.0, 30.0)).unwrap_or(q) } else { q };
            cp.pending = match cp.pending {
                Some((other, o)) if other != right => {
                    cp.pts.push(if right { (o, q) } else { (q, o) });
                    None
                }
                _ => Some((right, q)),
            };
        }
        if !close {
            self.cp = Some(cp);
        }
        if apply {
            self.cp_apply();
        }
    }

    /// An image as the views show it: with the stretch panel, or with the
    /// display mode (measured on `reference`, the whole preview, when given).
    fn shown_img(&self, a: &Img, gain: f64, reference: Option<&Img>) -> Img {
        if self.disp_processed {
            return look(a, gain, &self.p);
        }
        let g = gain as f32;
        let b = if g == 1.0 { a.clone() } else { a.map(move |c| [c[0] * g, c[1] * g, c[2] * g]) };
        crate::finish::display_view(&b, self.disp_view, reference.unwrap_or(&b))
    }

    fn disp_key(&self) -> u64 {
        if self.disp_processed { 0 } else { self.disp_view.key().rotate_left(5) | 2 }
    }

    /// The source masks of the images `vis` as soft rasters for rendering (None when there are none).
    fn force_job(&self, vis: &[usize]) -> Option<Arc<Vec<Option<Arc<crate::image::Gray>>>>> {
        let v: Vec<Option<Arc<crate::image::Gray>>> = vis.iter().map(|&i| mask_raster(&self.items[i].mask, self.items[i].img.h, self.items[i].img.w).map(Arc::new)).collect();
        v.iter().any(|m| m.is_some()).then(|| Arc::new(v))
    }

    /// Start or stop painting source masks on the panorama.
    /// Why the horizon cannot be levelled now, if it cannot.
    fn horizon_blocked(&self) -> Option<&'static str> {
        if !horizon::supported(self.p.proj) {
            Some(tr("Not for this projection: it bends every level line. For rectilinear, cylindrical, spherical, Mercator, Hammer and Mollweide."))
        } else if self.pano.is_none() || self.layout.is_none() {
            Some(tr("Preview the panorama first."))
        } else {
            None
        }
    }

    /// Find the horizon in the preview and level it. True when pitch / roll changed.
    fn auto_horizon(&mut self) -> bool {
        if let Some(why) = self.horizon_blocked() {
            self.status = why.into();
            return false;
        }
        let pv = self.pano.as_ref().unwrap();
        let pts = horizon::find_horizon(&pv.img, pv.covered.as_ref());
        let fitted = horizon::fit_curve(&pts, 4);
        let kept: Vec<Pt> = match &fitted {
            Some((_, _, w)) => pts.iter().zip(w).filter(|(_, &w)| w > 0.5).map(|(p, _)| *p).collect(),
            None => vec![],
        };
        let span = kept.iter().map(|p| p[0]).fold(f64::MIN, f64::max) - kept.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        if kept.len() < 8 || span < 0.15 * pv.img.w as f64 {
            self.status = tr("No horizon found in the preview (it needs sky above ground across a good part of the width). Mark it yourself: ✎ Horizon by hand.").into();
            return false;
        }
        let (curve, _, _) = fitted.unwrap();
        let (x0, x1) = (kept.iter().map(|p| p[0]).fold(f64::MAX, f64::min), kept.iter().map(|p| p[0]).fold(f64::MIN, f64::max));
        let to_uv = |q: Pt| [(q[0] + 0.5) / pv.scale + pv.box_.0 - 0.5, (q[1] + 0.5) / pv.scale + pv.box_.2 - 0.5];
        let line: Vec<Pt> = (0..=160).map(|k| x0 + (x1 - x0) * k as f64 / 160.0).map(|x| to_uv([x, curve.at(x)])).collect();
        let uv: Vec<Pt> = kept.iter().map(|&q| to_uv(q)).collect();
        let share = span / pv.img.w as f64 * 100.0;
        self.level_horizon(&uv, &line, &uv, crate::trf!(" Found across {} % of the width.", format!("{share:.0}")), Some(AUTO_HORIZON_MAX))
    }

    /// Set pitch and roll so that the horizon through `level_pts` (output
    /// coordinates of the preview) is straight; `line` and `marks` are shown.
    /// `limit`: largest change of pitch or roll accepted (°), None for any.
    fn level_horizon(&mut self, level_pts: &[Pt], line: &[Pt], marks: &[Pt], note: String, limit: Option<f64>) -> bool {
        let Some(pv) = &self.pano else { return false };
        let (proj, f, [y, p, r]) = pv.view;
        let dirs = horizon::directions(level_pts, proj, f, y, p, r);
        let Some((np, nr, before, after)) = horizon::level(&dirs, self.p.proj, self.p.yaw, self.p.pitch, self.p.roll) else {
            self.status = tr("Too few points on the horizon to level it.").into();
            return false;
        };
        let span = horizon::azimuth_span(&dirs, self.p.yaw, np, nr);
        if limit.is_some_and(|lim| span < AUTO_HORIZON_NARROW && (nr - self.p.roll).abs() > lim) {
            // The edge of the sky is the skyline, not the horizon: in a narrow
            // panorama a hillside would be "levelled" into a tilted picture.
            self.status = crate::trf!("The edge of the sky found would need roll {}° in a panorama only {}° wide: it is probably a slope of the landscape, not a tilt, so nothing was changed. Mark the true horizon yourself: ✎ Horizon by hand.", format!("{nr:.1}"), format!("{span:.0}"));
            return false;
        }
        // Not straighter after all (pitch and roll can be large: a reference photo
        // pointing near the zenith needs a pitch of 60–80°).
        if !(after < 0.7 * before || after < 0.5) || np.abs() > 89.5 {
            self.status = crate::trf!("The horizon found would need pitch {}° and roll {}°: that does not look right, so nothing was changed. Mark it yourself: ✎ Horizon by hand.", format!("{np:.1}"), format!("{nr:.1}"));
            return false;
        }
        self.horizon = Some(HorizonLine { curve: horizon::directions(line, proj, f, y, p, r), points: horizon::directions(marks, proj, f, y, p, r), ref_: self.layout.as_ref().map(|l| l.ref_).unwrap_or(0) });
        let (p0, r0) = (self.p.pitch, self.p.roll);
        (self.p.pitch, self.p.roll) = ((np * 100.0).round() / 100.0, (nr * 100.0).round() / 100.0);
        self.crop = None;
        self.status = crate::trf!("Horizon levelled: pitch {}° → {}°, roll {}° → {}°; its height varies {}° → {}°.{}", format!("{p0:.2}"), format!("{:.2}", self.p.pitch), format!("{r0:.2}"), format!("{:.2}", self.p.roll), format!("{before:.2}"), format!("{after:.2}"), note);
        // Kept when the new preview arrives.
        self.note_after = Some(self.status.clone());
        true
    }

    fn toggle_horizon_edit(&mut self) {
        if self.horizon_edit.take().is_some() {
            return;
        }
        if let Some(why) = self.horizon_blocked() {
            self.status = why.into();
            return;
        }
        if self.crop_edit.is_some() {
            self.toggle_crop();
        }
        self.mask_edit = None;
        self.horizon_edit = Some(vec![]);
        self.view = ViewKind::Pano;
        self.status = tr("Click 4 or more points along the horizon, spread over its length (right click removes the nearest). Then ✔ Level horizon (Enter); Esc cancels.").into();
    }

    /// The row under the view's buttons while the horizon is marked by hand.
    fn horizon_tools(&mut self, ui: &mut egui::Ui) {
        let n = self.horizon_edit.as_ref().map(|v| v.len()).unwrap_or(0);
        let mut go = false;
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(Color32::from_rgb(120, 230, 120), crate::trf!("Horizon by hand: {} point(s)", n));
            go = ui.add_enabled(n >= 2, egui::Button::new(tr("✔ Level horizon"))).on_hover_text(tr("Enter")).on_disabled_hover_text(tr("Click at least 2 points (4 or more is better)")).clicked();
            if ui.add_enabled(n > 0, egui::Button::new(tr("Clear points"))).clicked() {
                self.horizon_edit = Some(vec![]);
            }
            if ui.button(tr("Cancel")).on_hover_text(tr("Esc")).clicked() {
                self.horizon_edit = None;
            }
        });
        if !ui.ctx().egui_wants_keyboard_input() && n >= 2 && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            go = true;
        }
        if go {
            self.level_by_hand();
        }
    }

    /// Level the horizon through the points marked by hand.
    fn level_by_hand(&mut self) {
        let Some(mut pts) = self.horizon_edit.clone() else { return };
        if pts.len() < 2 {
            return;
        }
        pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let Some(curve) = horizon::curve_through(&pts, 4) else { return };
        let (u0, u1) = (pts[0][0], pts[pts.len() - 1][0]);
        let line: Vec<Pt> = (0..=160).map(|k| u0 + (u1 - u0) * k as f64 / 160.0).map(|u| [u, curve.at(u)]).collect();
        let n = pts.len();
        if self.level_horizon(&line, &line, &pts, crate::trf!(" Through {} point(s) marked by hand.", n), None) {
            self.horizon_edit = None;
            self.request_preview();
        }
    }

    /// Clicks on the panorama while the horizon is marked by hand.
    fn horizon_input(&mut self, ui: &egui::Ui, s: &super::Shown) {
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.horizon_edit = None;
            self.status = tr("Horizon by hand cancelled.").into();
            return;
        }
        if let Some(pos) = s.resp.hover_pos() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            let _ = pos;
        }
        let Some(pts) = self.horizon_edit.as_mut() else { return };
        if s.resp.clicked() {
            if let Some(pos) = s.resp.interact_pointer_pos() {
                let (u, v) = s.to_image(pos);
                pts.push([u, v]);
            }
        }
        if s.resp.secondary_clicked() {
            if let Some(pos) = s.resp.interact_pointer_pos() {
                let (u, v) = s.to_image(pos);
                if let Some(k) = (0..pts.len()).min_by(|&a, &b| (pts[a][0] - u).hypot(pts[a][1] - v).total_cmp(&(pts[b][0] - u).hypot(pts[b][1] - v))) {
                    pts.remove(k);
                }
            }
        }
    }

    /// The horizon: the curve through the points being marked, or the one found
    /// (turned with the current view, so a levelled horizon shows straight).
    fn draw_horizon(&self, p: &egui::Painter, s: &super::Shown) {
        let green = Color32::from_rgb(90, 230, 90);
        let poly = |pts: &[Pt], width: f32| {
            // Break the line where it jumps (across the ±180° seam): a step far
            // longer than the typical one.
            let steps: Vec<f64> = pts.windows(2).map(|w| (w[1][0] - w[0][0]).abs()).collect();
            let typical = if steps.is_empty() { 0.0 } else { crate::image::median_f64(steps.iter().copied()) };
            let mut seg: Vec<Pos2> = Vec::new();
            for w in pts.windows(2) {
                if (w[1][0] - w[0][0]).abs() > 10.0 * typical + 1.0 {
                    if seg.len() > 1 {
                        p.line(std::mem::take(&mut seg), Stroke::new(width, green));
                    }
                    seg.clear();
                    continue;
                }
                if seg.is_empty() {
                    seg.push(s.to_screen(w[0][0], w[0][1]));
                }
                seg.push(s.to_screen(w[1][0], w[1][1]));
            }
            if seg.len() > 1 {
                p.line(seg, Stroke::new(width, green));
            }
        };
        if let Some(pts) = &self.horizon_edit {
            let mut sorted = pts.clone();
            sorted.sort_by(|a, b| a[0].total_cmp(&b[0]));
            if sorted.len() >= 2 {
                if let Some(c) = horizon::curve_through(&sorted, 4) {
                    let (u0, u1) = (sorted[0][0], sorted[sorted.len() - 1][0]);
                    poly(&(0..=160).map(|k| u0 + (u1 - u0) * k as f64 / 160.0).map(|u| [u, c.at(u)]).collect::<Vec<_>>(), 2.5);
                }
            }
            for q in pts {
                p.circle_stroke(s.to_screen(q[0], q[1]), 6.0, Stroke::new(2.0, green));
            }
            return;
        }
        let (Some(h), Some(l)) = (&self.horizon, &self.layout) else { return };
        if h.ref_ != l.ref_ || !horizon::supported(self.p.proj) {
            return;
        }
        let f = l.f * self.p.focal;
        let out = |d: &nalgebra::Vector3<f64>| horizon::to_output(d, self.p.proj, f, self.p.yaw, self.p.pitch, self.p.roll);
        poly(&h.curve.iter().filter_map(out).collect::<Vec<_>>(), 2.0);
        for q in h.points.iter().filter_map(out) {
            p.circle_filled(s.to_screen(q[0], q[1]), 2.0, green);
        }
    }

    /// The sky's rotation into the shown panorama (see `sky_grid::fit`), its
    /// spread (degrees) and the number of photos it comes from.
    fn sky_mapping(&self) -> Option<(M3, f64, usize)> {
        let (f, c, g, vis, lens) = self.geometry()?;
        // Where the panorama shows each photo's stars (turned to one moment, if it does).
        let turned = self.sky_job(&vis, &g);
        let cat = solve::catalog();
        let mut pairs = Vec::new();
        let mut n = 0;
        for (k, &i) in vis.iter().enumerate() {
            let Some(sol) = self.items[i].solved.as_ref().and_then(|s| s.found()) else { continue };
            let gi = turned.as_ref().and_then(|j| j.g[k]).unwrap_or(g[i]);
            let ci = center(&self.items[i].img);
            n += 1;
            for &(idx, px) in &sol.stars {
                let (ux, uy) = undistort(px[0], px[1], ci, lens[i]);
                let q = gi * nalgebra::Vector3::new(ux, uy, 1.0);
                let d = nalgebra::Vector3::new((q[0] - c[0] * q[2]) / f, (q[1] - c[1] * q[2]) / f, q[2]);
                if let Some(sky) = cat.dir.get(idx as usize) {
                    pairs.push((*sky, d));
                }
            }
        }
        let (r, spread) = sky_grid::fit(&pairs)?;
        Some((r, spread, n))
    }

    /// The RA / Dec grid with numbers and the names of bright objects, over the panorama.
    fn draw_sky_grid(&self, p: &egui::Painter, s: &super::Shown) {
        let (Some((r, spread, n)), Some(l), Some(pv)) = (self.sky_mapping(), &self.layout, &self.pano) else { return };
        let (proj, f) = (self.p.proj, l.f * self.p.focal);
        // Only over the panorama itself; names only where a photo shows the sky.
        let b = pv.box_;
        let (u1, v1) = (pv.img.w as f64 / pv.scale + b.0 - 0.5, pv.img.h as f64 / pv.scale + b.2 - 0.5);
        let pano_rect = egui::Rect::from_min_max(s.to_screen(b.0 - 0.5, b.2 - 0.5), s.to_screen(u1, v1));
        let p = &p.with_clip_rect(s.rect.intersect(pano_rect));
        let shown = |uv: Pt| -> bool {
            let (x, y) = (((uv[0] - b.0 + 0.5) * pv.scale).floor(), ((uv[1] - b.2 + 0.5) * pv.scale).floor());
            if x < 0.0 || y < 0.0 || x >= pv.img.w as f64 || y >= pv.img.h as f64 {
                return false;
            }
            match &pv.covered {
                Some(c) if c.w == pv.img.w && c.h == pv.img.h => c.v[y as usize * c.w + x as usize],
                _ => true,
            }
        };
        let to_uv = |d: &nalgebra::Vector3<f64>| -> Option<Pt> {
            let q = r * d;
            let (a, b) = proj_forward(proj, q[0], q[1], q[2]);
            (a.is_finite() && b.is_finite()).then_some([a * f, b * f])
        };
        let rect = s.rect.intersect(pano_rect);
        let px_per_deg = s.zoom as f64 * f * std::f64::consts::PI / 180.0;
        // The part of the sky in view: RA / Dec of a grid of points over it.
        let (x0, x1, y0, y1) = s.visible();
        let (mut dmin, mut dmax, mut ras) = (90.0f64, -90.0f64, Vec::new());
        for a in 0..=16 {
            for b in 0..=16 {
                let (u, v) = (x0 + (x1 - x0) * a as f64 / 16.0, y0 + (y1 - y0) * b as f64 / 16.0);
                let Some((x, y, z)) = proj_inverse(proj, u / f, v / f) else { continue };
                let (ra, dec) = solve::radec(&(r.transpose() * nalgebra::Vector3::new(x, y, z).normalize()));
                dmin = dmin.min(dec);
                dmax = dmax.max(dec);
                ras.push(ra);
            }
        }
        if ras.is_empty() {
            return;
        }
        ras.sort_by(f64::total_cmp);
        // RA range: the circle minus its largest gap (all of it around a pole).
        let mut gap = (ras[0] + 360.0 - ras[ras.len() - 1], ras[ras.len() - 1]);
        for w in ras.windows(2) {
            if w[1] - w[0] > gap.0 {
                gap = (w[1] - w[0], w[0]);
            }
        }
        let pole = dmax > 80.0 || dmin < -80.0;
        let (ra0, ra_span) = if pole || gap.0 < 30.0 { (0.0, 360.0) } else { (gap.1 + gap.0, 360.0 - gap.0) };
        let (dmin, dmax) = ((dmin - 1.0).max(-89.9), (dmax + 1.0).min(89.9));
        let dstep = sky_grid::grid_step(90.0 / px_per_deg);
        let rstep = sky_grid::ra_step(90.0 / px_per_deg / dmin.abs().max(dmax.abs()).to_radians().cos().max(0.1));
        let grid = Color32::from_rgba_unmultiplied(120, 190, 255, 150);
        let text = Color32::from_rgb(150, 205, 255);
        let font = egui::FontId::proportional(13.0);
        let seam = 0.5 * std::f64::consts::PI * f;
        // A line through sky points: drawn in pieces (broken at the seam or where not shown);
        // returns its point in view nearest the left or top edge, for the number.
        let line = |pts: Vec<nalgebra::Vector3<f64>>, left: bool| -> Option<Pos2> {
            let mut best: Option<Pos2> = None;
            let mut seg: Vec<Pos2> = Vec::new();
            let mut prev: Option<Pt> = None;
            for d in &pts {
                let q = to_uv(d);
                if let (Some(a), Some(b)) = (prev, q) {
                    if (a[0] - b[0]).abs() > seam || (a[1] - b[1]).abs() > seam {
                        if seg.len() > 1 {
                            p.line(std::mem::take(&mut seg), Stroke::new(1.0, grid));
                        }
                        seg.clear();
                    }
                }
                match q {
                    Some(uv) => {
                        let sp = s.to_screen(uv[0], uv[1]);
                        if rect.shrink(4.0).contains(sp) && best.is_none_or(|b| if left { sp.x < b.x } else { sp.y < b.y }) {
                            best = Some(sp);
                        }
                        seg.push(sp);
                    }
                    None => {
                        if seg.len() > 1 {
                            p.line(std::mem::take(&mut seg), Stroke::new(1.0, grid));
                        }
                        seg.clear();
                    }
                }
                prev = q;
            }
            if seg.len() > 1 {
                p.line(seg, Stroke::new(1.0, grid));
            }
            best
        };
        // Declination lines (with their numbers at the left) …
        let mut dec = (dmin / dstep).ceil() * dstep;
        while dec <= dmax {
            let pts = (0..=240).map(|k| solve::direction(ra0 + ra_span * k as f64 / 240.0, dec)).collect();
            if let Some(at) = line(pts, true) {
                p.text(at + egui::vec2(3.0, -2.0), egui::Align2::LEFT_BOTTOM, sky_grid::dec_text(dec), font.clone(), text);
            }
            dec += dstep;
        }
        // … and right ascension lines (numbers at the top).
        let first = (ra0 / rstep).ceil() * rstep;
        let mut ra = first;
        while ra <= ra0 + ra_span + 1e-9 && ra < first + 360.0 - 1e-9 {
            let pts = (0..=180).map(|k| solve::direction(ra, dmin + (dmax - dmin) * k as f64 / 180.0)).collect();
            if let Some(at) = line(pts, false) {
                p.text(at + egui::vec2(3.0, 2.0), egui::Align2::LEFT_TOP, sky_grid::ra_text(ra), font.clone(), text);
            }
            ra += rstep;
        }
        // Names: more of them as the view zooms in.
        let star_mag = if px_per_deg < 4.0 { 1.5 } else if px_per_deg < 12.0 { 2.5 } else { 3.5 };
        let dso_mag = if px_per_deg < 4.0 { 4.5 } else if px_per_deg < 12.0 { 6.0 } else { 99.0 };
        for lb in sky_grid::labels() {
            if lb.mag > if lb.star { star_mag } else { dso_mag } {
                continue;
            }
            let Some(uv) = to_uv(&lb.dir) else { continue };
            let sp = s.to_screen(uv[0], uv[1]);
            if !rect.contains(sp) || !shown(uv) {
                continue;
            }
            let col = if lb.star { Color32::from_rgb(255, 215, 90) } else { Color32::from_rgb(130, 235, 150) };
            let rad = (lb.size / 2.0 * px_per_deg).max(if lb.star { 9.0 } else { 11.0 }) as f32;
            p.circle_stroke(sp, rad, Stroke::new(1.3, col));
            p.text(sp + egui::vec2(rad * 0.75 + 3.0, -rad * 0.75), egui::Align2::LEFT_BOTTOM, &lb.name, font.clone(), col);
        }
        p.with_clip_rect(s.rect).text(s.rect.left_bottom() + egui::vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, crate::trf!("Sky grid from {} photo(s) found on the star catalogue, within about {}°", n, format!("{:.1}", spread.max(0.05))), font, text);
    }

    fn toggle_masks(&mut self) {
        if self.mask_edit.take().is_some() {
            self.mask_tex = None;
            return;
        }
        if self.pano.is_none() {
            self.status = tr("Preview the panorama first.").into();
            return;
        }
        if self.crop_edit.is_some() {
            self.toggle_crop();
        }
        let vis = self.visible();
        let src = if vis.contains(&self.sel) { self.sel } else { vis.first().copied().unwrap_or(0) };
        self.mask_edit = Some(MaskEdit { src, brush: 40.0, stroke: vec![], erase: false });
        self.view = ViewKind::Pano;
        self.status = tr("Source masks: choose the image with Shift+click on the panorama (again for the next image there), then paint where it must be used. Right drag erases.").into();
    }

    /// The tools of source-mask painting (a row under the view's buttons).
    fn mask_tools(&mut self, ui: &mut egui::Ui) {
        let n = self.items.len();
        let Some(me) = self.mask_edit.as_mut() else { return };
        let mut clear: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Source image"));
            ui.add(egui::DragValue::new(&mut me.src).range(0..=n.saturating_sub(1)).custom_formatter(|v, _| format!("{}", v as usize + 1)).custom_parser(|s| s.trim().parse::<f64>().ok().map(|v| v - 1.0)))
                .on_hover_text(tr("The image to use where you paint. Shift+click on the panorama picks the image under the cursor; Shift+click again picks the next one there"));
            ui.spacing_mut().slider_width = 110.0;
            ui.add(egui::Slider::new(&mut me.brush, 4.0..=250.0).logarithmic(true).text(tr("Brush")).suffix(" px"));
            ui.weak(tr("left drag paints, right drag erases"));
            if ui.button(tr("Clear this image's mask")).clicked() {
                clear = Some(me.src);
            }
        });
        let masked: Vec<usize> = (0..n).filter(|&i| !self.items[i].mask.is_empty()).collect();
        if !masked.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::trf!("Masks ({}):", masked.len()));
                for i in masked {
                    let me = self.mask_edit.as_mut().unwrap();
                    if ui.selectable_label(me.src == i, (i + 1).to_string()).on_hover_text(tr("Paint on this image's mask")).clicked() {
                        me.src = i;
                    }
                    if ui.small_button("✕").on_hover_text(crate::trf!("Remove the mask of image {}", i + 1)).clicked() {
                        clear = Some(i);
                    }
                }
            });
        }
        if let Some(i) = clear {
            if !self.items[i].mask.is_empty() {
                self.items[i].mask = Arc::new(vec![]);
                self.mask_tex = None;
                self.request_preview();
            }
        }
    }

    /// Painting source masks on the shown panorama: Shift+click picks the image,
    /// drags paint (left) or erase (right) its mask, which is shown tinted.
    fn mask_input(&mut self, ui: &egui::Ui, s: &super::Shown) {
        let Some((f, c, g, vis, lens)) = self.geometry() else { return };
        let Some(pv) = self.pano.as_ref() else { return };
        let (box_, scale, pw, ph) = (pv.box_, pv.scale, pv.img.w, pv.img.h);
        let palette = [Color32::from_rgb(255, 60, 60), Color32::from_rgb(60, 220, 60), Color32::from_rgb(70, 140, 255), Color32::from_rgb(255, 200, 30), Color32::from_rgb(230, 70, 230), Color32::from_rgb(40, 220, 220), Color32::from_rgb(255, 130, 40)];
        // The masks, tinted in each image's colour and numbered (recomputed when they or the preview change).
        let mfp = hash_of(&format!("{:?}{}{box_:?}{scale}", self.items.iter().map(|it| (it.mask.len(), it.mask.iter().map(|d| d.x + d.y + d.r).sum::<f64>())).collect::<Vec<_>>(), self.pano_version));
        if self.mask_tex.as_ref().map(|m| m.0) != Some(mfp) {
            let force = self.force_job(&vis);
            let gv: Vec<M3> = vis.iter().map(|&i| g[i]).collect();
            let sizes: Vec<(usize, usize)> = vis.iter().map(|&i| (self.items[i].img.h, self.items[i].img.w)).collect();
            let cp = ComposeParams { scale, projection: self.p.proj, f, c, lenses: vis.iter().map(|&i| lens[i]).collect(), force, ..Default::default() };
            let mut labels = Vec::new();
            let tex = source_map(&gv, &sizes, box_, &cp).filter(|m| m.0 == pw && m.1 == ph).map(|(w, h, fi, fm)| {
                let mut rgba = vec![0u8; w * h * 4];
                let mut sums: HashMap<i32, (f64, f64, f64)> = HashMap::new();
                for k in 0..w * h {
                    if fi[k] > 0 && fm[k] > 0.02 {
                        let i = vis[(fi[k] - 1) as usize];
                        let col = palette[i % palette.len()];
                        rgba[4 * k..4 * k + 4].copy_from_slice(&[col.r(), col.g(), col.b(), (fm[k] * 110.0) as u8]);
                        if fm[k] > 0.5 {
                            let e = sums.entry(fi[k]).or_insert((0.0, 0.0, 0.0));
                            *e = (e.0 + (k % w) as f64, e.1 + (k / w) as f64, e.2 + 1.0);
                        }
                    }
                }
                for (k, (sx, sy, sn)) in sums {
                    labels.push((vis[(k - 1) as usize], (sx / sn + 0.5) / scale + box_.0 - 0.5, (sy / sn + 0.5) / scale + box_.2 - 0.5));
                }
                ui.ctx().load_texture("source_masks", egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba), egui::TextureOptions::LINEAR)
            });
            self.mask_tex = Some((mfp, tex, labels));
        }
        let p = ui.painter().with_clip_rect(s.rect);
        let (u0, u1, v0, v1) = (box_.0 - 0.5, pw as f64 / scale + box_.0 - 0.5, box_.2 - 0.5, ph as f64 / scale + box_.2 - 0.5);
        if let Some((_, Some(t), labels)) = &self.mask_tex {
            p.image(t.id(), egui::Rect::from_min_max(s.to_screen(u0, v0), s.to_screen(u1, v1)), egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            for &(i, u, v) in labels {
                p.text(s.to_screen(u, v), egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::proportional(20.0), palette[i % palette.len()]);
            }
        }
        let Some(me) = self.mask_edit.as_mut() else { return };
        let src = me.src.min(self.items.len().saturating_sub(1));
        // The chosen image's outline.
        if let Some(o) = self.pano.as_ref().and_then(|pv| pv.outlines.get(src)) {
            let mut pts: Vec<Pos2> = o.iter().filter(|q| q[0].is_finite()).map(|q| s.to_screen(q[0], q[1])).collect();
            if let Some(&f0) = pts.first() {
                pts.push(f0);
            }
            p.line(pts, Stroke::new(3.0, palette[src % palette.len()]));
        }
        let shift = ui.input(|i| i.modifiers.shift);
        let size = |i: usize| (self.items[i].img.h, self.items[i].img.w);
        let to_px = |i: usize, u: f64, v: f64| uv_to_pixel(&g[i], size(i), self.p.proj, f, c, lens[i], u, v);
        if let Some(pos) = s.resp.hover_pos() {
            let col = if me.erase && !me.stroke.is_empty() { Color32::from_rgb(255, 90, 90) } else { Color32::WHITE };
            p.circle_stroke(pos, me.brush, Stroke::new(1.5, col));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        // Shift+click: the image under the cursor (the next one on each click).
        if shift && s.resp.clicked() {
            if let Some(pos) = s.resp.interact_pointer_pos() {
                let (u, v) = s.to_image(pos);
                let here: Vec<usize> = vis.iter().copied().filter(|&i| to_px(i, u, v).is_some()).collect();
                if let Some(k) = here.iter().position(|&i| i == me.src).map(|k| (k + 1) % here.len()).or((!here.is_empty()).then_some(0)) {
                    me.src = here[k];
                    self.status = crate::trf!("Source image {} (images here: {}).", here[k] + 1, here.iter().map(|i| (i + 1).to_string()).collect::<Vec<_>>().join(", "));
                }
            }
            return;
        }
        let pointer = s.resp.interact_pointer_pos();
        let started = s.resp.drag_started_by(egui::PointerButton::Primary) || s.resp.drag_started_by(egui::PointerButton::Secondary);
        let single = (s.resp.clicked() || s.resp.secondary_clicked()) && !shift;
        if started || single {
            me.stroke.clear();
            me.erase = s.resp.drag_started_by(egui::PointerButton::Secondary) || s.resp.secondary_clicked();
        }
        if let Some(pos) = pointer {
            if s.resp.dragged() || started || single {
                let q = s.to_image(pos);
                if me.stroke.last() != Some(&q) {
                    me.stroke.push(q);
                }
            }
        }
        for &(u, v) in &me.stroke {
            p.circle_filled(s.to_screen(u, v), me.brush, if me.erase { Color32::from_rgba_unmultiplied(255, 60, 60, 40) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 40) });
        }
        if !(s.resp.drag_stopped() || single) || me.stroke.is_empty() {
            return;
        }
        // The stroke into the source image's own pixels, without gaps between the points.
        let r_uv = me.brush as f64 / s.zoom as f64;
        let mut dabs = Vec::new();
        let pts = std::mem::take(&mut me.stroke);
        let erase = me.erase;
        let mut dab = |u: f64, v: f64| {
            if let Some(q) = to_px(src, u, v) {
                let r = to_px(src, u + r_uv, v).or_else(|| to_px(src, u - r_uv, v)).map(|e| (e[0] - q[0]).hypot(e[1] - q[1])).unwrap_or(r_uv);
                dabs.push(MaskDab { x: q[0], y: q[1], r, erase });
            }
        };
        dab(pts[0].0, pts[0].1);
        for w in pts.windows(2) {
            let d = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
            let k = (d / (0.4 * r_uv)).ceil().max(1.0) as usize;
            for t in 1..=k {
                let a = t as f64 / k as f64;
                dab(w[0].0 + a * (w[1].0 - w[0].0), w[0].1 + a * (w[1].1 - w[0].1));
            }
        }
        if dabs.is_empty() {
            self.status = crate::trf!("The stroke is outside image {}: Shift+click there to choose the image under it.", src + 1);
            return;
        }
        if erase && self.items[src].mask.is_empty() {
            return;
        }
        Arc::make_mut(&mut self.items[src].mask).extend(dabs);
        self.request_preview();
    }

    /// Start drawing the crop frame on the panorama, or apply it.
    fn toggle_crop(&mut self) {
        let full = self.pano.as_ref().map(|p| bounds(&p.outlines)).filter(|b| b.1 > b.0 && b.3 > b.2);
        let Some(full) = full else {
            self.crop_edit = None;
            self.status = tr("Preview the panorama first.").into();
            return;
        };
        if let Some(e) = self.crop_edit.take() {
            let f = e.frame;
            let f = (f.0.max(full.0).round(), f.1.min(full.1).round(), f.2.max(full.2).round(), f.3.min(full.3).round());
            if f.1 - f.0 < 2.0 || f.3 - f.2 < 2.0 {
                self.crop = e.before;
                self.status = tr("The frame is too small: the crop is unchanged.").into();
            } else if f.0 <= full.0 + 0.5 && f.1 >= full.1 - 0.5 && f.2 <= full.2 + 0.5 && f.3 >= full.3 - 0.5 {
                self.crop = None;
                self.status = tr("The frame covers the whole panorama: not cropped.").into();
            } else {
                let area = (f.1 - f.0 + 1.0) * (f.3 - f.2 + 1.0) / ((full.1 - full.0 + 1.0) * (full.3 - full.2 + 1.0));
                self.crop = Some(f);
                self.note_after = Some(crate::trf!("Cropped to {}×{} px ({} % of the panorama). The crop is used when saving; it is removed when you change the geometry.", f.1 - f.0 + 1.0, f.3 - f.2 + 1.0, (100.0 * area).round()));
            }
            if self.crop.is_some() || e.before.is_some() {
                self.request_preview();
            }
        } else {
            self.crop_edit = Some(CropEdit { frame: self.crop.unwrap_or(full), before: self.crop, drag: None });
            self.view = ViewKind::Pano;
            self.drag = None;
            if self.crop.is_some() {
                self.request_preview();
            }
        }
        self.vstate.fit = true;
    }

    /// Drawing the crop frame on the shown panorama (`full`: its whole extent):
    /// draw, resize or move the frame; Esc cancels.
    fn crop_input(&mut self, ui: &egui::Ui, s: &super::Shown, full: (f64, f64, f64, f64)) {
        use super::finish::CropGrip;
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some(e) = self.crop_edit.take() {
                self.crop = e.before;
                if e.before.is_some() {
                    self.request_preview();
                }
                self.vstate.fit = true;
                self.status = tr("Crop cancelled.").into();
            }
            return;
        }
        let Some(e) = self.crop_edit.as_mut() else { return };
        let to_uv = |p: Pos2| {
            let (x, y) = s.to_image(p);
            (super::clamp_safe(x, full.0, full.1), super::clamp_safe(y, full.2, full.3))
        };
        let screen = |f: &(f64, f64, f64, f64)| egui::Rect::from_min_max(s.to_screen(f.0, f.2), s.to_screen(f.1, f.3));
        let r = screen(&e.frame);
        let whole = e.frame == full;
        let grip_at = |p: Pos2| -> CropGrip {
            let near = 8.0;
            let inside_y = p.y > r.top() - near && p.y < r.bottom() + near;
            let inside_x = p.x > r.left() - near && p.x < r.right() + near;
            let g = [(p.x - r.left()).abs() < near && inside_y, (p.y - r.top()).abs() < near && inside_x, (p.x - r.right()).abs() < near && inside_y, (p.y - r.bottom()).abs() < near && inside_x];
            if g.iter().any(|v| *v) {
                CropGrip::Edges(g)
            } else if r.contains(p) && !whole {
                CropGrip::Move
            } else {
                CropGrip::New
            }
        };
        if let Some(p) = s.resp.hover_pos() {
            let icon = match e.drag.map(|d| d.0).unwrap_or_else(|| grip_at(p)) {
                CropGrip::Edges([l, t, rr, b]) if (l && t) || (rr && b) => egui::CursorIcon::ResizeNwSe,
                CropGrip::Edges([l, t, rr, b]) if (rr && t) || (l && b) => egui::CursorIcon::ResizeNeSw,
                CropGrip::Edges([l, _, rr, _]) if l || rr => egui::CursorIcon::ResizeHorizontal,
                CropGrip::Edges(_) => egui::CursorIcon::ResizeVertical,
                CropGrip::Move => egui::CursorIcon::Grab,
                CropGrip::New => egui::CursorIcon::Crosshair,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        if s.resp.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = s.resp.interact_pointer_pos() {
                e.drag = Some((grip_at(p), e.frame, to_uv(p)));
            }
        }
        if let (Some((grip, f0, a)), Some(p)) = (e.drag, s.resp.interact_pointer_pos()) {
            let b = to_uv(p);
            let min = (0.01 * (full.1 - full.0).max(full.3 - full.2)).max(4.0);
            let f = match grip {
                CropGrip::New => (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1)),
                CropGrip::Move => {
                    let dx = super::clamp_safe(b.0 - a.0, full.0 - f0.0, full.1 - f0.1);
                    let dy = super::clamp_safe(b.1 - a.1, full.2 - f0.2, full.3 - f0.3);
                    (f0.0 + dx, f0.1 + dx, f0.2 + dy, f0.3 + dy)
                }
                CropGrip::Edges(g) => (
                    if g[0] { b.0.min(f0.1 - min) } else { f0.0 },
                    if g[2] { b.0.max(f0.0 + min) } else { f0.1 },
                    if g[1] { b.1.min(f0.3 - min) } else { f0.2 },
                    if g[3] { b.1.max(f0.2 + min) } else { f0.3 },
                ),
            };
            if f.1 - f.0 >= min && f.3 - f.2 >= min {
                e.frame = (f.0.max(full.0), f.1.min(full.1), f.2.max(full.2), f.3.min(full.3));
            }
        }
        if s.resp.drag_stopped() {
            e.drag = None;
        }
        // The frame, its size, and everything outside it dimmed.
        let f = e.frame;
        let r = screen(&f);
        let all = screen(&full);
        let p = ui.painter().with_clip_rect(s.rect);
        let dim = Color32::from_black_alpha(140);
        for part in [
            egui::Rect::from_min_max(all.min, Pos2::new(all.max.x, r.top())),
            egui::Rect::from_min_max(Pos2::new(all.min.x, r.bottom()), all.max),
            egui::Rect::from_min_max(Pos2::new(all.min.x, r.top()), Pos2::new(r.left(), r.bottom())),
            egui::Rect::from_min_max(Pos2::new(r.right(), r.top()), Pos2::new(all.max.x, r.bottom())),
        ] {
            if part.is_positive() {
                p.rect_filled(part, 0.0, dim);
            }
        }
        p.rect_stroke(r, 0.0, Stroke::new(2.0, Color32::from_rgb(255, 210, 0)), egui::StrokeKind::Middle);
        p.text(r.left_top() + egui::vec2(6.0, 6.0), egui::Align2::LEFT_TOP, format!("{:.0} × {:.0} px", f.1 - f.0 + 1.0, f.3 - f.2 + 1.0), egui::FontId::proportional(16.0), Color32::from_rgb(255, 210, 0));
    }

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            for (k, l) in [(ViewKind::Image, tr("Image + keypoints")), (ViewKind::Pair, tr("Pair matches")), (ViewKind::Pano, tr("Panorama"))] {
                if self.simple && k == ViewKind::Pair {
                    continue;
                }
                if ui.selectable_label(self.view == k, l).clicked() {
                    self.view = k;
                    self.vstate.fit = true;
                }
            }
            ui.separator();
            // Short buttons: the toolbar wraps to fewer lines on small screens.
            if ui.button(" + ").on_hover_text(tr("Zoom in (+ or the mouse wheel)")).clicked() {
                super::zoom_by(&mut self.vstate, 1.5);
            }
            if ui.button(" − ").on_hover_text(tr("Zoom out (− or the mouse wheel)")).clicked() {
                super::zoom_by(&mut self.vstate, 1.0 / 1.5);
            }
            if ui.button(tr("Fit")).on_hover_text(tr("F (or 0, Home); + / − zoom")).clicked() {
                self.vstate.fit = true;
            }
            ui.checkbox(&mut self.overlays, tr("overlays"));
            let solved = self.items.iter().any(|it| matches!(it.solved, Some(Solved::Found(_))));
            if ui
                .add_enabled(solved, egui::Button::selectable(self.sky_grid && solved, tr("⊕ Sky grid")))
                .on_hover_text(tr("The sky's coordinates on the panorama, as Siril's celestial grid: lines of right ascension and declination, and the names of the brightest stars and of the Messier and other bright objects. From the photos found on the star catalogue; approximate where the sky turned between the shots."))
                .on_disabled_hover_text(tr("Needs photos of the sky found on the star catalogue: switch on “plate solve photos of the sky” (step 2) and match again."))
                .clicked()
            {
                self.sky_grid = !self.sky_grid;
            }
            ui.separator();
            super::view_stretch_ui(ui, "pano_disp", &mut self.disp_view, Some(&mut self.disp_processed));
            ui.separator();
            let editing = self.crop_edit.is_some();
            if ui.selectable_label(editing, if editing { tr("✂ Crop: done") } else { tr("✂ Crop") }).on_hover_text(tr("Draw the part of the panorama to keep: drag a frame, drag its edges or corners to adjust it, drag inside it to move it. Click again when done (Esc cancels). Only that part is rendered and saved, so a big panorama needs less memory.")).clicked() {
                self.toggle_crop();
            }
            if ui.selectable_label(self.mask_edit.is_some(), tr("✐ Source masks")).on_hover_text(tr("Paint where a chosen image must be used, e.g. the one frame where a moving tree, person or car is sharp, or where the blend left a ghost. Each image has its own mask; paint as many as needed")).clicked() {
                self.toggle_masks();
            }
            if ui.selectable_label(self.p.autofill, tr("◧ Fill empty areas")).on_hover_text(tr("Auto-complete, as in Microsoft ICE: the parts of the frame no photo covers (the empty corners and edges, inside the crop frame if there is one) are filled with made-up sky and ground that continue what is around them, with stars and grain copied from the photos. In the preview and when saving. Made up: check it before you publish, and say so where it matters.")).clicked() {
                self.p.autofill = !self.p.autofill;
                self.status = if self.p.autofill { tr("The empty areas are filled (in the preview and when saving).") } else { tr("The empty areas are left empty.") }.into();
                self.request_preview();
            }
            if ui.button(tr("⬇ Save & post-process…")).on_hover_text(tr("Save the panorama, then open it in the Post-processing tab")).clicked() {
                self.ask_save_pano();
                self.post_after = self.dialog.as_ref().is_some_and(|d| d.1 == "save");
            }
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new(tr("↶ Undo"))).on_hover_text(format!("{} (Ctrl+Z)", self.undo.last().map(|u| u.1.clone()).unwrap_or_default())).clicked() {
                self.step(false);
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new(tr("↷ Redo"))).on_hover_text(crate::trf!("{} (Ctrl+Y or Ctrl+Shift+Z)", self.redo.last().map(|u| u.1.clone()).unwrap_or_default())).clicked() {
                self.step(true);
            }
        });
        if self.horizon_edit.is_some() {
            self.horizon_tools(ui);
        }
        if self.mask_edit.is_some() {
            self.mask_tools(ui);
        }
        let status_h = 60.0;
        let avail = ui.available_size();
        ui.allocate_ui(egui::vec2(avail.x, (avail.y - status_h).max(100.0)), |ui| match self.view {
            ViewKind::Image => self.show_image(ui, ctx),
            ViewKind::Pair => self.show_pair(ui, ctx),
            ViewKind::Pano => self.show_pano(ui, ctx),
        });
        let col = if ui.visuals().dark_mode { Color32::from_rgb(255, 160, 128) } else { Color32::from_rgb(140, 0, 0) };
        let status = match self.job_t0 {
            Some(t0) if self.busy && t0.elapsed().as_secs_f64() >= 1.0 => crate::trf!("{} – {} elapsed", self.status, super::duration(t0.elapsed().as_secs_f64())),
            _ => self.status.clone(),
        };
        egui::ScrollArea::vertical().id_salt("status").show(ui, |ui| ui.colored_label(col, status));
        // Shortcuts that work even while typing in a field.
        use egui::{Key, KeyboardShortcut as Ks, Modifiers as Mo};
        let (save_pano, save_proj, open, run) = ctx.input_mut(|i| {
            (i.consume_shortcut(&Ks::new(Mo::COMMAND | Mo::SHIFT, Key::S)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::S)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::O)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::Enter)))
        });
        if save_pano {
            self.ask_save_pano();
        } else if save_proj {
            self.ask_save_project(true);
        }
        if open {
            self.ask_open_project();
        }
        if run && !self.items.is_empty() {
            self.match_pairs(None, true);
        }
        // Keys: arrows nudge in the panorama view, +/-/0 zoom, F fits, P previews.
        if !ctx.egui_wants_keyboard_input() {
            let (fit, prev) = ui.input(|i| (i.key_pressed(Key::F) && i.modifiers.is_none(), i.key_pressed(Key::P) && i.modifiers.is_none()));
            if fit {
                self.vstate.fit = true;
            }
            if prev && self.layout.is_some() {
                self.request_preview();
            }
            let (z, y, sh) = ui.input(|i| (i.modifiers.command && i.key_pressed(egui::Key::Z), i.modifiers.command && i.key_pressed(egui::Key::Y), i.modifiers.shift));
            if z {
                self.step(sh);
            } else if y {
                self.step(true);
            }
            let (keys, shift) = ui.input(|i| {
                let k: Vec<egui::Key> = [egui::Key::ArrowLeft, egui::Key::ArrowRight, egui::Key::ArrowUp, egui::Key::ArrowDown, egui::Key::Plus, egui::Key::Equals, egui::Key::Minus, egui::Key::Num0, egui::Key::Home]
                    .into_iter()
                    .filter(|k| i.key_pressed(*k))
                    .collect();
                (k, i.modifiers.shift)
            });
            let m = if shift { 10.0 } else { 1.0 };
            for k in keys {
                match k {
                    egui::Key::ArrowLeft if self.view == ViewKind::Pano => self.nudge(-m, 0.0),
                    egui::Key::ArrowRight if self.view == ViewKind::Pano => self.nudge(m, 0.0),
                    egui::Key::ArrowUp if self.view == ViewKind::Pano => self.nudge(0.0, -m),
                    egui::Key::ArrowDown if self.view == ViewKind::Pano => self.nudge(0.0, m),
                    // Image and Pair views: the arrow keys pan by a tenth of the view.
                    egui::Key::ArrowLeft => self.vstate.center.x -= 100.0 / self.vstate.zoom,
                    egui::Key::ArrowRight => self.vstate.center.x += 100.0 / self.vstate.zoom,
                    egui::Key::ArrowUp => self.vstate.center.y -= 100.0 / self.vstate.zoom,
                    egui::Key::ArrowDown => self.vstate.center.y += 100.0 / self.vstate.zoom,
                    egui::Key::Plus | egui::Key::Equals => super::zoom_by(&mut self.vstate, 1.5),
                    egui::Key::Minus => super::zoom_by(&mut self.vstate, 1.0 / 1.5),
                    egui::Key::Num0 | egui::Key::Home => self.vstate.fit = true,
                    _ => {}
                }
            }
        }
    }

    fn show_image(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.items.is_empty() {
            ui.centered_and_justified(|ui| ui.weak(tr("Add images to begin.")));
            return;
        }
        let i = self.sel.min(self.items.len() - 1);
        let tex = self.tex_for(ctx, i);
        let it = &self.items[i];
        let (w, h) = (it.img.w as f64, it.img.h as f64);
        let kp = it.feats.as_ref().map(|f| crate::trf!("{} keypoints", f.pts.len())).unwrap_or_else(|| tr("no features yet").into());
        ui.label(crate::trf!("Image {}: {}  ({}×{}, {})", i + 1, basename(&it.path), w, h, kp));
        match &it.solved {
            Some(Solved::Found(sol)) => {
                let (ra, de) = sol.centre();
                let (fw, fh) = sol.fov();
                let ra_h = ra / 15.0;
                let pos = format!("{}h {:02}m, {}{:.1}°", ra_h.floor(), ((ra_h - ra_h.floor()) * 60.0).floor(), if de >= 0.0 { "+" } else { "−" }, de.abs());
                ui.weak(crate::trf!("★ Star catalogue: centre at RA {}, {} stars identified (mean error {} px), field {}° × {}°.", pos, sol.stars.len(), format!("{:.1}", sol.rms), format!("{fw:.0}"), format!("{fh:.0}")));
            }
            Some(Solved::Failed(e)) => {
                ui.weak(crate::trf!("Star catalogue: not found ({}).", e));
            }
            _ => {}
        }
        let s = image_view(ui, &mut self.vstate, (w, h), &[(&tex, [0.5, w + 0.5, 0.5, h + 0.5])], true);
        if self.overlays {
            if let Some(f) = &it.feats {
                let p = ui.painter().with_clip_rect(s.rect);
                for q in &f.pts {
                    p.circle_stroke(s.to_screen(q[0], q[1]), 4.0, Stroke::new(1.2, Color32::YELLOW));
                }
            }
            // Stars identified on the catalogue.
            if let Some(sol) = it.solved.as_ref().and_then(|s| s.found()) {
                let p = ui.painter().with_clip_rect(s.rect);
                for (_, q) in &sol.stars {
                    p.circle_stroke(s.to_screen(q[0], q[1]), 7.0, Stroke::new(1.2, Color32::from_rgb(90, 200, 255)));
                }
            }
        }
    }

    fn show_pair(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.cp.is_some() {
            self.show_cp(ui, ctx);
            return;
        }
        let Some(q) = self.pairs.get(self.pairsel).cloned() else {
            ui.centered_and_justified(|ui| ui.weak(tr("No pairs matched yet.")));
            return;
        };
        let (ta, tb) = (self.tex_for(ctx, q.i), self.tex_for(ctx, q.j));
        let (a, b) = (&self.items[q.i].img, &self.items[q.j].img);
        let (wa, ha, wb, hb) = (a.w as f64, a.h as f64, b.w as f64, b.h as f64);
        let xo = wa * 1.03;
        ui.label(crate::trf!("Pair {}–{}: {} matches, {} inliers (green){}{}", q.i + 1, q.j + 1, q.p1.len(), q.inl.len(), if self.show_outliers { tr(", outliers red") } else { "" }, if q.ok { "" } else { " · NOT USED" }));
        let s = image_view(ui, &mut self.vstate, (xo + wb, ha.max(hb)), &[(&ta, [0.5, wa + 0.5, 0.5, ha + 0.5]), (&tb, [xo + 0.5, xo + wb + 0.5, 0.5, hb + 0.5])], true);
        let p = ui.painter().with_clip_rect(s.rect);
        let mut isin = vec![false; q.p1.len()];
        q.inl.iter().for_each(|&k| isin[k] = true);
        for k in 0..q.p1.len() {
            if !isin[k] && !self.show_outliers {
                continue;
            }
            let col = if isin[k] { Color32::from_rgba_unmultiplied(0, 255, 0, 220) } else { Color32::from_rgba_unmultiplied(255, 0, 0, 150) };
            p.line_segment([s.to_screen(q.p1[k][0], q.p1[k][1]), s.to_screen(q.p2[k][0] + xo, q.p2[k][1])], Stroke::new(1.0, col));
        }
    }

    fn show_pano(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let dk = self.disp_key();
        let Some(pv) = &mut self.pano else {
            ui.centered_and_justified(|ui| ui.weak(tr("Panorama: press “Preview panorama”.")));
            return;
        };
        let key = (self.p.black.to_bits(), self.p.mid.to_bits(), self.p.white.to_bits(), dk | 1);
        if pv.tex.is_none() || pv.tex_key != key {
            let shown = if self.disp_processed { look(&pv.img, 1.0, &self.p) } else { crate::finish::display_view(&pv.img, self.disp_view, &pv.img) };
            pv.tex = Some(texture(ctx, "pano", &shown));
            pv.tex_key = key;
        }
        let tex = pv.tex.clone().unwrap();
        let b = pv.box_;
        let (u0, u1) = (b.0 - 0.5, pv.img.w as f64 / pv.scale + b.0 - 0.5);
        let (v0, v1) = (b.2 - 0.5, pv.img.h as f64 / pv.scale + b.2 - 0.5);
        if let Some(l) = &self.layout {
            let w = self.items[l.ref_].img.w as f64;
            let fov = 2.0 * (w / 2.0 / (l.f * self.p.focal)).atan().to_degrees();
            ui.label(crate::trf!("{}: {}×{} px · focal {} px (ref. FOV {}°){}{}", tr(self.p.proj.name()), b.1 - b.0 + 1.0, b.3 - b.2 + 1.0, format!("{:.0}", l.f * self.p.focal), format!("{fov:.1}"), l.typical.map(|t| crate::trf!(" · typical error {} px", format!("{t:.1}"))).unwrap_or_default(), if self.overlays { tr(" · orange = selected, cyan = reference, yellow = links") } else { "" }));
        }
        // Image coordinates of this view = output (u, v) coordinates.
        let mut vs = self.vstate;
        let s = image_view(ui, &mut vs, (u1 - u0, v1 - v0), &[], !ui.input(|i| i.modifiers.ctrl) && self.crop_edit.is_none() && self.mask_edit.is_none());
        // image_view assumes the box starts at 0.5: shift the view so (u, v) are used directly.
        if vs.fit != self.vstate.fit || self.vstate.fit {
            vs.center = egui::vec2(((u0 + u1) / 2.0) as f32, ((v0 + v1) / 2.0) as f32);
        }
        self.vstate = vs;
        let s = super::Shown { center: self.vstate.center, ..s };
        let p = ui.painter().with_clip_rect(s.rect);
        let uv = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        p.image(tex.id(), egui::Rect::from_min_max(s.to_screen(u0, v0), s.to_screen(u1, v1)), uv, Color32::WHITE);
        // When zoomed in, the visible area is re-rendered at screen resolution after a short pause.
        let pscale = self.pano.as_ref().unwrap().scale;
        let need = (s.zoom as f64 * ctx.pixels_per_point() as f64).min(1.0);
        if need > 1.25 * pscale {
            let (x0, x1, y0, y1) = s.visible();
            let bx = (x0.floor().max(b.0), x1.ceil().min(b.1), y0.floor().max(b.2), y1.ceil().min(b.3));
            if bx.1 > bx.0 && bx.3 > bx.2 {
                let key = hash_of(&format!("{bx:?}{need}{}", self.pano_version));
                if self.detail.as_ref().map(|d| d.key) != Some(key) {
                    match self.detail_req {
                        Some((k, t)) if k == key => {
                            if t.elapsed() > std::time::Duration::from_millis(350) && !self.detail_busy {
                                self.detail_req = None;
                                self.render_detail(key, bx, need);
                            }
                        }
                        _ => self.detail_req = Some((key, std::time::Instant::now())),
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(120));
                }
            }
        } else {
            self.detail = None;
        }
        if let Some(d) = &mut self.detail {
            let tk = (self.p.black.to_bits(), self.p.mid.to_bits(), self.p.white.to_bits(), dk);
            if d.tex_key != tk {
                let shown = if self.disp_processed { look(&d.img, 1.0, &self.p) } else { crate::finish::display_view(&d.img, self.disp_view, self.pano.as_ref().map(|p| &p.img).unwrap_or(&d.img)) };
                d.tex = texture(ctx, "detail", &shown);
                d.tex_key = tk;
            }
            let (du0, du1) = (d.box_.0 - 0.5, d.img.w as f64 / d.scale + d.box_.0 - 0.5);
            let (dv0, dv1) = (d.box_.2 - 0.5, d.img.h as f64 / d.scale + d.box_.2 - 0.5);
            p.image(d.tex.id(), egui::Rect::from_min_max(s.to_screen(du0, dv0), s.to_screen(du1, dv1)), uv, Color32::WHITE);
        }
        let outlines = self.pano.as_ref().unwrap().outlines.clone();
        let mids: Vec<Option<Pt>> = outlines.iter().map(|o| {
            let fin: Vec<&Pt> = o.iter().filter(|q| q[0].is_finite() && q[1].is_finite()).collect();
            (!fin.is_empty()).then(|| [fin.iter().map(|q| q[0]).sum::<f64>() / fin.len() as f64, fin.iter().map(|q| q[1]).sum::<f64>() / fin.len() as f64])
        }).collect();
        let refi = self.layout.as_ref().map(|l| l.ref_).unwrap_or(0);
        if self.overlays {
            if let Some(l) = &self.layout {
                for &v in &l.order {
                    if let (Some(pa), Some(a), Some(bb)) = (l.parent[v], mids[v], l.parent[v].and_then(|q| mids[q])) {
                        let _ = pa;
                        p.line_segment([s.to_screen(a[0], a[1]), s.to_screen(bb[0], bb[1])], Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 0, 200)));
                    }
                }
            }
            for (i, o) in outlines.iter().enumerate() {
                if o.is_empty() {
                    continue;
                }
                let col = if i == self.sel { Color32::from_rgb(255, 165, 0) } else if self.selected.contains(&i) { Color32::from_rgb(0, 191, 255) } else if i == refi { Color32::from_rgb(0, 255, 255) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 180) };
                let mut pts: Vec<Pos2> = o.iter().filter(|q| q[0].is_finite()).map(|q| s.to_screen(q[0], q[1])).collect();
                if let Some(&f0) = pts.first() {
                    pts.push(f0);
                }
                p.line(pts, Stroke::new(if i == self.sel { 3.0 } else { 1.2 }, col));
                if let Some(m) = mids[i] {
                    p.text(s.to_screen(m[0], m[1]), egui::Align2::CENTER_CENTER, if i == refi { crate::trf!("{} (ref)", i + 1) } else { (i + 1).to_string() }, egui::FontId::proportional(22.0), col);
                }
            }
        }
        if self.overlays || self.horizon_edit.is_some() {
            self.draw_horizon(&p, &s);
        }
        if self.sky_grid {
            self.draw_sky_grid(&p, &s);
        }
        if self.crop_edit.is_some() {
            self.crop_input(ui, &s, bounds(&outlines));
            return;
        }
        if self.mask_edit.is_some() {
            self.mask_input(ui, &s);
            return;
        }
        if self.horizon_edit.is_some() {
            self.horizon_input(ui, &s);
            return;
        }
        // Click selects; Ctrl + drag moves an image (with the images attached beyond it).
        let pick = |x: f64, y: f64| -> Option<usize> {
            let mut best = None;
            let mut bd = f64::MAX;
            for (i, o) in outlines.iter().enumerate() {
                let poly: Vec<(f64, f64)> = o.iter().map(|q| (q[0], q[1])).collect();
                if poly.is_empty() || !crate::image::inside_polygon(&poly, x, y) {
                    continue;
                }
                if let Some(m) = mids[i] {
                    let d = (x - m[0]).hypot(y - m[1]);
                    if d < bd {
                        bd = d;
                        best = Some(i);
                    }
                }
            }
            best
        };
        let ctrl = ui.input(|i| i.modifiers.ctrl);
        if s.resp.clicked() {
            if let Some(pos) = s.resp.interact_pointer_pos() {
                let (x, y) = s.to_image(pos);
                if let Some(k) = pick(x, y) {
                    self.sel = k;
                    self.selected = [k].into_iter().collect();
                }
            }
        }
        if s.resp.drag_started_by(egui::PointerButton::Primary) && ctrl {
            if let Some(pos) = s.resp.interact_pointer_pos() {
                let (x, y) = s.to_image(pos);
                if let Some(k) = pick(x, y) {
                    self.sel = k;
                    if k == refi {
                        self.status = tr("The reference image stays fixed; drag another image (or use Pitch / Roll).").into();
                    } else {
                        self.drag = Some((k, (x, y), self.items[k].offset));
                    }
                }
            }
        }
        if let Some((k, start, off0)) = self.drag {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                let (x, y) = s.to_image(pos);
                let mut pts: Vec<Pos2> = outlines[k].iter().filter(|q| q[0].is_finite()).map(|q| s.to_screen(q[0] + x - start.0, q[1] + y - start.1)).collect();
                if let Some(&f0) = pts.first() {
                    pts.push(f0);
                }
                p.line(pts, Stroke::new(3.0, Color32::from_rgb(255, 165, 0)));
                if s.resp.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
                    self.items[k].offset = [off0[0] + x - start.0, off0[1] + y - start.1];
                    self.drag = None;
                    self.request_preview();
                }
            }
        }
    }
}

/// An angle (row of a grid): a slider, a box to drag finely (0.02° per pixel)
/// or type into, and ◀ ▶ steps of 0.1° (Shift: 1°). `fine_centre`: the slider
/// is logarithmic, so small corrections near 0° (the usual case for pitch and
/// roll) take most of its length instead of a few pixels.
pub(crate) fn angle(ui: &mut egui::Ui, label: &str, v: &mut f64, max: f64, fine_centre: bool) -> bool {
    ui.label(label);
    let mut ch = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = 130.0;
        let mut s = egui::Slider::new(v, -max..=max).show_value(false);
        if fine_centre {
            s = s.logarithmic(true).smallest_positive(0.05);
        }
        ch |= ui.add(s).on_hover_text(tr("Drag for big changes; fine near 0°")).changed();
        ch |= ui.add(egui::DragValue::new(v).speed(0.02).range(-max..=max).max_decimals(2).suffix("°")).on_hover_text(tr("Drag slowly for fine changes, or double-click and type")).changed();
        let step = if ui.input(|i| i.modifiers.shift) { 1.0 } else { 0.1 };
        if ui.small_button("◀").on_hover_text(tr("−0.1° (Shift: −1°)")).clicked() {
            *v = ((*v - step) * 100.0).round() / 100.0;
            ch = true;
        }
        if ui.small_button("▶").on_hover_text(tr("+0.1° (Shift: +1°)")).clicked() {
            *v = ((*v + step) * 100.0).round() / 100.0;
            ch = true;
        }
        *v = v.clamp(-max, max);
    });
    ui.end_row();
    ch
}

/// Histogram (log scale) with black / white point markers and the transfer curve.
pub fn histogram(ui: &mut egui::Ui, img: &Img, stretch: Option<(f64, f64, f64)>) {
    histogram_of(ui, img, 1.0, stretch);
}

/// The histogram of `img` × `gain`, counted once per image and kept (the
/// window redraws often; the image stays the same between changes).
pub fn histogram_of(ui: &mut egui::Ui, img: &Img, gain: f32, stretch: Option<(f64, f64, f64)>) {
    // Identity of the image: where its pixels are, their number, a few of them, and the gain.
    let probe: Vec<u32> = (0..8).filter_map(|k| img.px.get(k * img.px.len() / 8)).map(|c| c[0].to_bits() ^ c[1].to_bits().rotate_left(11) ^ c[2].to_bits().rotate_left(22)).collect();
    let id = egui::Id::NULL.with(("histogram", img.px.as_ptr() as usize, img.px.len(), img.w, gain.to_bits(), probe));
    let hc: Vec<usize> = ui.ctx().memory_mut(|m| {
        m.data
            .get_temp_mut_or_insert_with(id, || {
                if gain == 1.0 {
                    lum_histogram(img, 256)
                } else {
                    let px: Vec<[f32; 3]> = img.subsample(img.step_for(500_000)).map(|c| [c[0] * gain, c[1] * gain, c[2] * gain]).collect();
                    lum_histogram(&Img::from_vec(1, px.len(), px), 256)
                }
            })
            .clone()
    });
    let ys: Vec<f32> = hc.iter().map(|&c| ((c + 1) as f32).log10()).collect();
    let mx = ys.iter().cloned().fold(1.0, f32::max);
    let (resp, p) = ui.allocate_painter(egui::vec2(ui.available_width(), 60.0), egui::Sense::hover());
    let r = resp.rect;
    p.rect_filled(r, 2.0, Color32::from_gray(30));
    let n = ys.len() as f32;
    for (k, y) in ys.iter().enumerate() {
        let x = r.left() + (k as f32 + 0.5) / n * r.width();
        p.line_segment([Pos2::new(x, r.bottom()), Pos2::new(x, r.bottom() - y / mx * r.height())], Stroke::new(r.width() / n + 0.5, Color32::from_gray(170)));
    }
    if let Some((b, m, w)) = stretch {
        let pts: Vec<Pos2> = (0..=100).map(|t| {
            let x = t as f64 / 100.0;
            let v = mtf(m as f32, ((x - b) / (w - b).max(1e-6)).clamp(0.0, 1.0) as f32);
            Pos2::new(r.left() + x as f32 * r.width(), r.bottom() - v * r.height())
        }).collect();
        p.line(pts, Stroke::new(1.5, Color32::from_rgb(255, 165, 0)));
        for (v, c) in [(b, Color32::from_rgb(0, 191, 255)), (w, Color32::WHITE)] {
            let x = r.left() + v as f32 * r.width();
            p.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(1.5, c));
        }
    }
}

/// Hooks for the headless window tests (tests/gui.rs).
#[doc(hidden)]
impl StitchTab {
    pub fn t_report(&self) -> Option<&super::JobReport> {
        self.report.as_ref()
    }
    /// Show the preview in a display mode of its own (not "As processed").
    pub fn t_set_display(&mut self, v: crate::finish::ViewStretch) {
        (self.disp_view, self.disp_processed) = (v, false);
    }
    pub fn t_status(&self) -> &str {
        &self.status
    }
    pub fn t_items(&self) -> usize {
        self.items.len()
    }
    pub fn t_offsets(&self) -> Vec<[f64; 2]> {
        self.items.iter().map(|i| i.offset).collect()
    }
    pub fn t_pairs_ok(&self) -> usize {
        self.pairs.iter().filter(|q| q.ok).count()
    }
    pub fn t_run_all(&mut self) {
        self.match_pairs(None, true);
    }
    pub fn t_pair_msgs(&self) -> Vec<String> {
        self.pairs.iter().map(|q| q.msg.clone()).collect()
    }
    pub fn t_set_autofill(&mut self, on: bool) {
        self.p.autofill = on;
    }
    pub fn t_set_out_pct(&mut self, pct: f64) {
        self.p.out_pct = pct;
    }
    pub fn t_set_solve(&mut self, on: bool) {
        self.p.solve = on;
    }
    /// Images found on the star catalogue.
    pub fn t_solved(&self) -> Vec<usize> {
        (0..self.items.len()).filter(|&i| self.items[i].solved.as_ref().and_then(|s| s.found()).is_some()).collect()
    }
    pub fn t_pano_size(&self) -> Option<(usize, usize)> {
        self.pano.as_ref().map(|p| (p.img.w, p.img.h))
    }
    pub fn t_set_blend(&mut self, b: Blend) {
        self.p.blend = b;
        self.request_preview();
    }
    pub fn t_set_projection(&mut self, p: Projection) {
        self.p.proj = p;
        self.crop = None;
        self.request_preview();
    }
    pub fn t_crop(&mut self) {
        self.auto_crop();
    }
    pub fn t_cropped(&self) -> bool {
        self.crop.is_some()
    }
    pub fn t_undo(&mut self, redo: bool) {
        self.step(redo);
    }
    pub fn t_undo_len(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    pub fn t_set_corr(&mut self, i: usize, c: Correction) {
        self.items[i].corr = c;
        self.request_preview();
    }
    pub fn t_corr(&self) -> Vec<Correction> {
        self.items.iter().map(|i| i.corr).collect()
    }
    /// The outline of image i in the preview (output coordinates).
    pub fn t_outline(&self, i: usize) -> Vec<Pt> {
        self.pano.as_ref().and_then(|p| p.outlines.get(i).cloned()).unwrap_or_default()
    }
    /// Auto horizon, as the button (the preview follows).
    pub fn t_set_overlays(&mut self, on: bool) {
        self.overlays = on;
    }
    pub fn t_set_sky_grid(&mut self, on: bool) {
        self.sky_grid = on;
    }
    pub fn t_sky_mapping(&self) -> Option<(M3, f64, usize)> {
        self.sky_mapping()
    }
    pub fn t_auto_horizon(&mut self) -> bool {
        let ok = self.auto_horizon();
        if ok {
            self.request_preview();
        }
        ok
    }
    /// Level the horizon through points (output coordinates), as marked by hand.
    pub fn t_horizon_by_hand(&mut self, pts: &[Pt]) {
        self.horizon_edit = Some(pts.to_vec());
        self.level_by_hand();
    }
    pub fn t_angles(&self) -> (f64, f64, f64) {
        (self.p.yaw, self.p.pitch, self.p.roll)
    }
    pub fn t_set_angles(&mut self, yaw: f64, pitch: f64, roll: f64) {
        (self.p.yaw, self.p.pitch, self.p.roll) = (yaw, pitch, roll);
        self.request_preview();
    }
    /// The preview's box and scale (output coordinates of its pixels).
    pub fn t_preview_box(&self) -> Option<((f64, f64, f64, f64), f64)> {
        self.pano.as_ref().map(|p| (p.box_, p.scale))
    }
    pub fn t_set_lens_fit(&mut self, fit: LensFit, own: bool) {
        (self.p.lens_fit, self.p.lens_own) = (fit, own);
        let note = self.relayout_now();
        self.status = format!("{}{note}", self.lens_summary());
    }
    /// Typical pair error after the refinement (px).
    pub fn t_typical(&self) -> Option<f64> {
        self.layout.as_ref().and_then(|l| l.typical)
    }
    pub fn t_lenses(&self) -> Vec<Lens> {
        self.lenses()
    }
    pub fn t_nudge(&mut self, sel: usize, dx: f64) {
        self.sel = sel;
        self.nudge(dx, 0.0);
    }
    pub fn t_save(&mut self, path: &str) {
        self.save_full(path.to_string());
    }
    pub fn t_zoom(&mut self, k: f32) {
        super::zoom_by(&mut self.vstate, k);
    }
    pub fn t_preview_px(&mut self, px: usize) {
        self.p.prev_px = px;
        self.request_preview();
    }
    pub fn t_detail(&self) -> bool {
        self.detail.is_some()
    }
    pub fn t_ref(&self) -> Option<usize> {
        self.layout.as_ref().map(|l| l.ref_)
    }
}

#[doc(hidden)]
impl StitchTab {
    pub fn t_select(&mut self, sel: usize, also: &[usize]) {
        self.sel = sel;
        self.selected = std::iter::once(sel).chain(also.iter().copied()).collect();
    }
    pub fn t_move(&mut self, d: i64) {
        self.move_selected(d);
    }
    pub fn t_remove(&mut self) {
        self.remove_selected();
    }
    pub fn t_names(&self) -> Vec<String> {
        self.items.iter().map(|i| basename(&i.path)).collect()
    }
}

#[doc(hidden)]
impl StitchTab {
    pub fn t_detect(&mut self) {
        self.detect_only();
    }
    pub fn t_keypoints(&self) -> Vec<usize> {
        self.items.iter().map(|i| i.feats.as_ref().map(|f| f.pts.len()).unwrap_or(0)).collect()
    }
    pub fn t_preview(&self) -> Option<&Img> {
        self.pano.as_ref().map(|p| &p.img)
    }
    /// Images whose outline in the preview contains preview pixel (x, y).
    pub fn t_images_at(&self, x: f64, y: f64) -> Vec<usize> {
        let Some(pv) = &self.pano else { return vec![] };
        let (u, v) = (x / pv.scale + pv.box_.0, y / pv.scale + pv.box_.2);
        pv.outlines.iter().enumerate().filter(|(_, o)| {
            let o: Vec<&Pt> = o.iter().filter(|p| p[0].is_finite() && p[1].is_finite()).collect();
            let mut inside = false;
            for k in 0..o.len() {
                let (a, b) = (o[k], o[(k + 1) % o.len()]);
                if (a[1] > v) != (b[1] > v) && u < a[0] + (v - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                    inside = !inside;
                }
            }
            inside
        }).map(|(i, _)| i).collect()
    }
    /// Accepted pairs: (i, j, by stars).
    /// Control points on images `i` (left) and `j` (right), used as with ✔ Use the points.
    pub fn t_control_points(&mut self, i: usize, j: usize, pts: &[(Pt, Pt)]) {
        let mut cp = self.cp_for(i, j, false);
        cp.pts = pts.to_vec();
        self.cp = Some(cp);
        self.cp_apply();
    }
    pub fn t_set_model(&mut self, m: Model) {
        self.p.model = m;
    }
    /// Pairs placed by hand: (i, j, accepted, points).
    pub fn t_manual_pairs(&self) -> Vec<(usize, usize, bool, usize)> {
        self.pairs.iter().filter(|q| q.manual).map(|q| (q.i, q.j, q.ok, q.p1.len())).collect()
    }
    pub fn t_paint_mask(&mut self, i: usize, dabs: &[MaskDab]) {
        Arc::make_mut(&mut self.items[i].mask).extend_from_slice(dabs);
        self.request_preview();
    }
    pub fn t_masks(&self) -> Vec<Vec<MaskDab>> {
        self.items.iter().map(|it| it.mask.to_vec()).collect()
    }
    pub fn t_links(&self) -> Vec<(usize, usize, bool)> {
        self.pairs.iter().filter(|q| q.ok).map(|q| (q.i, q.j, q.stars)).collect()
    }
    /// Images not linked to the reference image.
    pub fn t_unlinked(&self) -> Vec<usize> {
        self.layout.as_ref().map(|l| (0..self.items.len()).filter(|&i| l.comp[i] != l.comp[l.ref_]).collect()).unwrap_or_default()
    }
    /// Every matched pair: (i, j, accepted, matches, inliers, message).
    pub fn t_pair_results(&self) -> Vec<(usize, usize, bool, usize, usize, String)> {
        self.pairs.iter().map(|q| (q.i, q.j, q.ok, q.p1.len(), q.inl.len(), q.msg.clone())).collect()
    }
    /// Where each image points: (yaw, elevation) in degrees, relative to the reference image.
    pub fn t_directions(&self) -> Vec<(f64, f64)> {
        let Some(l) = &self.layout else { return vec![] };
        let kr = inv(&kmat(l.f, center(&self.items[l.ref_].img)));
        self.items.iter().enumerate().map(|(i, it)| {
            let c = center(&it.img);
            let d = kr * l.g[i] * nalgebra::Vector3::new(c[0], c[1], 1.0);
            let d = d / d.norm() * d[2].signum();
            (d[0].atan2(d[2]).to_degrees(), (-d[1]).asin().to_degrees())
        }).collect()
    }
    pub fn t_sky_rate(&self) -> Option<f64> {
        self.layout.as_ref().and_then(|l| l.sky_rate)
    }
}

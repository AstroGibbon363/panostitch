//! The "Panorama stitcher" tab. Every step runs only when you press its
//! button, and each result can be inspected (keypoints, matches with inliers /
//! outliers, placed outlines and links) and overridden (pairs, reference
//! image, projection, horizon, nudges, brightness).
//!
//! View: left-drag pans, wheel zooms. Panorama view: click an image to select
//! it; Ctrl + drag or the arrow keys (Shift = ×10) move it with the images
//! attached beyond it.

use super::dialogs::{self, Picked};
use super::{combo, image_view, slider, texture, View};
use crate::image::{luminance, quantiles, resize, Bits, Img};
use crate::io::{self, basename, LoadOptions, RawDecoder};
use crate::stitcher::*;
use egui::{Color32, Pos2, Stroke};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

const DISP_MAX: usize = 1600;

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
}

pub type FeatKey = (usize, usize, u64, u64);

#[derive(Clone)]
pub struct PairRes {
    pub i: usize,
    pub j: usize,
    pub p1: Vec<Pt>,
    pub p2: Vec<Pt>,
    pub h: M3,
    pub inl: Vec<usize>,
    pub ok: bool,
    /// Shown in the Pair menu (accepted, or matched on purpose).
    pub listed: bool,
    pub msg: String,
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
    pub lens: Lens,
    pub typical: Option<f64>,
}

struct Preview {
    img: Img,
    tex: Option<egui::TextureHandle>,
    tex_key: (u64, u64, u64, u64),
    box_: (f64, f64, f64, f64),
    scale: f64,
    outlines: Vec<Vec<Pt>>,
    covered: Option<Bits>,
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
}

impl Default for Params {
    fn default() -> Self {
        Self {
            det_px: 2000, nkp: 5000, thr: 0.001, denoise: 1.5, layout: Layout::All, cols: 5, snake: false, ratio: 0.8, maxd: 0.35,
            tol: 5.0, mininl: 20, model: Model::Homography, refine: true, proj: Projection::Cylindrical, blend: Blend::Smart, focal: 1.0,
            yaw: 0.0, pitch: 0.0, roll: 0.0, prev_px: 1800, ev: 0.0, black: 0.0, mid: 0.5, white: 1.0, decoder: RawDecoder::Auto, half: true,
            unlinked: false,
        }
    }
}

impl Params {
    fn preset(&mut self, name: &str) -> &'static str {
        let (det, nkp, thr, den, ratio, maxd, tol, mininl, model, refine, proj, ev, note) = match name {
            "day landscape" => (2000, 5000, 0.001, 0.5, 0.8, 0.35, 4.0, 20, Model::Homography, true, Projection::Cylindrical, 0.0, "Daylight panorama, camera rotated in place."),
            "astro / night landscape" => (2000, 6000, 0.0005, 2.0, 0.85, 0.38, 5.0, 16, Model::Homography, true, Projection::Spherical, 2.0, "Night or Milky Way panorama: more denoising, looser matching, spherical projection."),
            "deep-sky mosaic" => (3000, 4000, 0.002, 0.75, 0.75, 0.3, 3.0, 12, Model::Similarity, false, Projection::Rectilinear, 0.0, "Telescope mosaic: frames only shift and rotate. Use Auto stretch to see faint detail."),
            "macro / microscope" => (2000, 5000, 0.001, 1.0, 0.8, 0.35, 3.0, 15, Model::Translation, false, Projection::Rectilinear, 0.0, "Stage moved under a fixed camera: frames only shift."),
            _ => (2500, 5000, 0.001, 0.5, 0.8, 0.35, 3.0, 20, Model::Affine, false, Projection::Rectilinear, 0.0, "Flat object scanned or photographed in parts."),
        };
        (self.det_px, self.nkp, self.thr, self.denoise, self.ratio, self.maxd, self.tol, self.mininl) = (det, nkp, thr, den, ratio, maxd, tol, mininl);
        (self.model, self.refine, self.proj, self.ev, self.layout, self.blend) = (model, refine, proj, ev, Layout::All, Blend::Smart);
        note
    }
    fn stretched(&self) -> bool {
        !(self.black == 0.0 && self.mid == 0.5 && self.white == 1.0)
    }
    fn featkey(&self) -> FeatKey {
        (self.det_px, self.nkp, self.thr.to_bits(), self.denoise.to_bits())
    }
}

const PRESETS: [&str; 5] = ["day landscape", "astro / night landscape", "deep-sky mosaic", "macro / microscope", "scan / flat document"];

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
    Detected(Vec<(usize, Arc<Features>)>, FeatKey),
    Matched { feats: Vec<(usize, Arc<Features>)>, key: FeatKey, pairs: Vec<PairRes>, layout: Option<Placement>, msg: String },
    Preview(Img, (f64, f64, f64, f64), f64, Vec<Vec<Pt>>, Option<Bits>),
    Done(String),
    Error(String),
}

#[derive(PartialEq, Clone, Copy)]
enum ViewKind {
    Image,
    Pair,
    Pano,
}

pub struct StitchTab {
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
    /// Multi-selection in the image list (always contains `sel` when there are images).
    selected: std::collections::BTreeSet<usize>,
    /// The message to show when the preview being rendered arrives (instead of “Preview ready”).
    note_after: Option<String>,
    undo: Vec<(Snap, String)>,
    redo: Vec<(Snap, String)>,
    committed: Option<(u64, Snap)>,
    pending: Option<(u64, std::time::Instant)>,
    project: Option<String>,
    pending_project: Option<ProjData>,
    pano_version: u64,
    detail: Option<Detail>,
    detail_req: Option<(u64, std::time::Instant)>,
    detail_busy: bool,
}

struct Detail {
    key: u64,
    tex: egui::TextureHandle,
    tex_key: (u64, u64, u64),
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
        s!("Detection size (px)", det_px, usize), s!("Max keypoints", nkp, usize), s!("Corner threshold", thr, f64),
        s!("Denoise (σ px)", denoise, f64), s!("Grid columns", cols, usize), s!("Ratio test", ratio, f64),
        s!("Max distance", maxd, f64), s!("RANSAC tol. (px)", tol, f64), s!("Min. inliers", mininl, usize),
        s!("Focal (× auto)", focal, f64), s!("Yaw (°)", yaw, f64), s!("Pitch (°)", pitch, f64), s!("Roll (°)", roll, f64),
        s!("Preview size", prev_px, usize), s!("Exposure", ev, f64), s!("Black point", black, f64), s!("Midtones", mid, f64),
        s!("White point", white, f64),
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
        return if pb.len() > pa.len() { "add images" } else if pb.len() < pa.len() { "remove images" } else { "reorder images" }.into();
    }
    if a.items.iter().zip(&b.items).any(|(x, y)| x.offset != y.offset) {
        return "move images".into();
    }
    if a.items.iter().zip(&b.items).any(|(x, y)| x.gain != y.gain) {
        return "brightness".into();
    }
    let (mut qa, mut qb) = (a.p.clone(), b.p.clone());
    for ((n, x, _), (_, y, _)) in sliders(&mut qa).into_iter().zip(sliders(&mut qb)) {
        if x != y {
            return format!("{n} {} → {}", fmt_num(x), fmt_num(y));
        }
    }
    let menus = [
        ("Pairs", layout_name(a.p.layout), layout_name(b.p.layout)),
        ("Model", a.p.model.name(), b.p.model.name()),
        ("Projection", a.p.proj.name(), b.p.proj.name()),
        ("Blend", a.p.blend.name(), b.p.blend.name()),
    ];
    for (n, x, y) in menus {
        if x != y {
            return format!("{n} → {y}");
        }
    }
    for (n, x, y) in [("snake", a.p.snake, b.p.snake), ("refine", a.p.refine, b.p.refine), ("show unlinked images", a.p.unlinked, b.p.unlinked)] {
        if x != y {
            return format!("{n} {}", if y { "on" } else { "off" });
        }
    }
    if a.crop != b.crop {
        return if b.crop.is_none() { "uncrop" } else { "auto crop" }.into();
    }
    if a.ref_choice != b.ref_choice {
        return "reference image".into();
    }
    "matching / pairs".into()
}

impl Default for StitchTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            items: vec![], pairs: vec![], layout: None, p: Params::default(), sel: 0, ref_choice: None, pairsel: 0, show_outliers: true,
            overlays: true, view: ViewKind::Image, vstate: View::default(), pano: None, crop: None,
            status: "Add images to begin (any order).".into(), pathbox: String::new(), tx, rx, busy: false, preview_pending: false,
            dialog: None, tex_cache: HashMap::new(), drag: None, simple: false, selected: Default::default(), note_after: None, undo: vec![], redo: vec![], committed: None, pending: None,
            project: None, pending_project: None, pano_version: 0, detail: None, detail_req: None, detail_busy: false,
        }
    }
}

fn center(img: &Img) -> Pt {
    [(img.w as f64 + 1.0) / 2.0, (img.h as f64 + 1.0) / 2.0]
}

/// Placement from the accepted pairs: spanning tree, reference image, transforms
/// and (optionally) the global refinement. Pairs that do not fit the refined
/// solution are rejected and the placement is redone (up to 3 times).
pub fn relayout(sizes: &[(usize, usize)], pairs: &mut [PairRes], ref_choice: Option<usize>, p: &Params) -> (Option<Placement>, String) {
    let n = sizes.len();
    if n == 0 {
        return (None, String::new());
    }
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
        let f = estimate_focal(&e.iter().map(|&k| (pairs[k].h, cs[pairs[k].i], cs[pairs[k].j])).collect::<Vec<_>>())
            .unwrap_or(sizes[ref_].0.max(sizes[ref_].1) as f64);
        let mut g = place(&hd, &parent, &order, ref_, &sizes.iter().map(|s| s.1).collect::<Vec<_>>());
        let (mut f, mut k1, mut typical) = (f, 0.0, None);
        if p.refine && p.model == Model::Homography && members.len() > 1 {
            let m: Vec<MatchSet> = e.iter().map(|&k| { let pr = &pairs[k]; (pr.i, pr.j, pr.inl.iter().map(|&q| pr.p1[q]).collect(), pr.inl.iter().map(|&q| pr.p2[q]).collect()) }).collect();
            let b = bundle_adjust(&g, &cs, ref_, f, &members, &m, 40, p.tol, 60, true);
            let fin: Vec<f64> = b.med.iter().copied().filter(|v| v.is_finite()).collect();
            let typ = if fin.is_empty() { 0.0 } else { crate::image::median_f64(fin) };
            typical = Some(typ);
            // Reject pairs far off the common solution, or clearly worse than typical while resting on few inliers.
            let weak = 2 * p.mininl;
            let bad: Vec<usize> = (0..e.len())
                .filter(|&q| b.med[q].is_finite() && (b.med[q] > (4.0 * p.tol).max(8.0) || (b.med[q] > (3.0 * typ).max(2.0 * p.tol) && pairs[e[q]].inl.len() < weak)))
                .collect();
            if pass < 3 && !bad.is_empty() {
                for &q in &bad {
                    let pr = &mut pairs[e[q]];
                    pr.ok = false;
                    pr.msg = format!("Pair {}–{} REJECTED by the global refinement: it is off by {:.1} px from all other pairs, so it is probably a false match (or something moved). Re-match it to try again.", pr.i + 1, pr.j + 1, b.med[q]);
                }
                nrej += bad.len();
                continue;
            }
            g = b.g;
            f = b.f;
            k1 = b.k1;
        }
        let g = park_unlinked(g, &parent, &order, &comp, ref_, &members, f, &cs, sizes[ref_]);
        let note = if nrej == 0 { String::new() } else { format!(" Rejected {nrej} pair(s) that did not fit the others.") };
        return (Some(Placement { parent, order, comp, ref_, members, g, f, lens: (k1, f), typical }), note);
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
    pub fn busy(&self) -> bool {
        self.busy || self.dialog.is_some()
    }

    fn spawn(&mut self, msg: &str, f: impl FnOnce(&Sender<Msg>) -> Result<Msg, String> + Send + 'static) {
        if self.busy {
            self.status = "Busy – wait for the current step to finish.".into();
            return;
        }
        self.busy = true;
        self.status = msg.into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&tx))).unwrap_or_else(|_| Err("internal error (panic)".into()));
            let _ = tx.send(r.unwrap_or_else(Msg::Error));
        });
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
        self.spawn(&format!("Loading {n} image(s)…"), move |tx| {
            let done = std::sync::atomic::AtomicUsize::new(0);
            let res: Vec<Result<Item, String>> = ps
                .par_iter()
                .map(|p| {
                    if p.is_empty() {
                        return Err(String::new());
                    }
                    let r = io::load_any(p, &opt).map(|img| {
                        let f = (DISP_MAX as f64 / img.w.max(img.h) as f64).min(1.0);
                        let disp = if f < 1.0 { resize(&img, f) } else { img.clone() };
                        Item { path: p.clone(), img: Arc::new(img), disp: Arc::new(disp), f, feats: None, featkey: None, gain: 1.0, offset: [0.0; 2] }
                    });
                    let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let _ = tx.send(Msg::Status(format!("Loading {k}/{n}: {}", basename(p))));
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
        self.status = format!("Removed {n} image(s); {} left.", self.items.len());
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
            self.status = "Add images first.".into();
            return;
        }
        let p = self.p.clone();
        let key = p.featkey();
        let todo: Vec<(usize, Arc<Img>)> = self.items.iter().enumerate().filter(|(_, it)| it.featkey != Some(key)).map(|(i, it)| (i, it.img.clone())).collect();
        self.spawn(&format!("Detecting features in {} image(s)…", todo.len()), move |_| {
            let feats: Vec<(usize, Arc<Features>)> = todo
                .par_iter()
                .map(|(i, img)| {
                    let dp = DetectParams { scale: (p.det_px as f64 / img.w.max(img.h) as f64).min(1.0), nkeypoints: p.nkp, threshold: p.thr, denoise: p.denoise, oriented: false };
                    (*i, Arc::new(detect(img, &dp)))
                })
                .collect();
            Ok(Msg::Detected(feats, key))
        });
    }

    /// Detect (where needed), match `ij` (all candidate pairs if None) and place, then optionally preview.
    fn match_pairs(&mut self, ij: Option<Vec<(usize, usize)>>, then_preview: bool) {
        if self.items.len() < 2 {
            self.status = "Add at least two images.".into();
            return;
        }
        let p = self.p.clone();
        let key = p.featkey();
        let imgs: Vec<(Arc<Img>, Option<Arc<Features>>)> = self.items.iter().map(|it| (it.img.clone(), if it.featkey == Some(key) { it.feats.clone() } else { None })).collect();
        let n = imgs.len();
        let given = ij.is_some();
        let ij = ij.unwrap_or_else(|| candidate_pairs(n, p.layout, p.cols, p.snake));
        let mut old: Vec<PairRes> = self.pairs.iter().filter(|q| !ij.contains(&(q.i, q.j))).cloned().collect();
        // Pairs whose images got new features are stale.
        let stale: Vec<usize> = (0..n).filter(|&i| imgs[i].1.is_none()).collect();
        old.retain(|q| !stale.contains(&q.i) && !stale.contains(&q.j));
        let ref_choice = self.ref_choice;
        let sizes: Vec<(usize, usize)> = self.items.iter().map(|it| (it.img.h, it.img.w)).collect();
        self.preview_pending = then_preview;
        let listed = given || p.layout != Layout::All;
        self.spawn("Detecting features…", move |tx| {
            let feats: Vec<Arc<Features>> = imgs
                .par_iter()
                .map(|(img, f)| f.clone().unwrap_or_else(|| {
                    let dp = DetectParams { scale: (p.det_px as f64 / img.w.max(img.h) as f64).min(1.0), nkeypoints: p.nkp, threshold: p.thr, denoise: p.denoise, oriented: false };
                    Arc::new(detect(img, &dp))
                }))
                .collect();
            let _ = tx.send(Msg::Status(format!("Matching {} pair(s)…", ij.len())));
            let mut res: Vec<PairRes> = ij
                .par_iter()
                .map(|&(i, j)| {
                    let (p1, p2) = match_pair(&feats[i], &feats[j], p.ratio, p.maxd);
                    let name = format!("Pair {}–{} ({})", i + 1, j + 1, p.model.name());
                    match ransac(&p2, &p1, p.model, 2000, p.tol) {
                        Ok((h, inl)) => {
                            let (ok, why) = accept_pair(p1.len(), inl.len(), &h, sizes[j].0, sizes[j].1, p.mininl);
                            let err = inl.iter().map(|&k| reproj_err(&h, p2[k], p1[k])).sum::<f64>() / inl.len().max(1) as f64;
                            let msg = format!("{name}: {} matches, {} inliers ({}%), mean error {err:.2} px{}", p1.len(), inl.len(), 100 * inl.len() / p1.len().max(1), if ok { String::new() } else { format!("\nREJECTED: {why}.") });
                            PairRes { i, j, p1, p2, h, inl, ok, listed: listed || ok, msg }
                        }
                        Err(e) => PairRes { i, j, p1, p2, h: M3::zeros(), inl: vec![], ok: false, listed, msg: format!("{name} FAILED: {e}") },
                    }
                })
                .collect();
            let nacc = res.iter().filter(|r| r.ok).count();
            let ntried = res.len();
            res.extend(old);
            res.sort_by_key(|r| (r.i, r.j));
            let _ = tx.send(Msg::Status(format!("Placing images{}", if p.refine { " and refining globally…" } else { "…" })));
            let (layout, note) = relayout(&sizes, &mut res, ref_choice, &p);
            let mut msg = format!("{nacc} of {ntried} pairs accepted.{note}");
            if let Some(l) = &layout {
                if let Some(t) = l.typical {
                    msg += &format!(" Typical pair error after refinement: {t:.1} px.");
                }
                let unl: Vec<String> = (0..n).filter(|&i| l.comp[i] != l.comp[l.ref_]).map(|i| (i + 1).to_string()).collect();
                msg += &if unl.is_empty() { " All images are linked.".to_string() } else { format!(" NOT linked: images {} – hidden; switch on “show unlinked images” to place them by hand, or try other matching settings.", unl.join(", ")) };
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
                        msg += " Tip: the panorama reaches far up/down; the spherical projection suits it better.";
                    }
                }
            }
            Ok(Msg::Matched { feats: feats.into_iter().enumerate().collect(), key, pairs: res, layout, msg })
        });
    }

    fn relayout_now(&mut self) -> String {
        let sizes: Vec<(usize, usize)> = self.items.iter().map(|it| (it.img.h, it.img.w)).collect();
        let (l, note) = relayout(&sizes, &mut self.pairs, self.ref_choice, &self.p);
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
        (0..self.items.len()).map(|i| vrot * nm[i] * l.g[i]).collect()
    }

    fn geometry(&self) -> Option<(f64, Pt, Vec<M3>, Vec<usize>, Lens)> {
        let l = self.layout.as_ref()?;
        let f = l.f * self.p.focal;
        let c = center(&self.items[l.ref_].img);
        Some((f, c, self.placements(f, c), self.visible(), l.lens))
    }

    fn request_preview(&mut self) {
        if self.busy {
            self.preview_pending = true;
            return;
        }
        let Some((f, c, g, vis, lens)) = self.geometry() else {
            self.status = "Match the images first.".into();
            return;
        };
        let p = self.p.clone();
        let outlines: Vec<Vec<Pt>> = (0..self.items.len()).map(|i| if vis.contains(&i) { outline(&g[i], self.items[i].img.h, self.items[i].img.w, p.proj, f, c, lens, 24) } else { vec![] }).collect();
        let full = bounds(&vis.iter().map(|&i| outlines[i].clone()).collect::<Vec<_>>());
        let box_ = self.crop.unwrap_or(full);
        let side = (box_.1 - box_.0).max(box_.3 - box_.2) + 1.0;
        let scale = (p.prev_px as f64 / side).min(1.0);
        let (imgs, scales, sizes, gains, gv) = self.compose_inputs(&vis, &g, scale);
        let uncropped = self.crop.is_none();
        self.spawn("Rendering preview…", move |_| {
            let refs: Vec<&Img> = imgs.iter().map(|a| a.as_ref()).collect();
            let cp = ComposeParams { scale, projection: p.proj, f, c, blend: p.blend, lens, sharpness: 3.0, deghost: 0.12 };
            let (img, cov) = compose(&refs, &gv, box_, &cp, &gains, &scales, &sizes, uncropped);
            Ok(Msg::Preview(img, box_, scale, outlines, cov))
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

    /// Re-render the visible part of the panorama at screen resolution (in the background).
    fn render_detail(&mut self, key: u64, box_: (f64, f64, f64, f64), scale: f64) {
        let Some((f, c, g, vis, lens)) = self.geometry() else { return };
        let (imgs, scales, sizes, gains, gv) = self.compose_inputs(&vis, &g, scale);
        let (proj, blend, tx) = (self.p.proj, self.p.blend, self.tx.clone());
        self.detail_busy = true;
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(|| {
                let refs: Vec<&Img> = imgs.iter().map(|a| a.as_ref()).collect();
                let cp = ComposeParams { scale, projection: proj, f, c, blend, lens, sharpness: 3.0, deghost: 0.12 };
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
        let items: Vec<(&str, [f64; 2], f64)> = self.items.iter().map(|i| (i.path.as_str(), i.offset, i.gain)).collect();
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
        let msg = format!("{}: {label}. ({} undo / {} redo steps left)", if redo { "Redone" } else { "Undone" }, self.undo.len(), self.redo.len());
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
        m.insert("projection".into(), self.p.proj.name().into());
        m.insert("blend".into(), self.p.blend.name().into());
        d.insert("menus".into(), Value::Table(m));
        let mut t = Table::new();
        t.insert("snake".into(), self.p.snake.into());
        t.insert("refine".into(), self.p.refine.into());
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
                Value::Table(t)
            })
            .collect();
        d.insert("pairs".into(), Value::Array(pairs));
        if let Some(c) = self.crop {
            d.insert("crop".into(), arr(vec![c.0, c.1, c.2, c.3]));
        }
        std::fs::write(&path, toml::to_string(&d).map_err(|e| e.to_string())?).map_err(|e| format!("Cannot save {path}: {e}"))?;
        self.project = Some(path.clone());
        Ok(path)
    }

    pub fn open_project(&mut self, path: &str) {
        if self.busy {
            self.status = "Busy – wait for the current step to finish.".into();
            return;
        }
        match self.parse_project(path) {
            Ok((paths, pd)) => {
                self.pending_project = Some(pd);
                self.items.clear();
                self.list_changed();
                self.add_paths_keep(paths);
            }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    fn parse_project(&mut self, path: &str) -> Result<(Vec<String>, ProjData), String> {
        use toml::Value;
        let text = std::fs::read_to_string(path).map_err(|e| format!("Project file not found: {path} ({e})"))?;
        let d: toml::Table = text.parse().map_err(|e| format!("Cannot read {}: {e}", basename(path)))?;
        if d.get("format").and_then(|v| v.as_str()) != Some("PanoStitch project") {
            return Err(format!("{} is not a PanoStitch project.", basename(path)));
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
        self.p.unlinked = tb("show unlinked images").unwrap_or(self.p.unlinked);
        self.overlays = tb("overlays").unwrap_or(self.overlays);
        // Find the images: stored absolute path, else relative to the project file.
        let recs = d.get("images").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let mut paths = Vec::new();
        let mut offsets = Vec::new();
        let mut missing = Vec::new();
        for r in &recs {
            let ap = r.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let rel = dir.join(r.get("relative").and_then(|v| v.as_str()).unwrap_or(""));
            let found = if std::path::Path::new(ap).is_file() { Some(ap.to_string()) } else if rel.is_file() { Some(rel.to_string_lossy().to_string()) } else { None };
            let off = r.get("offset").and_then(|v| v.as_array()).map(|a| [a.first().and_then(num).unwrap_or(0.0), a.get(1).and_then(num).unwrap_or(0.0)]).unwrap_or([0.0; 2]);
            offsets.push((off, r.get("gain").and_then(num).unwrap_or(1.0)));
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
                p1,
                p2,
                h,
                inl,
                ok: r.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false),
                listed: r.get("listed").and_then(|v| v.as_bool()).unwrap_or(true),
                msg: r.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            });
        }
        let reference = d.get("reference").and_then(|v| v.as_integer()).filter(|&r| r > 0).map(|r| r as usize - 1);
        let crop = d.get("crop").map(|v| fv(Some(v))).filter(|c| c.len() == 4).map(|c| (c[0], c[1], c[2], c[3]));
        Ok((paths, ProjData { path: path.to_string(), offsets, missing, pairs, reference, crop }))
    }

    /// Images of an opened project, placed as saved. `items[k]` belongs to record k.
    fn apply_project(&mut self, items: Vec<Option<Item>>, failed: Vec<String>, pd: ProjData) {
        let mut newidx = vec![None; items.len()];
        self.items.clear();
        for (k, it) in items.into_iter().enumerate() {
            if let Some(mut it) = it {
                (it.offset, it.gain) = pd.offsets[k];
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
        let failed: Vec<String> = failed.into_iter().filter(|f| !f.is_empty()).collect();
        self.status = format!(
            "Opened {}: {} images, {} pairs.{note}{}{}",
            basename(&pd.path),
            self.items.len(),
            self.pairs.iter().filter(|q| q.ok).count(),
            if pd.missing.is_empty() { String::new() } else { format!(" Missing files: {}.", pd.missing.join(", ")) },
            if failed.is_empty() { String::new() } else { format!(" Could not load: {}.", failed.join("; ")) }
        );
        if self.pairs.iter().any(|q| q.ok) {
            self.vstate.fit = true;
            self.note_after = Some(self.status.clone());
            self.request_preview();
        } else {
            self.view = ViewKind::Image;
        }
    }

    fn save_full(&mut self, out: String) {
        let Some((f, c, g, vis, lens)) = self.geometry() else { return };
        let p = self.p.clone();
        let o: Vec<Vec<Pt>> = vis.iter().map(|&i| outline(&g[i], self.items[i].img.h, self.items[i].img.w, p.proj, f, c, lens, 24)).collect();
        let box_ = self.crop.unwrap_or_else(|| bounds(&o));
        let (w, h) = (box_.1 - box_.0 + 1.0, box_.3 - box_.2 + 1.0);
        if w.max(h) > 40_000.0 {
            self.status = format!("The panorama would be {w}×{h} px; check the pairs or use another projection.");
            return;
        }
        let (imgs, _, sizes, gains, gv) = self.compose_inputs(&vis, &g, 1.0);
        let imgs: Vec<Arc<Img>> = imgs.iter().zip(&vis).map(|(_, &i)| self.items[i].img.clone()).collect();
        let cropped = self.crop.is_some();
        self.spawn("Rendering full-resolution panorama…", move |_| {
            let refs: Vec<&Img> = imgs.iter().map(|a| a.as_ref()).collect();
            let cp = ComposeParams { scale: 1.0, projection: p.proj, f, c, blend: p.blend, lens, sharpness: 3.0, deghost: 0.12 };
            let (img, _) = compose(&refs, &gv, box_, &cp, &gains, &vec![1.0; refs.len()], &sizes, false);
            let astro = io::is_astro(&out);
            io::save_panorama(&out, &if astro { img } else { look(&img, 1.0, &p) })?;
            Ok(Msg::Done(format!("Saved {out} ({w}×{h} px, {} images{}{}).", refs.len(), if cropped { ", cropped" } else { "" }, if astro { ", linear 32-bit" } else if p.stretched() { ", stretched as shown" } else { "" })))
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Status(s) => self.status = s,
                Msg::Error(e) => {
                    self.status = format!("Error: {e}");
                    self.busy = false;
                    self.preview_pending = false;
                    self.note_after = None;
                }
                Msg::Done(s) => {
                    self.status = s;
                    self.busy = false;
                }
                Msg::Detail(key, img, box_, scale) => {
                    self.detail_busy = false;
                    if self.pano.is_some() {
                        let tex = texture(ctx, "detail", &look(&img, 1.0, &self.p));
                        let tk = (self.p.black.to_bits(), self.p.mid.to_bits(), self.p.white.to_bits());
                        self.detail = Some(Detail { key, tex, tex_key: tk, img, box_, scale });
                    }
                }
                Msg::DetailFailed => self.detail_busy = false,
                Msg::Loaded(items, failed) if self.pending_project.is_some() => {
                    self.busy = false;
                    let pd = self.pending_project.take().unwrap();
                    self.apply_project(items, failed, pd);
                }
                Msg::Loaded(items, failed) => {
                    self.busy = false;
                    let items: Vec<Item> = items.into_iter().flatten().collect();
                    let failed: Vec<String> = failed.into_iter().filter(|f| !f.is_empty()).collect();
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
                        note = " Auto stretch applied for viewing (FITS / XISF data is linear).".into();
                    }
                    self.status = format!("Loaded {n} image(s). Next: Run all steps (or detect, match, preview).{note}{}", if failed.is_empty() { String::new() } else { format!("\nSkipped {}:\n{}", failed.len(), failed.join("\n")) });
                }
                Msg::Detected(feats, key) => {
                    self.busy = false;
                    let changed: Vec<usize> = feats.iter().map(|f| f.0).collect();
                    for (i, f) in feats {
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
                    self.status = "Features detected. Look for keypoints spread over the overlap areas; if too few, lower the corner threshold or raise Denoise. Then match.".into();
                }
                Msg::Matched { feats, key, pairs, layout, msg } => {
                    self.busy = false;
                    for (i, f) in feats {
                        if let Some(it) = self.items.get_mut(i) {
                            it.feats = Some(f);
                            it.featkey = Some(key);
                        }
                    }
                    self.pairs = pairs;
                    self.layout = layout;
                    self.crop = None;
                    self.status = format!("{msg} Inspect pairs in the Pair menu, then preview.");
                    if self.preview_pending {
                        self.note_after = Some(format!("Done. {msg}"));
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
                    self.pano = Some(Preview { img, tex: None, tex_key: (0, 0, 0, 0), box_, scale, outlines, covered });
                    self.view = ViewKind::Pano;
                    if fit {
                        self.vstate.fit = true;
                    }
                    self.status = self.note_after.take().unwrap_or_else(|| "Preview ready. Drag to pan, wheel to zoom (detail sharpens). Click an image to select it; Ctrl+drag or arrow keys move it.".into());
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
                        Ok(p) => self.status = format!("Saved project {p}."),
                        Err(e) => self.status = format!("Error: {e}"),
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
        let key = (self.items[i].path.clone(), self.p.ev.to_bits() ^ self.p.black.to_bits().rotate_left(7) ^ self.p.mid.to_bits().rotate_left(13) ^ self.p.white.to_bits().rotate_left(19));
        if let Some(t) = self.tex_cache.get(&key) {
            return t.clone();
        }
        if self.tex_cache.len() > 64 {
            self.tex_cache.clear();
        }
        let t = texture(ctx, &key.0, &look(&self.items[i].disp, 2f64.powf(self.p.ev), &self.p));
        self.tex_cache.insert(key, t.clone());
        t
    }

    fn nudge(&mut self, dx: f64, dy: f64) {
        let Some(l) = &self.layout else { return };
        if self.sel == l.ref_ {
            self.status = "The reference image stays fixed; select another image to nudge (or use Pitch / Roll).".into();
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
        egui::Panel::left("stitch_left").exact_size(390.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("sl").show(ui, |ui| self.left_panel(ui));
        });
        egui::Panel::right("stitch_right").exact_size(330.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("sr").show(ui, |ui| redo_preview = self.right_panel(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.center(ui, ctx));
        if redo_preview && self.pano.is_some() {
            self.request_preview();
        }
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("⇪ Open project…").clicked() && self.dialog.is_none() {
                self.dialog = Some((dialogs::pick_open("Open project", "PanoStitch projects", PROJECT_EXT), "openproj"));
            }
            if ui.button("⬇ Save project…").clicked() && self.dialog.is_none() {
                if self.items.is_empty() {
                    self.status = "Nothing to save yet.".into();
                } else {
                    let default = self.project.clone().unwrap_or_else(|| {
                        std::path::Path::new(&self.items[0].path).parent().unwrap_or(std::path::Path::new(".")).join(format!("project{PROJECT_EXT}")).to_string_lossy().to_string()
                    });
                    self.dialog = Some((dialogs::pick_save("Save project", &default), "saveproj"));
                }
            }
        });
        ui.heading("Images");
        ui.horizontal(|ui| {
            if ui.button("Add…").clicked() && self.dialog.is_none() {
                self.dialog = Some((dialogs::pick_images(), "add"));
            }
            if ui.button("Remove").clicked() {
                self.remove_selected();
            }
            if ui.button(if self.simple { "Clear" } else { "Clear all" }).clicked() && !self.busy {
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
        let r = ui.add(egui::TextEdit::singleline(&mut self.pathbox).hint_text("…or type a file/folder path + Enter").desired_width(370.0));
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
                ui.weak("(no images)");
            }
            let refi = self.layout.as_ref().map(|l| l.ref_);
            for (i, it) in self.items.iter().enumerate() {
                let kp = it.feats.as_ref().map(|f| format!(" {}kp", f.pts.len())).unwrap_or_default();
                let unl = self.layout.as_ref().is_some_and(|l| self.pairs.iter().any(|p| p.ok) && l.comp[i] != l.comp[l.ref_]);
                let txt = format!("{}{:>3} {}{kp}{}", if Some(i) == refi { "R" } else { " " }, i + 1, basename(&it.path), if unl { "  UNLINKED" } else { "" });
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
            ui.label("Reference");
            let names: Vec<String> = self.items.iter().enumerate().map(|(i, it)| format!("{}: {}", i + 1, basename(&it.path))).collect();
            let cur = self.ref_choice.map(|r| names.get(r).cloned().unwrap_or_default()).unwrap_or_else(|| "auto (centre)".into());
            let before = self.ref_choice;
            egui::ComboBox::from_id_salt("ref").selected_text(cur).width(250.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.ref_choice, None, "auto (centre)");
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut self.ref_choice, Some(i), n);
                }
            });
            ui.end_row();
            if before != self.ref_choice && !self.pairs.is_empty() {
                let note = self.relayout_now();
                self.status = format!("Reference changed.{note}");
                if self.pano.is_some() {
                    self.request_preview();
                }
            }
            let mut decs = vec![RawDecoder::Auto];
            decs.extend(io::available_raw_decoders());
            combo(ui, "RAW via", &mut self.p.decoder, &decs, |d| d.name());
            ui.label("");
            ui.checkbox(&mut self.p.half, "RAW at half size");
            ui.end_row();
            }
            ui.label(egui::RichText::new("Preset").strong());
            egui::ComboBox::from_id_salt("preset").selected_text("choose…").show_ui(ui, |ui| {
                for name in PRESETS {
                    if ui.selectable_label(false, name).clicked() {
                        let note = self.p.preset(name);
                        self.status = format!("Preset “{name}”: {note} Next: Run all steps.");
                    }
                }
            });
            ui.end_row();
        });
        ui.separator();
        if self.simple {
            if ui.add(egui::Button::new(egui::RichText::new("▶ Run all steps").strong())).clicked() {
                self.match_pairs(None, true);
            }
            return;
        }
        ui.heading("1 · Features");
        egui::Grid::new("feat").num_columns(2).show(ui, |ui| {
            slider(ui, "Detection size (px)", &mut self.p.det_px, 500..=6000, 100.0);
            slider(ui, "Max keypoints", &mut self.p.nkp, 500..=15000, 500.0);
            slider(ui, "Corner threshold", &mut self.p.thr, 0.0..=0.02, 0.0001);
            slider(ui, "Denoise (σ px)", &mut self.p.denoise, 0.0..=4.0, 0.05);
        });
        ui.horizontal(|ui| {
            if ui.button("Detect features").clicked() {
                self.detect_only();
            }
            if ui.add(egui::Button::new(egui::RichText::new("▶ Run all steps").strong())).clicked() {
                self.match_pairs(None, true);
            }
        });
        ui.separator();
        ui.heading("2 · Matching");
        egui::Grid::new("match").num_columns(2).show(ui, |ui| {
            combo(ui, "Pairs", &mut self.p.layout, &[Layout::All, Layout::Sequential, Layout::Grid], |l| match l { Layout::All => "all pairs", Layout::Sequential => "sequential", Layout::Grid => "grid" });
            if self.p.layout == Layout::Grid {
                slider(ui, "Grid columns", &mut self.p.cols, 1..=60, 1.0);
                ui.label("");
                ui.checkbox(&mut self.p.snake, "snake (every 2nd row reversed)");
                ui.end_row();
            }
            slider(ui, "Ratio test", &mut self.p.ratio, 0.5..=0.95, 0.005);
            slider(ui, "Max distance", &mut self.p.maxd, 0.1..=0.5, 0.005);
            slider(ui, "RANSAC tol. (px)", &mut self.p.tol, 0.5..=20.0, 0.1);
            slider(ui, "Min. inliers", &mut self.p.mininl, 6..=100, 1.0);
            combo(ui, "Model", &mut self.p.model, &Model::ALL, |m| m.name());
            ui.label("");
            if ui.checkbox(&mut self.p.refine, "global refinement").changed() && !self.pairs.is_empty() {
                let note = self.relayout_now();
                self.status = format!("Global refinement {}.{note}", if self.p.refine { "on" } else { "off" });
            }
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("Match").clicked() {
                self.match_pairs(None, false);
            }
            let cur = self.pairs.get(self.pairsel).map(|q| (q.i, q.j));
            if ui.button("Re-match pair").clicked() {
                match cur {
                    Some(ij) => self.match_pairs(Some(vec![ij]), false),
                    None => self.status = "Choose a pair first.".into(),
                }
            }
            if ui.button("Discard pair").clicked() && !self.busy {
                if let Some(q) = self.pairs.get_mut(self.pairsel) {
                    q.ok = false;
                    q.msg = "Discarded by you. Use Re-match pair to try it again.".into();
                    let note = self.relayout_now();
                    self.status = format!("Pair discarded; placement updated.{note}");
                    if self.pano.is_some() {
                        self.request_preview();
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Pair");
            let label = |q: &PairRes| format!("{}–{}  {}", q.i + 1, q.j + 1, if q.ok { format!("{} inl", q.inl.len()) } else { "rejected".into() });
            let cur = self.pairs.get(self.pairsel).filter(|q| q.listed).map(label).unwrap_or_else(|| "(none)".into());
            egui::ComboBox::from_id_salt("pair").selected_text(cur).width(150.0).show_ui(ui, |ui| {
                for (k, q) in self.pairs.iter().enumerate().filter(|(_, q)| q.listed) {
                    if ui.selectable_label(k == self.pairsel, label(q)).clicked() {
                        self.pairsel = k;
                        self.view = ViewKind::Pair;
                        self.vstate.fit = true;
                    }
                }
            });
            ui.checkbox(&mut self.show_outliers, "outliers");
        });
        if let Some(q) = self.pairs.get(self.pairsel) {
            ui.label(egui::RichText::new(&q.msg).small());
        }
    }

    /// Returns whether the preview should be re-rendered.
    fn right_panel(&mut self, ui: &mut egui::Ui) -> bool {
        let mut redo = false;
        ui.heading("3 · Projection");
        egui::Grid::new("proj").num_columns(2).show(ui, |ui| {
            if combo(ui, "Projection", &mut self.p.proj, &Projection::ALL, |p| p.name()) {
                self.crop = None;
                self.vstate.fit = true;
                redo = true;
            }
            redo |= slider(ui, "Focal (× auto)", &mut self.p.focal, 0.3..=3.0, 0.005);
            redo |= slider(ui, "Yaw (°)", &mut self.p.yaw, -180.0..=180.0, 0.1);
            redo |= slider(ui, "Pitch (°)", &mut self.p.pitch, -90.0..=90.0, 0.1);
            redo |= slider(ui, "Roll (°)", &mut self.p.roll, -45.0..=45.0, 0.05);
        });
        if ui.button("Reset focal / yaw / pitch / roll").clicked() {
            (self.p.focal, self.p.yaw, self.p.pitch, self.p.roll) = (1.0, 0.0, 0.0, 0.0);
            redo = true;
        }
        if redo {
            self.crop = None;
        }
        ui.separator();
        ui.heading("4 · Compose");
        let simple = self.simple;
        egui::Grid::new("comp").num_columns(2).show(ui, |ui| {
            if !simple {
                redo |= combo(ui, "Blend", &mut self.p.blend, &Blend::ALL, |b| b.name());
                slider(ui, "Preview size (px)", &mut self.p.prev_px, 500..=4000, 100.0);
            }
            redo |= slider(ui, "Exposure (EV)", &mut self.p.ev, -3.0..=8.0, 0.05);
            if !simple && self.sel < self.items.len() {
                // Brightness applies to all selected images.
                let mut g = self.items[self.sel].gain;
                if slider(ui, "Brightness (sel.)", &mut g, 0.3..=3.0, 0.005) {
                    for i in self.targets() {
                        self.items[i].gain = g;
                    }
                    redo = true;
                }
            }
        });
        if !simple && ui.checkbox(&mut self.p.unlinked, "show unlinked images").changed() {
            self.crop = None;
            redo = true;
        }
        ui.horizontal(|ui| {
            if ui.button("Auto exposure").clicked() {
                // Middle grey for the typical brightness, without clipping more than the brightest 0.5 %.
                match &self.pano {
                    Some(p) => {
                        let mut lum: Vec<f32> = p.img.px.iter().map(|c| luminance(*c)).filter(|l| *l > 0.0).collect();
                        if !lum.is_empty() {
                            let q = quantiles(&mut lum, &[0.5, 0.995]);
                            let d = ((0.18 / q[0] as f64).log2()).min((1.0 / q[1] as f64).log2()).clamp(-6.0, 10.0);
                            self.p.ev = (self.p.ev + d).clamp(-3.0, 8.0);
                            self.status = format!("Exposure set to {:.1} EV.", self.p.ev);
                            redo = true;
                        }
                    }
                    None => self.status = "Preview the panorama first.".into(),
                }
            }
            if ui.button("Even out").clicked() {
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
                    _ => self.status = "Match the images first.".into(),
                }
            }
            if ui.button("Reset moves").clicked() {
                self.items.iter_mut().for_each(|it| it.offset = [0.0; 2]);
                redo = true;
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Auto crop").clicked() {
                self.auto_crop();
            }
            if ui.button("Uncrop").clicked() && self.crop.is_some() {
                self.crop = None;
                self.vstate.fit = true;
                redo = true;
            }
        });
        if !simple {
        ui.horizontal(|ui| {
            ui.label("Nudge selected:");
            for (l, d) in [("←", (-1.0, 0.0)), ("↑", (0.0, -1.0)), ("↓", (0.0, 1.0)), ("→", (1.0, 0.0))] {
                if ui.button(l).clicked() {
                    self.nudge(d.0, d.1);
                }
            }
        });
        ui.separator();
        ui.heading("5 · Stretch");
        if let Some(src) = self.hist_source() {
            histogram(ui, &src, Some((self.p.black, self.p.mid, self.p.white)));
        }
        egui::Grid::new("str").num_columns(2).show(ui, |ui| {
            slider(ui, "Black point", &mut self.p.black, 0.0..=0.5, 0.0005);
            slider(ui, "Midtones", &mut self.p.mid, 0.0005..=0.9995, 0.0005);
            slider(ui, "White point", &mut self.p.white, 0.5..=1.0, 0.0005);
        });
        ui.horizontal(|ui| {
            if ui.button("Auto stretch").clicked() {
                if let Some(src) = self.hist_source() {
                    (self.p.black, self.p.mid, self.p.white) = auto_stretch(&src, 0.25, 2.8);
                }
            }
            if ui.button("Reset stretch").clicked() {
                (self.p.black, self.p.mid, self.p.white) = (0.0, 0.5, 1.0);
            }
        });
        }
        ui.separator();
        let big = |t: &str| egui::Button::new(egui::RichText::new(t).strong().size(15.0)).min_size(egui::vec2(310.0, 36.0));
        if ui.add(big("◐ Preview panorama")).clicked() {
            self.request_preview();
        }
        if ui.add(big("⬇ Save full resolution…")).clicked() && self.layout.is_some() && self.dialog.is_none() {
            let ext = if self.items.iter().all(|it| io::is_fits(&it.path)) { "fits" } else if self.items.iter().all(|it| io::is_astro(&it.path)) { "xisf" } else { "tif" };
            let dir = std::path::Path::new(&self.items[0].path).parent().map(|d| d.to_path_buf()).unwrap_or_default();
            let default = dir.join(format!("panorama.{ext}")).to_string_lossy().to_string();
            self.dialog = Some((dialogs::pick_save("Save panorama", &default), "save"));
        }
        redo
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
            self.status = "Preview the panorama first.".into();
            return;
        };
        let Some(cov) = &p.covered else {
            self.status = "Uncrop and preview first.".into();
            return;
        };
        let Some(((r0, r1), (c0, c1))) = largest_rectangle(cov) else {
            self.status = "Nothing to crop.".into();
            return;
        };
        // Output coordinates, one preview pixel inside on every side.
        let pu = |c: f64| (c - 0.5) / p.scale + p.box_.0 - 0.5;
        let pv = |r: f64| (r - 0.5) / p.scale + p.box_.2 - 0.5;
        let b = (pu(c0 as f64 + 2.0).ceil(), pu(c1 as f64).floor(), pv(r0 as f64 + 2.0).ceil(), pv(r1 as f64).floor());
        if b.1 <= b.0 || b.3 <= b.2 {
            self.status = "The covered area is too small to crop.".into();
            return;
        }
        let full = (p.box_.1 - p.box_.0 + 1.0) * (p.box_.3 - p.box_.2 + 1.0);
        let area = (b.1 - b.0 + 1.0) * (b.3 - b.2 + 1.0);
        self.crop = Some(b);
        self.vstate.fit = true;
        self.note_after = Some(format!("Cropped to {}×{} px ({} % of the panorama). The crop is used when saving; it is removed when you change the geometry.", b.1 - b.0 + 1.0, b.3 - b.2 + 1.0, (100.0 * area / full).round()));
        self.request_preview();
    }

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            for (k, l) in [(ViewKind::Image, "Image + keypoints"), (ViewKind::Pair, "Pair matches"), (ViewKind::Pano, "Panorama")] {
                if self.simple && k == ViewKind::Pair {
                    continue;
                }
                if ui.selectable_label(self.view == k, l).clicked() {
                    self.view = k;
                    self.vstate.fit = true;
                }
            }
            if ui.button("Zoom +").clicked() {
                super::zoom_by(&mut self.vstate, 1.5);
            }
            if ui.button("Zoom -").clicked() {
                super::zoom_by(&mut self.vstate, 1.0 / 1.5);
            }
            if ui.button("Fit").clicked() {
                self.vstate.fit = true;
            }
            ui.checkbox(&mut self.overlays, "overlays");
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Undo")).on_hover_text(self.undo.last().map(|u| u.1.clone()).unwrap_or_default()).clicked() {
                self.step(false);
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Redo")).on_hover_text(self.redo.last().map(|u| u.1.clone()).unwrap_or_default()).clicked() {
                self.step(true);
            }
        });
        let status_h = 60.0;
        let avail = ui.available_size();
        ui.allocate_ui(egui::vec2(avail.x, (avail.y - status_h).max(100.0)), |ui| match self.view {
            ViewKind::Image => self.show_image(ui, ctx),
            ViewKind::Pair => self.show_pair(ui, ctx),
            ViewKind::Pano => self.show_pano(ui, ctx),
        });
        let col = if ui.visuals().dark_mode { Color32::from_rgb(255, 160, 128) } else { Color32::from_rgb(140, 0, 0) };
        egui::ScrollArea::vertical().id_salt("status").show(ui, |ui| ui.colored_label(col, &self.status));
        // Keys: arrows nudge in the panorama view, +/-/0 zoom.
        if !ctx.egui_wants_keyboard_input() {
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
            ui.centered_and_justified(|ui| ui.weak("Add images to begin."));
            return;
        }
        let i = self.sel.min(self.items.len() - 1);
        let tex = self.tex_for(ctx, i);
        let it = &self.items[i];
        let (w, h) = (it.img.w as f64, it.img.h as f64);
        let kp = it.feats.as_ref().map(|f| format!("{} keypoints", f.pts.len())).unwrap_or_else(|| "no features yet".into());
        ui.label(format!("Image {}: {}  ({}×{}, {kp})", i + 1, basename(&it.path), w, h));
        let s = image_view(ui, &mut self.vstate, (w, h), &[(&tex, [0.5, w + 0.5, 0.5, h + 0.5])], true);
        if self.overlays {
            if let Some(f) = &it.feats {
                let p = ui.painter().with_clip_rect(s.rect);
                for q in &f.pts {
                    p.circle_stroke(s.to_screen(q[0], q[1]), 4.0, Stroke::new(1.2, Color32::YELLOW));
                }
            }
        }
    }

    fn show_pair(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(q) = self.pairs.get(self.pairsel).cloned() else {
            ui.centered_and_justified(|ui| ui.weak("No pairs matched yet."));
            return;
        };
        let (ta, tb) = (self.tex_for(ctx, q.i), self.tex_for(ctx, q.j));
        let (a, b) = (&self.items[q.i].img, &self.items[q.j].img);
        let (wa, ha, wb, hb) = (a.w as f64, a.h as f64, b.w as f64, b.h as f64);
        let xo = wa * 1.03;
        ui.label(format!("Pair {}–{}: {} matches, {} inliers (green){}{}", q.i + 1, q.j + 1, q.p1.len(), q.inl.len(), if self.show_outliers { ", outliers red" } else { "" }, if q.ok { "" } else { " · NOT USED" }));
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
        let Some(pv) = &mut self.pano else {
            ui.centered_and_justified(|ui| ui.weak("Panorama: press “Preview panorama”."));
            return;
        };
        let key = (self.p.black.to_bits(), self.p.mid.to_bits(), self.p.white.to_bits(), 1);
        if pv.tex.is_none() || pv.tex_key != key {
            pv.tex = Some(texture(ctx, "pano", &look(&pv.img, 1.0, &self.p)));
            pv.tex_key = key;
        }
        let tex = pv.tex.clone().unwrap();
        let b = pv.box_;
        let (u0, u1) = (b.0 - 0.5, pv.img.w as f64 / pv.scale + b.0 - 0.5);
        let (v0, v1) = (b.2 - 0.5, pv.img.h as f64 / pv.scale + b.2 - 0.5);
        if let Some(l) = &self.layout {
            let w = self.items[l.ref_].img.w as f64;
            let fov = 2.0 * (w / 2.0 / (l.f * self.p.focal)).atan().to_degrees();
            ui.label(format!(
                "{}: {}×{} px · focal {:.0} px (ref. FOV {fov:.1}°){}{}",
                self.p.proj.name(), b.1 - b.0 + 1.0, b.3 - b.2 + 1.0, l.f * self.p.focal,
                l.typical.map(|t| format!(" · typical error {t:.1} px")).unwrap_or_default(),
                if self.overlays { " · orange = selected, cyan = reference, yellow = links" } else { "" }
            ));
        }
        // Image coordinates of this view = output (u, v) coordinates.
        let mut vs = self.vstate;
        let s = image_view(ui, &mut vs, (u1 - u0, v1 - v0), &[], !ui.input(|i| i.modifiers.ctrl));
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
            let tk = (self.p.black.to_bits(), self.p.mid.to_bits(), self.p.white.to_bits());
            if d.tex_key != tk {
                d.tex = texture(ctx, "detail", &look(&d.img, 1.0, &self.p));
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
                    p.text(s.to_screen(m[0], m[1]), egui::Align2::CENTER_CENTER, if i == refi { format!("{} (ref)", i + 1) } else { (i + 1).to_string() }, egui::FontId::proportional(22.0), col);
                }
            }
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
                        self.status = "The reference image stays fixed; drag another image (or use Pitch / Roll).".into();
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

/// Histogram (log scale) with black / white point markers and the transfer curve.
pub fn histogram(ui: &mut egui::Ui, img: &Img, stretch: Option<(f64, f64, f64)>) {
    let hc = lum_histogram(img, 256);
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
    pub fn t_pano_size(&self) -> Option<(usize, usize)> {
        self.pano.as_ref().map(|p| (p.img.w, p.img.h))
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
}

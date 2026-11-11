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

#[derive(Clone)]
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
    Loaded(Vec<Item>, Vec<String>),
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
}

impl Default for StitchTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            items: vec![], pairs: vec![], layout: None, p: Params::default(), sel: 0, ref_choice: None, pairsel: 0, show_outliers: true,
            overlays: true, view: ViewKind::Image, vstate: View::default(), pano: None, crop: None,
            status: "Add images to begin (any order).".into(), pathbox: String::new(), tx, rx, busy: false, preview_pending: false,
            dialog: None, tex_cache: HashMap::new(), drag: None,
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
        if ps.is_empty() {
            return;
        }
        let opt = LoadOptions { decoder: self.p.decoder, half: self.p.half, ..Default::default() };
        let n = ps.len();
        self.spawn(&format!("Loading {n} image(s)…"), move |tx| {
            let done = std::sync::atomic::AtomicUsize::new(0);
            let res: Vec<Result<Item, String>> = ps
                .par_iter()
                .map(|p| {
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
                    Ok(i) => items.push(i),
                    Err(e) => failed.push(e),
                }
            }
            Ok(Msg::Loaded(items, failed))
        });
    }

    fn list_changed(&mut self) {
        self.pairs.clear();
        self.layout = None;
        self.pano = None;
        self.crop = None;
        self.ref_choice = self.ref_choice.filter(|&r| r < self.items.len());
        self.sel = self.sel.min(self.items.len().saturating_sub(1));
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
        let ij = ij.unwrap_or_else(|| candidate_pairs(n, p.layout, p.cols, p.snake));
        let mut old: Vec<PairRes> = self.pairs.iter().filter(|q| !ij.contains(&(q.i, q.j))).cloned().collect();
        // Pairs whose images got new features are stale.
        let stale: Vec<usize> = (0..n).filter(|&i| imgs[i].1.is_none()).collect();
        old.retain(|q| !stale.contains(&q.i) && !stale.contains(&q.j));
        let ref_choice = self.ref_choice;
        let sizes: Vec<(usize, usize)> = self.items.iter().map(|it| (it.img.h, it.img.w)).collect();
        self.preview_pending = then_preview;
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
                            PairRes { i, j, p1, p2, h, inl, ok, msg }
                        }
                        Err(e) => PairRes { i, j, p1, p2, h: M3::zeros(), inl: vec![], ok: false, msg: format!("{name} FAILED: {e}") },
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
        let use_disp = vis.iter().all(|&i| scale <= self.items[i].f);
        let imgs: Vec<Arc<Img>> = vis.iter().map(|&i| if use_disp { self.items[i].disp.clone() } else { self.items[i].img.clone() }).collect();
        let scales: Vec<f64> = vis.iter().map(|&i| if use_disp { self.items[i].f } else { 1.0 }).collect();
        let sizes: Vec<(usize, usize)> = vis.iter().map(|&i| (self.items[i].img.h, self.items[i].img.w)).collect();
        let gains: Vec<f64> = vis.iter().map(|&i| self.items[i].gain * 2f64.powf(p.ev)).collect();
        let gv: Vec<M3> = vis.iter().map(|&i| g[i]).collect();
        let uncropped = self.crop.is_none();
        self.spawn("Rendering preview…", move |_| {
            let refs: Vec<&Img> = imgs.iter().map(|a| a.as_ref()).collect();
            let cp = ComposeParams { scale, projection: p.proj, f, c, blend: p.blend, lens, sharpness: 3.0, deghost: 0.12 };
            let (img, cov) = compose(&refs, &gv, box_, &cp, &gains, &scales, &sizes, uncropped);
            Ok(Msg::Preview(img, box_, scale, outlines, cov))
        });
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
        let imgs: Vec<Arc<Img>> = vis.iter().map(|&i| self.items[i].img.clone()).collect();
        let sizes: Vec<(usize, usize)> = imgs.iter().map(|i| (i.h, i.w)).collect();
        let gains: Vec<f64> = vis.iter().map(|&i| self.items[i].gain * 2f64.powf(p.ev)).collect();
        let gv: Vec<M3> = vis.iter().map(|&i| g[i]).collect();
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

    fn poll(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Status(s) => self.status = s,
                Msg::Error(e) => {
                    self.status = format!("Error: {e}");
                    self.busy = false;
                    self.preview_pending = false;
                }
                Msg::Done(s) => {
                    self.status = s;
                    self.busy = false;
                }
                Msg::Loaded(items, failed) => {
                    self.busy = false;
                    let astro = items.iter().any(|it| io::is_astro(&it.path));
                    let n = items.len();
                    self.items.extend(items);
                    self.sel = self.items.len().saturating_sub(1);
                    self.list_changed();
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
                    if !self.preview_pending {
                        self.view = ViewKind::Pair;
                        self.vstate.fit = true;
                    }
                }
                Msg::Preview(img, box_, scale, outlines, covered) => {
                    self.busy = false;
                    let fit = self.view != ViewKind::Pano || self.pano.is_none();
                    self.pano = Some(Preview { img, tex: None, tex_key: (0, 0, 0, 0), box_, scale, outlines, covered });
                    self.view = ViewKind::Pano;
                    if fit {
                        self.vstate.fit = true;
                    }
                    self.status = "Preview ready. Drag to pan, wheel to zoom. Click an image to select it; Ctrl+drag or arrow keys move it.".into();
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
                    Picked::Files(ps) => self.add_paths(ps),
                    Picked::Save(Some(p)) if what == "save" => self.save_full(p),
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
        self.poll();
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
        ui.heading("Images");
        ui.horizontal(|ui| {
            if ui.button("Add…").clicked() && self.dialog.is_none() {
                self.dialog = Some((dialogs::pick_images(), "add"));
            }
            if ui.button("Remove").clicked() && !self.busy && self.sel < self.items.len() {
                self.items.remove(self.sel);
                self.list_changed();
            }
            if ui.button("Clear").clicked() && !self.busy {
                self.items.clear();
                self.list_changed();
            }
            if ui.button("▲").clicked() && !self.busy && self.sel > 0 {
                self.items.swap(self.sel, self.sel - 1);
                self.sel -= 1;
                self.list_changed();
            }
            if ui.button("▼").clicked() && !self.busy && self.sel + 1 < self.items.len() {
                self.items.swap(self.sel, self.sel + 1);
                self.sel += 1;
                self.list_changed();
            }
        });
        let r = ui.add(egui::TextEdit::singleline(&mut self.pathbox).hint_text("…or type a file/folder path + Enter").desired_width(370.0));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.pathbox.trim().is_empty() {
            let ps = dialogs::expand_paths(&self.pathbox);
            self.pathbox.clear();
            self.add_paths(ps);
        }
        egui::ScrollArea::vertical().id_salt("list").max_height(160.0).show(ui, |ui| {
            if self.items.is_empty() {
                ui.weak("(no images)");
            }
            let refi = self.layout.as_ref().map(|l| l.ref_);
            for (i, it) in self.items.iter().enumerate() {
                let kp = it.feats.as_ref().map(|f| format!(" {}kp", f.pts.len())).unwrap_or_default();
                let unl = self.layout.as_ref().is_some_and(|l| self.pairs.iter().any(|p| p.ok) && l.comp[i] != l.comp[l.ref_]);
                let txt = format!("{}{:>3} {}{kp}{}", if Some(i) == refi { "R" } else { " " }, i + 1, basename(&it.path), if unl { "  UNLINKED" } else { "" });
                if ui.selectable_label(i == self.sel, egui::RichText::new(txt).monospace()).clicked() {
                    self.sel = i;
                    if self.view != ViewKind::Pano {
                        self.view = ViewKind::Image;
                        self.vstate.fit = true;
                    }
                }
            }
        });
        egui::Grid::new("load").num_columns(2).show(ui, |ui| {
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
        ui.heading("1 · Features");
        egui::Grid::new("feat").num_columns(2).show(ui, |ui| {
            slider(ui, "Detection size (px)", &mut self.p.det_px, 500..=6000, 100.0);
            slider(ui, "Max keypoints", &mut self.p.nkp, 500..=15000, 500.0);
            slider(ui, "Corner threshold", &mut self.p.thr, 0.0..=0.02, 0.0001);
            slider(ui, "Denoise (σ px)", &mut self.p.denoise, 0.0..=4.0, 0.05);
        });
        ui.horizontal(|ui| {
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
            let cur = self.pairs.get(self.pairsel).map(label).unwrap_or_else(|| "(none)".into());
            egui::ComboBox::from_id_salt("pair").selected_text(cur).width(150.0).show_ui(ui, |ui| {
                for (k, q) in self.pairs.iter().enumerate().filter(|(_, q)| q.ok || !q.p1.is_empty()) {
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
        egui::Grid::new("comp").num_columns(2).show(ui, |ui| {
            redo |= combo(ui, "Blend", &mut self.p.blend, &Blend::ALL, |b| b.name());
            slider(ui, "Preview size (px)", &mut self.p.prev_px, 500..=4000, 100.0);
            redo |= slider(ui, "Exposure (EV)", &mut self.p.ev, -3.0..=8.0, 0.05);
            if let Some(it) = self.items.get_mut(self.sel) {
                redo |= slider(ui, "Brightness (sel.)", &mut it.gain, 0.3..=3.0, 0.005);
            }
        });
        if ui.checkbox(&mut self.p.unlinked, "show unlinked images").changed() {
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
        self.status = format!("Cropped to {}×{} px ({} % of the panorama). The crop is used when saving; it is removed when you change the geometry.", b.1 - b.0 + 1.0, b.3 - b.2 + 1.0, (100.0 * area / full).round());
        self.request_preview();
    }

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            for (k, l) in [(ViewKind::Image, "Image + keypoints"), (ViewKind::Pair, "Pair matches"), (ViewKind::Pano, "Panorama")] {
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
            let (keys, shift) = ui.input(|i| {
                let k: Vec<egui::Key> = [egui::Key::ArrowLeft, egui::Key::ArrowRight, egui::Key::ArrowUp, egui::Key::ArrowDown, egui::Key::Plus, egui::Key::Equals, egui::Key::Minus, egui::Key::Num0]
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
                    egui::Key::Plus | egui::Key::Equals => super::zoom_by(&mut self.vstate, 1.5),
                    egui::Key::Minus => super::zoom_by(&mut self.vstate, 1.0 / 1.5),
                    egui::Key::Num0 => self.vstate.fit = true,
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
        p.image(tex.id(), egui::Rect::from_min_max(s.to_screen(u0, v0), s.to_screen(u1, v1)), egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
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
                let col = if i == self.sel { Color32::from_rgb(255, 165, 0) } else if i == refi { Color32::from_rgb(0, 255, 255) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 180) };
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

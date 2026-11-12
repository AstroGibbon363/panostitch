//! The "Star stacking" tab, modelled on Sequator: input files on the top left,
//! the processing settings below; on the right the post-processing (applied
//! live to the result); in the middle the base image (where the sky region is
//! drawn), the result, and the frame report.
//!
//! Base image view: in "Boundary line" / "Gradient" mode drag a line along the
//! horizon; in "Irregular mask" mode paint the sky with the left button, erase
//! with the right one ("Detect sky" paints a first mask). Frame report: click
//! a frame (in the table or the chart) to exclude it (or include it again).

use super::dialogs::{self, Picked};
use super::{combo, image_view, slider, texture, View};
use crate::image::{resize, Bits, Gray, Img};
use crate::io::{self, basename};
use crate::stacker::*;
use crate::stitcher::Model;
use crate::sys::gb;
use egui::{Color32, Pos2, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub const PROJECT_EXT: &str = ".stackproj";
const GROUP_KEYS: [&str; 7] = ["stars", "base", "darks", "bias", "flats", "flatdarks", "foreground"];
const DEM: [(u32, &str); 7] = [(3, "AHD"), (11, "DHT"), (12, "AAHD"), (4, "DCB"), (2, "PPG"), (1, "VNG"), (0, "Bilinear")];
const WB: [(io::WhiteBalance, &str); 3] = [(io::WhiteBalance::Camera, "Camera"), (io::WhiteBalance::Auto, "Average of image"), (io::WhiteBalance::Daylight, "Daylight")];
const STACK_PRESETS: [&str; 3] = ["Milky Way landscape", "Star trails", "Deep sky (telescope)"];

/// Sequator's names for the input groups (simple mode; also the keys of the explanations).
const GROUPS: [&str; 8] = ["Star images", "Base image", "Noise images", "Bias images", "Vignetting images", "Flat-dark images", "Foreground image", "Output"];
/// The usual names (normal mode).
const GROUPS_STD: [&str; 8] = ["Light frames", "Base frame", "Dark frames", "Bias frames", "Flat field frames", "Flat-dark frames", "Foreground image", "Output"];

fn group_name(g: usize, simple: bool) -> &'static str {
    if simple { GROUPS[g] } else { GROUPS_STD[g] }
}
const MASK_COLS: usize = 400;
const PREVIEW_MAX: usize = 1600;
const RESULT_MAX: usize = 3000;

/// How far a run is (for the progress bar): work units done of total.
#[derive(Clone, Default)]
struct Progress {
    msg: String,
    done: f64,
    total: f64,
    start: Option<std::time::Instant>,
}

enum Msg {
    /// A file's preview, and its size (h, w) as loaded at half size.
    Preview(String, Arc<Img>, (usize, usize)),
    Sky(Bits, String),
    Result(Box<StackResult>, Img, Gray, String),
    Post(u64, PostResult),
    Status(String),
    Error(String),
}

#[derive(PartialEq, Clone, Copy)]
enum ViewKind {
    Base,
    Result,
    Report,
}

pub struct StackTab {
    pub to_stitcher: Vec<String>,
    files: [Vec<String>; 7],
    group: usize,
    cfg: StackConfig,
    shown: Option<String>,
    disp: Option<Arc<Img>>,
    /// A light frame and its size (h, w) at half size, for the space estimate.
    frame_dims: Option<(String, (usize, usize))>,
    disp_tex: Option<egui::TextureHandle>,
    overlay_tex: Option<(String, egui::TextureHandle)>,
    aux: bool,
    show_stars: bool,
    brush: f64,
    view: ViewKind,
    vstate: View,
    result: Option<StackResult>,
    raw_small: Option<(Img, Gray, f64)>,
    post: Option<PostResult>,
    post_key: u64,
    post_busy: bool,
    show_bg: bool,
    res_tex: Option<(u64, egui::TextureHandle)>,
    running: bool,
    loading: bool,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Progress>>,
    status: String,
    pathbox: String,
    /// The output name while it is being typed (it may be empty meanwhile).
    name_edit: Option<String>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    dialog: Option<(Receiver<Picked>, &'static str)>,
    pub simple: bool,
    autorun_done: bool,
    running_preview: bool,
    /// The highlighted settings entry (None: the options of the highlighted file group).
    setting: Option<usize>,
    project: Option<String>,
    /// Fingerprint of the setup last saved to (or opened from) a project file.
    saved_fp: Option<u64>,
    post_note: &'static str,
}

/// The painted mask as run lengths (alternately not sky / sky), column by column
/// (the order the Julia version used, so its project files open here too).
fn runs(m: &Bits) -> Vec<i64> {
    let (mut out, mut cur, mut n) = (Vec::new(), false, 0i64);
    for c in 0..m.w {
        for r in 0..m.h {
            let b = m.at(r, c);
            if b == cur {
                n += 1;
            } else {
                out.push(n);
                cur = b;
                n = 1;
            }
        }
    }
    out.push(n);
    out
}
fn unruns(r: &[i64], rows: usize, cols: usize) -> Bits {
    let mut m = Bits::new(rows, cols);
    let mut p = 0usize;
    for (k, &n) in r.iter().enumerate() {
        for q in p..(p + n.max(0) as usize).min(rows * cols) {
            if k % 2 == 1 {
                m.set(q % rows, q / rows, true);
            }
        }
        p += n.max(0) as usize;
    }
    m
}

impl Default for StackTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        let mut cfg = StackConfig::default();
        cfg.sky.gradient = 0.1;
        Self {
            to_stitcher: vec![], files: Default::default(), group: 0, cfg, shown: None, disp: None, frame_dims: None, disp_tex: None, overlay_tex: None,
            aux: false, show_stars: false, brush: 5.0, view: ViewKind::Base, vstate: View::default(), result: None, raw_small: None,
            post: None, post_key: 0, post_busy: false, show_bg: false, res_tex: None, running: false, loading: false,
            cancel: Arc::new(AtomicBool::new(false)), progress: Arc::new(Mutex::new(Progress::default())),
            status: "Add light frames to begin. Choose a group, then Add… (or type a path).".into(), pathbox: String::new(), name_edit: None, tx, rx, dialog: None,
            simple: false, autorun_done: false, running_preview: false, setting: None, project: None, saved_fp: None, post_note: super::notes::note("Tone"),
        }
    }
}

fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl StackTab {
    pub fn busy(&self) -> bool {
        self.running || self.loading || self.post_busy || self.dialog.is_some()
    }

    fn base(&self) -> Option<String> {
        if let Some(b) = self.files[1].first() {
            return Some(b.clone());
        }
        let mut s = self.files[0].clone();
        s.sort();
        s.get(s.len().div_ceil(2).saturating_sub(1)).cloned()
    }

    fn settings(&self) -> StackConfig {
        let mut c = self.cfg.clone();
        c.stars = self.files[0].clone();
        c.base = self.files[1].first().cloned();
        c.darks = self.files[2].clone();
        c.bias = self.files[3].clone();
        c.flats = self.files[4].clone();
        c.flatdarks = self.files[5].clone();
        c.foreground.path = self.files[6].first().cloned().unwrap_or_default();
        c
    }

    fn add(&mut self, ps: Vec<String>) {
        if ps.is_empty() {
            return;
        }
        let g = self.group;
        if g == 7 {
            self.cfg.output = ps[0].clone();
            return;
        }
        let ps: Vec<String> = ps.into_iter().filter(|p| !self.files[g].contains(p)).collect();
        let n = ps.len();
        match g {
            1 => {
                self.files[1] = vec![ps[0].clone()];
                if !self.files[0].contains(&ps[0]) {
                    self.files[0].push(ps[0].clone());
                }
            }
            6 => self.files[6] = vec![ps[0].clone()],
            _ => {
                self.files[g].extend(ps);
                self.files[g].sort();
            }
        }
        if self.cfg.output.is_empty() {
            if let Some(f) = self.files[0].first() {
                self.cfg.output = std::path::Path::new(f).parent().unwrap_or(std::path::Path::new(".")).join("stacked.tif").to_string_lossy().to_string();
            }
        }
        self.status = format!("Added {n} file(s) to {}.", group_name(g, self.simple));
        self.cfg.excluded.retain(|p| self.files[0].contains(p));
        self.show_preview(self.base());
    }

    fn show_preview(&mut self, path: Option<String>) {
        if path == self.shown && self.disp.is_some() {
            return;
        }
        let Some(p) = path else {
            self.shown = None;
            self.disp = None;
            self.disp_tex = None;
            return;
        };
        if self.loading {
            return;
        }
        self.loading = true;
        let tx = self.tx.clone();
        self.status = format!("Loading preview of {}…", basename(&p));
        crate::sys::background(move || {
            let r = io::load_any(&p, &io::LoadOptions { half: true, orient: false, ..Default::default() }).map(|a| {
                let f = PREVIEW_MAX as f64 / a.w.max(a.h) as f64;
                ((a.h, a.w), if f < 1.0 { resize(&a, f) } else { a })
            });
            let _ = tx.send(match r {
                Ok((hw, img)) => Msg::Preview(p, Arc::new(img), hw),
                Err(e) => Msg::Error(format!("Cannot preview {}: {e}", basename(&p))),
            });
        });
    }

    fn mask_size(&self) -> (usize, usize) {
        let (h, w) = self.disp.as_ref().map(|d| (d.h, d.w)).unwrap_or((1, 1));
        (((MASK_COLS * h) as f64 / w as f64).round().max(1.0) as usize, MASK_COLS)
    }

    fn run(&mut self, preview: bool) {
        if self.running {
            self.cancel.store(true, Ordering::Relaxed);
            self.status = "Cancelling…".into();
            return;
        }
        if self.files[0].is_empty() {
            self.status = "Add light frames first.".into();
            return;
        }
        if !preview && self.cfg.output.is_empty() {
            self.status = "Set the output file first (Output).".into();
            return;
        }
        if !preview {
            if let Err(e) = crate::sys::check_writable(&self.cfg.output) {
                self.status = e;
                return;
            }
        }
        let cfg = self.settings();
        self.running = true;
        self.running_preview = preview;
        self.cancel.store(false, Ordering::Relaxed);
        let (tx, cancel, prog) = (self.tx.clone(), self.cancel.clone(), self.progress.clone());
        // Work units for the progress bar: one per frame loaded (calibration
        // frames, light frames), and combining about a third of loading.
        let pick = |n: usize, cap: usize| if preview { n.min(cap) } else { n };
        let lights = pick(self.files[0].iter().filter(|p| !cfg.excluded.contains(p)).count(), PREVIEW_FRAMES);
        let kinds = [("bias", pick(cfg.bias.len(), PREVIEW_CALIB)), ("dark", pick(cfg.darks.len(), PREVIEW_CALIB)), ("flat", pick(cfg.flats.len(), PREVIEW_CALIB)), ("flat dark", pick(cfg.flatdarks.len(), PREVIEW_CALIB))];
        let ncal: usize = kinds.iter().map(|k| k.1).sum();
        let combine_units = 0.35 * lights as f64;
        let total = ncal as f64 + lights as f64 + combine_units;
        *prog.lock().unwrap_or_else(|e| e.into_inner()) = Progress { msg: "Starting…".into(), done: 0.0, total, start: Some(std::time::Instant::now()) };
        crate::sys::background(move || {
            let t0 = std::time::Instant::now();
            let progress = |m: &str| -> Result<(), String> {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled.".into());
                }
                let mut p = prog.lock().unwrap_or_else(|e| e.into_inner());
                // Where in the run this message is.
                let frac = |s: &str| -> Option<(f64, f64)> {
                    let t = s.split(':').nth(1)?.split_whitespace().next()?;
                    let (a, b) = t.split_once('/')?;
                    Some((a.parse().ok()?, b.parse().ok()?))
                };
                let done = if let Some(kind) = m.strip_prefix("Master ").and_then(|r| r.split(':').next()) {
                    let before: usize = kinds.iter().take_while(|k| k.0 != kind).map(|k| k.1).sum();
                    frac(m).map(|(a, _)| before as f64 + a)
                } else if m.starts_with("Loading, calibrating, aligning") {
                    frac(m).map(|(a, _)| ncal as f64 + a)
                } else if m.starts_with("Loading the base") {
                    Some(ncal as f64)
                } else if let Some(r) = m.strip_prefix("Combining: ") {
                    r.trim_end_matches(" %").parse::<f64>().ok().map(|pc| ncal as f64 + lights as f64 + combine_units * pc / 100.0)
                } else if m.starts_with("Time-lapse frame") {
                    frac(m).map(|(a, b)| ncal as f64 + lights as f64 + combine_units * (a - 1.0) / b.max(1.0))
                } else if m.starts_with("Post-processing") || m.starts_with("Saving") {
                    Some(p.total)
                } else {
                    None
                };
                if let Some(d) = done {
                    p.done = d.max(p.done).min(p.total);
                }
                p.msg = m.to_string();
                Ok(())
            };
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stack(&cfg, preview, &progress, None))).unwrap_or_else(|_| Err("internal error (panic)".into()));
            let msg = match r {
                Ok(r) => {
                    let f = (RESULT_MAX as f64 / r.image.w.max(r.image.h) as f64).min(1.0);
                    let (raw, w) = if f < 1.0 { (resize(&r.raw, f), resize(&r.weight, f)) } else { (r.raw.clone(), r.weight.clone()) };
                    let nused = r.report.iter().filter(|fi| fi.used()).count();
                    let rej: Vec<&FrameInfo> = r.report.iter().filter(|fi| matches!(fi.status, Status::Rejected | Status::Failed)).collect();
                    let nexcl = r.report.iter().filter(|fi| fi.status == Status::Excluded).count();
                    let s = format!(
                        "{}{nused} frame(s) stacked{}{}. Brightness ×{:.2}.{} ({} s)",
                        if preview { "Quick preview: " } else { "Done: " },
                        if rej.is_empty() { String::new() } else { format!(", {} left out ({}{}) – see Frame report", rej.len(), rej.iter().take(2).map(|fi| format!("{}: {}", basename(&fi.path), fi.reason)).collect::<Vec<_>>().join("; "), if rej.len() > 2 { "; …" } else { "" }) },
                        if nexcl > 0 { format!(", {nexcl} excluded by you") } else { String::new() },
                        r.gain,
                        if r.saved.is_empty() { String::new() } else if r.saved.len() == 1 { format!(" Saved {}.", r.saved[0]) } else { format!(" Saved {} files.", r.saved.len()) },
                        t0.elapsed().as_secs()
                    );
                    let fw = r.fwhm * f;
                    let mut r = r;
                    r.fwhm = fw.max(r.fwhm * 0.0 + fw);
                    Msg::Result(Box::new(r), raw, w, s)
                }
                Err(e) => Msg::Error(if cancel.load(Ordering::Relaxed) { "Cancelled.".into() } else { e }),
            };
            let _ = tx.send(msg);
        });
    }

/// Starting points for typical subjects; every setting can still be changed.
    fn apply_preset(&mut self, name: &str) -> &'static str {
        let c = &mut self.cfg;
        // Common: sigma clipping, automatic frame rejection, auto brightness, no extras.
        c.composition = Composition::AlignStars;
        c.align_mode = AlignMode::Best;
        c.selective = false;
        c.trails = (Fade::Off, false);
        c.reject.auto = true;
        c.auto_brightness = true;
        c.hdr = false;
        c.bg_extraction.0 = false;
        c.color_calibration = false;
        c.enhance_stars.0 = false;
        c.light_pollution.filter = Pollution::Off;
        c.tone = Tone::SrgbCurve;
        c.stretch.auto = true;
        c.timelapse.0 = false;
        c.model = Model::Homography;
        c.distortion = Distortion::Off;
        match name {
            "Milky Way landscape" => {
                c.align_mode = AlignMode::Freeze;
                if matches!(c.sky.mode, SkyMode::None | SkyMode::All) {
                    c.sky.mode = SkyMode::Automatic;
                }
                c.distortion = Distortion::Auto;
                c.light_pollution.filter = Pollution::Uneven;
                c.light_pollution.strength = 50.0;
                "the sky is aligned, the foreground stays sharp; sky region detected automatically (or draw it), wide-angle distortion corrected, uneven light pollution reduced."
            }
            "Star trails" => {
                c.composition = Composition::StarTrails;
                c.trails = (Fade::Off, true);
                c.reject.auto = false;
                if matches!(c.sky.mode, SkyMode::None | SkyMode::All) {
                    c.sky.mode = SkyMode::Automatic;
                }
                "no alignment, the brightest value per pixel, the gaps between frames filled; sky region detected automatically (or draw it). Try Trail fade for a comet look."
            }
            _ => {
                // Deep sky (telescope)
                c.sky.mode = SkyMode::All;
                c.model = Model::Similarity;
                c.bg_extraction = (true, 2);
                c.color_calibration = true;
                c.tone = Tone::Stretch;
                "the whole frame is sky; frames only shift and turn; gradients removed, colours calibrated, automatic stretch. Save as .fits for further processing."
            }
        }
    }

    fn post_fingerprint(&self) -> u64 {
        let c = &self.cfg;
        hash_str(&format!("{:?}{:?}{:?}{:?}{:?}{:?}{}{}{:?}{:?}{:?}{:?}", c.bg_extraction, c.color_calibration, c.color_noise, c.light_pollution, c.enhance_stars, c.auto_brightness, c.hdr, c.tone.name(), c.stretch, c.color_space, c.sky.mode, self.raw_small.as_ref().map(|r| r.0.w)))
    }

    fn poll(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Preview(p, img, hw) => {
                    self.loading = false;
                    self.frame_dims = Some((p.clone(), hw));
                    self.shown = Some(p);
                    self.disp = Some(img);
                    self.disp_tex = None;
                    self.overlay_tex = None;
                    self.vstate.fit = true;
                    if !self.running {
                        self.status = "Ready.".into();
                    }
                }
                Msg::Sky(mask, s) => {
                    self.loading = false;
                    self.cfg.sky.mask = mask;
                    self.cfg.sky.mode = SkyMode::IrregularMask;
                    self.status = s;
                }
                Msg::Result(r, raw, w, s) => {
                    self.running = false;
                    let fw = r.fwhm;
                    self.raw_small = Some((raw, w, fw));
                    if let Some(m) = &r.mask {
                        if self.cfg.sky.mode == SkyMode::Automatic {
                            let _ = m;
                        }
                    }
                    self.result = Some(*r);
                    self.post = None;
                    self.post_key = 0;
                    self.res_tex = None;
                    self.view = ViewKind::Result;
                    self.vstate.fit = true;
                    self.status = s;
                }
                Msg::Post(key, p) => {
                    self.post_busy = false;
                    if self.cfg.tone == Tone::Stretch && self.cfg.stretch.auto {
                        if let Some((b, m, w)) = p.stretch {
                            (self.cfg.stretch.black, self.cfg.stretch.mid, self.cfg.stretch.white) = (b, m, w);
                        }
                    }
                    self.post = Some(p);
                    self.post_key = key;
                    self.res_tex = None;
                }
                Msg::Status(s) => self.status = s,
                Msg::Error(e) => {
                    self.running = false;
                    self.loading = false;
                    self.post_busy = false;
                    self.status = format!("Error: {e}");
                }
            }
        }
        if self.running {
            self.status = self.progress.lock().unwrap_or_else(|e| e.into_inner()).msg.clone();
        }
        // Live post-processing of the screen-size stack.
        if let Some((raw, w, fw)) = &self.raw_small {
            let key = self.post_fingerprint();
            if key != self.post_key && !self.post_busy {
                self.post_busy = true;
                let (raw, w, fw, cfg, tx) = (raw.clone(), w.clone(), *fw, self.cfg.clone(), self.tx.clone());
                crate::sys::background(move || {
                    let p = std::panic::catch_unwind(|| postprocess(&raw, &w, &cfg, fw, None));
                    let _ = tx.send(match p {
                        Ok(p) => Msg::Post(key, p),
                        Err(_) => Msg::Error("post-processing failed".into()),
                    });
                });
            }
        }
        if let Some((rx, what)) = &self.dialog {
            if let Ok(r) = rx.try_recv() {
                let what = *what;
                self.dialog = None;
                match r {
                    Picked::NoTool => self.status = dialogs::NO_DIALOG.into(),
                    Picked::Save(Some(p)) if what == "output" => self.cfg.output = p,
                    Picked::Save(Some(p)) if what == "result" => self.save_result(p),
                    Picked::Save(Some(p)) if what == "saveproj" => match self.save_project(&p) {
                        Ok(p) => self.status = format!("Saved project {p}."),
                        Err(e) => self.status = format!("Error: {e}"),
                    },
                    Picked::Files(ps) if what == "openproj" => {
                        if let Some(p) = ps.first() {
                            self.open_project(&p.clone());
                        }
                    }
                    Picked::Files(ps) => self.add(ps),
                    Picked::Save(_) => {}
                }
            }
        }
    }

    /// Toggles, sliders and menus by the names used in project files.
    #[allow(clippy::type_complexity)]
    fn named(&mut self) -> (Vec<(&'static str, &mut bool)>, Vec<(&'static str, f64, Box<dyn FnMut(&mut StackConfig, f64)>)>) {
        let c = &mut self.cfg;
        let sl: Vec<(&'static str, f64, Box<dyn FnMut(&mut StackConfig, f64)>)> = vec![
            ("gradient", c.sky.gradient * 100.0, Box::new(|c, v| c.sky.gradient = v / 100.0)),
            ("feather", c.sky.feather * 100.0, Box::new(|c, v| c.sky.feather = v / 100.0)),
            ("sigma low", c.sigma.low, Box::new(|c, v| c.sigma.low = v)),
            ("sigma high", c.sigma.high, Box::new(|c, v| c.sigma.high = v)),
            ("sigma iterations", c.sigma.iters as f64, Box::new(|c, v| c.sigma.iters = v as usize)),
            ("min light", c.reject.min_light * 100.0, Box::new(|c, v| c.reject.min_light = v / 100.0)),
            ("min stars", c.reject.min_stars * 100.0, Box::new(|c, v| c.reject.min_stars = v / 100.0)),
            ("max fwhm", c.reject.max_fwhm, Box::new(|c, v| c.reject.max_fwhm = v)),
            ("max elongation", c.reject.max_elong, Box::new(|c, v| c.reject.max_elong = v)),
            ("light pollution strength", c.light_pollution.strength, Box::new(|c, v| c.light_pollution.strength = v)),
            ("light pollution degree", c.light_pollution.degree as f64, Box::new(|c, v| c.light_pollution.degree = v as usize)),
            ("light pollution tiles", c.light_pollution.tiles as f64, Box::new(|c, v| c.light_pollution.tiles = v as usize)),
            ("background degree", c.bg_extraction.1 as f64, Box::new(|c, v| c.bg_extraction.1 = v as usize)),
            ("colour noise strength", c.color_noise.1, Box::new(|c, v| c.color_noise.1 = v)),
            ("black point", c.stretch.black, Box::new(|c, v| c.stretch.black = v)),
            ("midtones", c.stretch.mid, Box::new(|c, v| c.stretch.mid = v)),
            ("white point", c.stretch.white, Box::new(|c, v| c.stretch.white = v)),
            ("enhance strength", c.enhance_stars.1, Box::new(|c, v| c.enhance_stars.1 = v)),
            ("time-lapse frames", c.timelapse.1 as f64, Box::new(|c, v| c.timelapse.1 = v as usize)),
            ("foreground ev", c.foreground.ev, Box::new(|c, v| c.foreground.ev = v)),
        ];
        let tg: Vec<(&'static str, &mut bool)> = vec![
            ("unify exposure", &mut c.unify_exposure), ("homogenize vignetting", &mut c.homogenize_vignetting),
            ("selective", &mut c.selective), ("fill gaps", &mut c.trails.1), ("reject", &mut c.reject.auto),
            ("foreground match", &mut c.foreground.match_brightness), ("half size", &mut c.raw.half),
            ("auto brightness", &mut c.auto_brightness), ("hdr", &mut c.hdr), ("dynamic noises", &mut c.dynamic_noise),
            ("iaa", &mut c.light_pollution.iaa), ("background extraction", &mut c.bg_extraction.0),
            ("color calibration", &mut c.color_calibration), ("colour noise", &mut c.color_noise.0),
            ("automatic stretch", &mut c.stretch.auto), ("enhance star light", &mut c.enhance_stars.0),
            ("merge 4 pixels", &mut c.merge4), ("time-lapse", &mut c.timelapse.0),
            ("auxiliary highlight", &mut self.aux), ("show stars", &mut self.show_stars), ("show background", &mut self.show_bg),
        ];
        (tg, sl)
    }

    fn menus(&self) -> Vec<(&'static str, String)> {
        let c = &self.cfg;
        vec![
            ("composition", c.composition.name().into()),
            ("stacking", c.align_mode.name().into()),
            ("model", c.model.name().into()),
            ("sky region", c.sky.mode.name().into()),
            ("white balance", WB.iter().find(|w| w.0 == c.raw.wb).map(|w| w.1).unwrap_or("Camera").into()),
            ("demosaicing", DEM.iter().find(|d| d.0 == c.raw.quality).map(|d| d.1).unwrap_or("AHD").into()),
            ("distortion", c.distortion.name().into()),
            ("light pollution", c.light_pollution.filter.name().into()),
            ("color space", c.color_space.name().into()),
            ("tone", c.tone.name().into()),
            ("interpolation", c.interpolation.name().into()),
            ("trail fade", c.trails.0.name().into()),
            ("raw decoder", c.raw.decoder.name().into()),
        ]
    }

    fn set_menu(&mut self, k: &str, v: &str) {
        let c = &mut self.cfg;
        match k {
            "composition" => c.composition = Composition::from_name(v).unwrap_or(c.composition),
            "stacking" => c.align_mode = AlignMode::from_name(v).unwrap_or(c.align_mode),
            "model" => c.model = Model::from_name(v).filter(|m| matches!(m, Model::Homography | Model::Similarity)).unwrap_or(c.model),
            "sky region" => c.sky.mode = SkyMode::from_name(v).unwrap_or(c.sky.mode),
            "white balance" => c.raw.wb = WB.iter().find(|w| w.1 == v).map(|w| w.0).unwrap_or(c.raw.wb),
            "demosaicing" => c.raw.quality = DEM.iter().find(|d| d.1 == v).map(|d| d.0).unwrap_or(c.raw.quality),
            "distortion" => c.distortion = Distortion::from_name(v).unwrap_or(c.distortion),
            "light pollution" => c.light_pollution.filter = Pollution::from_name(v).unwrap_or(c.light_pollution.filter),
            "color space" => c.color_space = ColorSpace::from_name(v).unwrap_or(c.color_space),
            "tone" => c.tone = Tone::from_name(v).unwrap_or(c.tone),
            "interpolation" => c.interpolation = Interpolation::from_name(v).unwrap_or(c.interpolation),
            "trail fade" => c.trails.0 = Fade::from_name(v).unwrap_or(c.trails.0),
            "raw decoder" => {
                if let Some(d) = io::RawDecoder::from_name(v).filter(|d| *d == io::RawDecoder::Auto || io::available_raw_decoders().contains(d)) {
                    c.raw.decoder = d;
                }
            }
            _ => {}
        }
    }

    /// Projects (.stackproj, TOML text) keep the file lists, the sky region and every setting.
    pub fn save_project(&mut self, path: &str) -> Result<String, String> {
        use toml::{Table, Value};
        let mut path = path.to_string();
        if !path.to_lowercase().ends_with(PROJECT_EXT) {
            path += PROJECT_EXT;
        }
        let dir = std::path::absolute(&path).map_err(|e| e.to_string())?.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let fref = |p: &str| {
            let ap = std::path::absolute(p).unwrap_or_else(|_| p.into());
            let mut t = Table::new();
            t.insert("path".into(), ap.to_string_lossy().to_string().into());
            t.insert("relative".into(), super::relpath(&ap, &dir).into());
            Value::Table(t)
        };
        let mut d = Table::new();
        d.insert("format".into(), "PanoStitch stacking project".into());
        d.insert("version".into(), 2.into());
        d.insert("files".into(), Value::Table((0..7).map(|g| (GROUP_KEYS[g].to_string(), Value::Array(self.files[g].iter().map(|p| fref(p)).collect()))).collect()));
        let excl: Vec<Value> = self.files[0].iter().enumerate().filter(|(_, p)| self.cfg.excluded.contains(p)).map(|(k, _)| Value::from((k + 1) as i64)).collect();
        d.insert("excluded".into(), Value::Array(excl));
        d.insert("menus".into(), Value::Table(self.menus().into_iter().map(|(k, v)| (k.to_string(), Value::from(v))).collect()));
        let out = self.cfg.output.clone();
        let line = self.cfg.sky.line;
        let mask = self.cfg.sky.mask.clone();
        let brush = self.brush;
        let (tg, sl) = self.named();
        d.insert("toggles".into(), Value::Table(tg.into_iter().map(|(k, v)| (k.to_string(), Value::from(*v))).collect()));
        let mut slt: Table = sl.into_iter().map(|(k, v, _)| (k.to_string(), Value::from(v))).collect();
        slt.insert("brush".into(), brush.into());
        d.insert("sliders".into(), Value::Table(slt));
        if !out.is_empty() {
            d.insert("output".into(), fref(&out));
        }
        if let Some(l) = line {
            d.insert("sky_line".into(), Value::Array(vec![l.0.into(), l.1.into(), l.2.into(), l.3.into()]));
        }
        if mask.any() {
            let mut t = Table::new();
            t.insert("size".into(), Value::Array(vec![(mask.h as i64).into(), (mask.w as i64).into()]));
            t.insert("runs".into(), Value::Array(runs(&mask).into_iter().map(Value::from).collect()));
            d.insert("sky_mask".into(), Value::Table(t));
        }
        std::fs::write(&path, toml::to_string(&d).map_err(|e| e.to_string())?).map_err(|e| format!("Cannot save {path}: {e}"))?;
        self.project = Some(path.clone());
        self.saved_fp = Some(self.fingerprint());
        Ok(path)
    }

    pub fn open_project(&mut self, path: &str) {
        if self.running {
            self.status = "Wait until the stacking has finished.".into();
            return;
        }
        match self.read_project(path) {
            Ok(s) => {
                self.status = s;
                self.saved_fp = Some(self.fingerprint());
            }
            Err(e) => self.status = e,
        }
    }

    fn read_project(&mut self, path: &str) -> Result<String, String> {
        use toml::Value;
        let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", basename(path)))?;
        let d: toml::Table = text.parse().map_err(|e| format!("Cannot read {}: {e}", basename(path)))?;
        if d.get("format").and_then(|v| v.as_str()) != Some("PanoStitch stacking project") {
            return Err(format!("{} is not a stacking project.", basename(path)));
        }
        let dir = std::path::absolute(path).map_err(|e| e.to_string())?.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let resolve = |r: &Value| -> (Option<String>, String) {
            let ap = r.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let rel = dir.join(r.get("relative").and_then(|v| v.as_str()).unwrap_or(""));
            let found = if std::path::Path::new(&ap).is_file() { Some(ap.clone()) } else if rel.is_file() { Some(rel.to_string_lossy().to_string()) } else { None };
            (found, ap)
        };
        let mut missing = Vec::new();
        let files = d.get("files").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        let mut found_stars: Vec<Option<String>> = Vec::new();
        for g in 0..7 {
            let recs = files.get(GROUP_KEYS[g]).and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let fs: Vec<Option<String>> = recs.iter().map(|r| { let (f, ap) = resolve(r); if f.is_none() { missing.push(basename(&ap)); } f }).collect();
            if g == 0 {
                found_stars = fs.clone();
            }
            self.files[g] = fs.into_iter().flatten().collect();
        }
        self.cfg.excluded = d.get("excluded").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|k| k.as_integer()).filter_map(|k| found_stars.get(k as usize - 1).cloned().flatten()).collect()).unwrap_or_default();
        // The output need not exist yet: keep its absolute path if that folder exists.
        self.cfg.output = match d.get("output") {
            Some(o) => {
                let ap = o.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if std::path::Path::new(&ap).parent().is_some_and(|p| p.is_dir()) { ap } else { dir.join(o.get("relative").and_then(|v| v.as_str()).unwrap_or("stacked.tif")).to_string_lossy().to_string() }
            }
            None => String::new(),
        };
        let tbl = |k: &str| d.get(k).and_then(|v| v.as_table()).cloned().unwrap_or_default();
        for (k, v) in tbl("menus") {
            if let Some(v) = v.as_str() {
                self.set_menu(&k, v);
            }
        }
        let toggles = tbl("toggles");
        // Version 1 projects: "motion effect" became the trail fade "Fade in".
        if toggles.get("motion effect").and_then(|v| v.as_bool()) == Some(true) && !tbl("menus").contains_key("trail fade") {
            self.cfg.trails.0 = Fade::In;
        }
        let sliders = tbl("sliders");
        if let Some(b) = sliders.get("brush").and_then(|v| v.as_float().or(v.as_integer().map(|i| i as f64))) {
            self.brush = b;
        }
        let (tg, sl) = self.named();
        for (k, v) in tg {
            if let Some(b) = toggles.get(k).and_then(|x| x.as_bool()) {
                *v = b;
            }
        }
        let mut setters = Vec::new();
        for (k, _, f) in sl {
            if let Some(v) = sliders.get(k).and_then(|x| x.as_float().or(x.as_integer().map(|i| i as f64))) {
                setters.push((v, f));
            }
        }
        for (v, mut f) in setters {
            f(&mut self.cfg, v);
        }
        let num = |v: &Value| v.as_float().or(v.as_integer().map(|i| i as f64));
        self.cfg.sky.line = d.get("sky_line").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(num).collect::<Vec<f64>>()).filter(|a| a.len() == 4).map(|a| (a[0], a[1], a[2], a[3]));
        self.cfg.sky.mask = match d.get("sky_mask") {
            Some(m) => {
                let size: Vec<i64> = m.get("size").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_integer()).collect()).unwrap_or_default();
                let r: Vec<i64> = m.get("runs").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_integer()).collect()).unwrap_or_default();
                if size.len() == 2 { unruns(&r, size[0] as usize, size[1] as usize) } else { Bits::new(0, 0) }
            }
            None => Bits::new(0, 0),
        };
        self.shown = None;
        self.disp = None;
        self.result = None;
        self.raw_small = None;
        self.post = None;
        self.project = Some(path.to_string());
        self.group = 0;
        self.view = ViewKind::Base;
        self.show_preview(self.base());
        let counts: String = (2..7).filter(|&g| !self.files[g].is_empty()).map(|g| format!(", {} {}", self.files[g].len(), group_name(g, self.simple).to_lowercase())).collect();
        Ok(format!(
            "Opened {}: {} light frames{counts}.{}",
            basename(path),
            self.files[0].len(),
            if missing.is_empty() { String::new() } else { format!(" Missing files: {}.", missing.join(", ")) }
        ))
    }

    fn save_result(&mut self, p: String) {
        let Some(r) = self.result.clone() else { return };
        let mut cfg = self.settings();
        cfg.output = p.clone();
        let tx = self.tx.clone();
        self.status = format!("Saving {}…", basename(&p));
        crate::sys::background(move || {
            let _ = tx.send(match save_result(&r, &cfg) {
                Ok(_) => Msg::Status(format!("Saved {p}{}", if io::is_astro(&p) { " (linear 32-bit)." } else { "." })),
                Err(e) => Msg::Error(e),
            });
        });
    }

    fn toggle_exclude(&mut self, path: &str) {
        if Some(path.to_string()) == self.base() || self.result.as_ref().is_some_and(|r| r.base == path) {
            self.status = "The base frame cannot be excluded (choose another base first).".into();
            return;
        }
        if let Some(k) = self.cfg.excluded.iter().position(|p| p == path) {
            self.cfg.excluded.remove(k);
            self.status = format!("{} will be used in the next run.", basename(path));
        } else {
            self.cfg.excluded.push(path.to_string());
            self.status = format!("{} will be left out of the next run.", basename(path));
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut as Ks, Modifiers as Mo};
        let (save, open, start, prev) = ctx.input_mut(|i| {
            (i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::S)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::O)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::Enter)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::P)))
        });
        if save {
            self.ask_save_project(true);
        }
        if open {
            self.ask_open_project();
        }
        // These start a run, never cancel one (Esc cancels).
        if (start || prev) && !self.running {
            self.run(prev && !start);
        }
        if !ctx.egui_wants_keyboard_input() {
            let (fit, esc) = ctx.input(|i| (i.key_pressed(Key::F) && i.modifiers.is_none(), i.key_pressed(Key::Escape)));
            if fit {
                self.vstate.fit = true;
            }
            if esc && self.running {
                self.run(false);
            }
        }
    }

    fn ask_open_project(&mut self) {
        if self.dialog.is_none() {
            self.dialog = Some((dialogs::pick_open("Open project", "Stacking projects", PROJECT_EXT), "openproj"));
        }
    }

    /// Save the project: `quick` saves straight to the open project file, if there is one.
    pub fn ask_save_project(&mut self, quick: bool) {
        if quick {
            if let Some(p) = self.project.clone() {
                self.status = match self.save_project(&p) {
                    Ok(p) => format!("Saved project {p}."),
                    Err(e) => format!("Error: {e}"),
                };
                return;
            }
        }
        if self.dialog.is_none() {
            let default = self.project.clone().unwrap_or_else(|| match self.files[0].first() {
                Some(f) => std::path::Path::new(f).parent().unwrap_or(std::path::Path::new(".")).join(format!("stacking{PROJECT_EXT}")).to_string_lossy().to_string(),
                None => format!("stacking{PROJECT_EXT}"),
            });
            self.dialog = Some((dialogs::pick_save("Save stacking project", &default), "saveproj"));
        }
    }

    /// Everything a project file keeps, as one number.
    fn fingerprint(&mut self) -> u64 {
        use std::hash::{Hash, Hasher};
        let menus = self.menus();
        let (tg, sl) = self.named();
        let t: Vec<(&str, bool)> = tg.iter().map(|(k, v)| (*k, **v)).collect();
        let v: Vec<(&str, f64)> = sl.iter().map(|(k, v, _)| (*k, *v)).collect();
        let text = format!("{:?}{:?}{t:?}{v:?}{:?}{:?}{:?}{:?}{}", self.files, self.cfg.excluded, menus, self.cfg.output, self.cfg.sky.line, runs(&self.cfg.sky.mask), self.brush);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        h.finish()
    }

    /// Whether there are light frames whose setup is not saved in a project file.
    pub fn unsaved(&mut self) -> bool {
        let fp = self.fingerprint();
        !self.files[0].is_empty() && self.saved_fp != Some(fp)
    }

    /// Stop a running stack (when the window closes).
    pub fn cancel_run(&mut self) {
        if self.running {
            self.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Whether a stacking run is going.
    pub fn running(&self) -> bool {
        self.running
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.poll();
        // Development aid (screenshots without clicking): PANOSTITCH_AUTORUN=preview presses
        // Quick preview once the files are in; PANOSTITCH_VIEW=result|report then shows that view.
        if !self.autorun_done && !self.busy() && !self.files[0].is_empty() {
            self.autorun_done = true;
            if std::env::var("PANOSTITCH_AUTORUN").as_deref() == Ok("preview") {
                self.run(true);
            }
        }
        if self.result.is_some() && !self.running {
            match std::env::var("PANOSTITCH_VIEW").as_deref() {
                Ok("report") if self.view != ViewKind::Report => {
                    self.view = ViewKind::Report;
                    std::env::remove_var("PANOSTITCH_VIEW");
                }
                Ok("result") => std::env::remove_var("PANOSTITCH_VIEW"),
                _ => {}
            }
        }
        self.shortcuts(ctx);
        egui::Panel::left("stack_left").default_size(470.0).size_range(400.0..=750.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("kl").show(ui, |ui| self.left(ui));
        });
        egui::Panel::right("stack_right").default_size(340.0).size_range(300.0..=600.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("kr").show(ui, |ui| self.right(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.center(ui, ctx));
    }

    /// The output path split into folder, name (without extension) and extension, with defaults.
    fn out_parts(&self) -> (String, String, String) {
        let def_dir = || self.files[0].first().and_then(|f| std::path::Path::new(f).parent()).map(|d| d.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
        if self.cfg.output.is_empty() {
            return (def_dir(), "stacked".into(), "tif".into());
        }
        let p = std::path::Path::new(&self.cfg.output);
        let dir = p.parent().map(|d| d.to_string_lossy().to_string()).filter(|d| !d.is_empty()).unwrap_or_else(def_dir);
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "stacked".into());
        let ext = match io::ext_of(&self.cfg.output).as_str() {
            "" => "tif".to_string(),
            "tiff" => "tif".to_string(),
            "jpeg" => "jpg".to_string(),
            e => e.to_string(),
        };
        (dir, stem, ext)
    }

    fn set_out(&mut self, dir: &str, stem: &str, ext: &str) {
        let clean: String = stem.trim().chars().map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
        let stem = if clean.is_empty() { "stacked" } else { clean.as_str() };
        self.cfg.output = std::path::Path::new(dir).join(format!("{stem}.{ext}")).to_string_lossy().to_string();
    }

    /// Names to offer for the output file.
    fn name_suggestions(&self) -> Vec<String> {
        let stem = |f: &String| std::path::Path::new(f).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let kind = match self.cfg.composition {
            Composition::StarTrails => "star_trails",
            _ if self.cfg.sky.mode == SkyMode::None => "deep_sky",
            _ => "milky_way",
        };
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut v = vec!["stacked".to_string(), kind.to_string(), format!("{kind}_{date}")];
        if let (Some(a), Some(b)) = (self.files[0].first(), self.files[0].last()) {
            v.push(format!("{}_stacked", stem(a)));
            if a != b {
                v.push(format!("{}-{}", stem(a), stem(b)));
            }
            if let Some(d) = std::path::Path::new(a).parent().and_then(|d| d.file_name()) {
                v.push(format!("{}_stacked", d.to_string_lossy()));
            }
        }
        v.push(format!("{kind}_{}lights", self.files[0].len()));
        let mut seen = std::collections::HashSet::new();
        v.retain(|n| seen.insert(n.clone()));
        v
    }

    /// Estimated working space for the frames of a full run (bytes), and the
    /// space the aligned frames take on disk (Align only), from the frame size
    /// (or, before a preview, the average file size) and the number of frames.
    fn work_needs(&self) -> Option<(f64, f64)> {
        let k = self.files[0].iter().filter(|p| !self.cfg.excluded.contains(p)).count();
        if k == 0 {
            return None;
        }
        let half_px = match &self.frame_dims {
            Some((p, (h, w))) if self.files[0].contains(p) => {
                // Previews are loaded at half size; RAWs are stacked at full size unless Half size is on.
                let px = (h * w) as f64;
                if io::is_raw(p) { px * 4.0 } else { px }
            }
            _ => {
                // No preview yet: guess the pixels from the average file size.
                let sizes: Vec<f64> = self.files[0].iter().take(20).filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len() as f64).collect();
                if sizes.is_empty() {
                    return None;
                }
                let avg = sizes.iter().sum::<f64>() / sizes.len() as f64;
                let p = &self.files[0][0];
                avg / if io::is_raw(p) { 1.3 } else if io::is_astro(p) { 2.0 } else { 6.0 }
            }
        };
        let raw = io::is_raw(&self.files[0][0]);
        let mut px = half_px;
        if raw && self.cfg.raw.half {
            px /= 4.0;
        }
        if self.cfg.merge4 {
            px /= 4.0;
        }
        let store = 6.0 * px * k as f64;
        let aligned = if self.cfg.align_mode == AlignMode::AlignOnly { px * k as f64 * if io::is_astro(&self.cfg.output) { 12.0 } else { 6.0 } } else { 0.0 };
        Some((store, aligned))
    }

    /// Always-visible output choice: folder, file name (with suggestions) and file type.
    fn output_row(&mut self, ui: &mut egui::Ui) {
        let (dir, stem0, ext) = self.out_parts();
        let mut stem = self.name_edit.clone().unwrap_or(stem0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Save as").strong());
            let r = ui.add(egui::TextEdit::singleline(&mut stem).desired_width(150.0).hint_text("file name"));
            if r.has_focus() {
                self.name_edit = Some(stem.clone());
                if !stem.trim().is_empty() {
                    self.set_out(&dir, &stem, &ext);
                }
            } else if self.name_edit.take().is_some() || r.lost_focus() {
                self.set_out(&dir, &stem, &ext);
            }
            let mut pick: Option<String> = None;
            egui::ComboBox::from_id_salt("outname").selected_text("").width(18.0).height(400.0).show_ui(ui, |ui| {
                ui.weak("Suggested names");
                for n in self.name_suggestions() {
                    if ui.selectable_label(n == stem, &n).clicked() {
                        pick = Some(n);
                    }
                }
            }).response.on_hover_text("Suggested file names");
            if let Some(n) = pick {
                self.set_out(&dir, &n, &ext);
            }
            ui.label(".");
            let mut new_ext: Option<&str> = None;
            egui::ComboBox::from_id_salt("outext").selected_text(ext.as_str()).width(55.0).show_ui(ui, |ui| {
                for (e, d) in io::OUTPUT_FORMATS {
                    if ui.selectable_label(ext == e, format!("{e}   {d}")).clicked() {
                        new_ext = Some(e);
                    }
                }
            }).response.on_hover_text(io::OUTPUT_FORMATS.iter().find(|f| f.0 == ext).map(|f| f.1).unwrap_or("file type"));
            if let Some(e) = new_ext {
                self.set_out(&dir, &stem, e);
            }
            if ui.button("Folder…").on_hover_text(format!("Saved in {dir}")).clicked() && self.dialog.is_none() {
                let d = if self.cfg.output.is_empty() { std::path::Path::new(&dir).join(format!("{stem}.{ext}")).to_string_lossy().to_string() } else { self.cfg.output.clone() };
                self.dialog = Some((dialogs::pick_save("Output", &d), "output"));
            }
        });
        if let Some((store, aligned)) = self.work_needs() {
            let (mem, disk) = crate::sys::free_cached(std::path::Path::new(&dir));
            let in_ram = store < 0.5 * mem;
            let disk_need = aligned + if in_ram { 0.0 } else { store + 2e9_f64.max(0.02 * store) };
            let disk_ok = disk.is_none_or(|d| disk_need <= d as f64);
            let disk_free = disk.map(|d| format!("{} free here", gb(d as f64))).unwrap_or_else(|| "free space unknown".into());
            let mut text = format!(
                "Frames need ≈ {}: {}",
                gb(store),
                if in_ram { format!("kept in memory ({} free)", gb(mem)) } else { format!("too big for memory ({} free), kept on disk here ({disk_free})", gb(mem)) }
            );
            if aligned > 0.0 {
                text += &format!("; aligned frames ≈ {} on disk ({disk_free})", gb(aligned));
            }
            if disk_ok {
                ui.weak(text);
            } else {
                ui.colored_label(Color32::from_rgb(220, 120, 0), format!("⚠ {text}. Not enough space: switch on Half size (RAW conversion) or Merge 4 pixels, use fewer frames, or choose a folder on a bigger disk."));
            }
        }
        let exists = !self.cfg.output.is_empty() && std::path::Path::new(&self.cfg.output).exists();
        ui.horizontal_wrapped(|ui| {
            ui.weak(format!("in {dir}"));
            if exists {
                ui.colored_label(Color32::from_rgb(200, 120, 0), "– exists, will be replaced");
                if ui.small_button("next free name").clicked() {
                    let base = stem.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('_').to_string();
                    if let Some(n) = (2..10_000).map(|k| format!("{base}_{k}")).find(|n| !std::path::Path::new(&dir).join(format!("{n}.{ext}")).exists()) {
                        self.set_out(&dir, &n, &ext);
                    }
                }
            }
        });
    }

    fn left(&mut self, ui: &mut egui::Ui) {
        ui.heading("Input");
        // One row per group: status dot, name, and a dropdown with its files
        // (hundreds of light frames do not fit in a list).
        let base = self.base();
        let mut show: Option<String> = None;
        let mut new_base: Option<Option<String>> = None;
        egui::Grid::new("inputs").num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
            for g in 0..8 {
                let n = if g < 7 { self.files[g].len() } else { usize::from(!self.cfg.output.is_empty()) };
                let dot = if (g == 0 || g == 7) && n == 0 { Color32::RED } else if n > 0 || (g == 1 && base.is_some()) { Color32::from_rgb(0, 170, 0) } else { Color32::GRAY };
                ui.colored_label(dot, "●");
                let name = group_name(g, self.simple);
                let t = if [0, 2, 3, 4, 5].contains(&g) && n > 0 { format!("{name} ({n})") } else { name.to_string() };
                if ui.selectable_label(self.group == g && self.setting.is_none(), egui::RichText::new(t).strong()).clicked() {
                    self.group = g;
                    self.setting = None;
                }
                let w = 175.0;
                match g {
                    1 => {
                        // The base frame: automatic (the middle one) or any light frame.
                        let cur = match self.files[1].first() {
                            Some(b) => basename(b),
                            None => base.as_ref().map(|b| format!("(auto) {}", basename(b))).unwrap_or_else(|| "–".into()),
                        };
                        egui::ComboBox::from_id_salt("pick1").selected_text(cur).width(w).height(400.0).show_ui(ui, |ui| {
                            if ui.selectable_label(self.files[1].is_empty(), "(auto: the middle one)").clicked() {
                                new_base = Some(None);
                            }
                            for f in &self.files[0] {
                                if ui.selectable_label(self.files[1].contains(f), basename(f)).clicked() {
                                    new_base = Some(Some(f.clone()));
                                }
                            }
                        });
                    }
                    7 => {
                        ui.label(if self.cfg.output.is_empty() { "–".to_string() } else { basename(&self.cfg.output) });
                    }
                    _ if n == 0 => {
                        ui.weak("–");
                    }
                    _ => {
                        let cur = self.shown.as_ref().filter(|s| self.files[g].contains(s)).map(|s| basename(s)).unwrap_or_else(|| if n == 1 { basename(&self.files[g][0]) } else { format!("{n} files") });
                        egui::ComboBox::from_id_salt(("pick", g)).selected_text(cur).width(w).height(400.0).show_ui(ui, |ui| {
                            for f in &self.files[g] {
                                let mut t = basename(f);
                                if g == 0 && self.files[1].contains(f) {
                                    t += "  ← base";
                                }
                                if g == 0 && self.cfg.excluded.contains(f) {
                                    t += "  ✗ excluded";
                                }
                                if ui.selectable_label(self.shown.as_deref() == Some(f.as_str()), t).clicked() {
                                    show = Some(f.clone());
                                    self.group = g;
                                    self.setting = None;
                                }
                            }
                        });
                    }
                }
                ui.end_row();
            }
        });
        self.output_row(ui);
        if let Some(b) = new_base {
            self.files[1] = b.iter().cloned().collect();
            if let Some(b) = &b {
                self.cfg.excluded.retain(|p| p != b);
                self.status = format!("Base frame: {}.", basename(b));
            } else {
                self.status = "Base frame: automatic (the middle light frame).".into();
            }
            self.group = 1;
            self.setting = None;
            show = self.base();
        }
        if let Some(f) = show {
            self.view = ViewKind::Base;
            self.show_preview(Some(f));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Add…").clicked() && self.dialog.is_none() {
                if self.group == 7 {
                    let d = if self.cfg.output.is_empty() { "stacked.tif".to_string() } else { self.cfg.output.clone() };
                    self.dialog = Some((dialogs::pick_save("Output", &d), "output"));
                } else {
                    self.dialog = Some((dialogs::pick_images(), "add"));
                }
            }
            if ui.button("Remove").clicked() {
                let g = self.group;
                if g == 7 {
                    self.cfg.output.clear();
                } else if let Some(s) = self.shown.clone().filter(|s| self.files[g].contains(s)) {
                    self.files[g].retain(|p| *p != s);
                    if g == 0 {
                        self.files[1].retain(|p| *p != s);
                    }
                    self.show_preview(self.base());
                } else {
                    self.status = "Choose a file in the group's dropdown first (Clear removes the whole group).".into();
                }
            }
            if ui.button("Clear").clicked() {
                let g = self.group;
                if g == 7 {
                    self.cfg.output.clear();
                } else {
                    self.files[g].clear();
                    if g == 0 {
                        self.files[1].clear();
                    }
                }
                self.show_preview(self.base());
            }
            if ui.button("Use as base").clicked() {
                match self.shown.clone().filter(|s| self.files[0].contains(s)) {
                    Some(s) => {
                        self.files[1] = vec![s.clone()];
                        self.cfg.excluded.retain(|p| *p != s);
                        self.status = format!("{}: {}.", group_name(1, self.simple), basename(&s));
                    }
                    None => self.status = format!("Choose one of the {} in its dropdown first.", group_name(0, self.simple).to_lowercase()),
                }
            }
        });
        let r = ui.add(egui::TextEdit::singleline(&mut self.pathbox).hint_text("…or type a file/folder path + Enter").desired_width(ui.available_width().min(420.0)));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.pathbox.trim().is_empty() {
            let t = self.pathbox.trim().to_string();
            self.pathbox.clear();
            if t.to_lowercase().ends_with(PROJECT_EXT) {
                self.open_project(&dialogs::expand_paths(&t)[0]);
            } else {
                let ps = if self.group == 7 { vec![t] } else { dialogs::expand_paths(&t) };
                self.add(ps);
            }
        }
        // The settings list: click an entry to see its options below.
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Preset").strong());
            egui::ComboBox::from_id_salt("stackpreset").selected_text("choose…").width(220.0).show_ui(ui, |ui| {
                for name in STACK_PRESETS {
                    if ui.selectable_label(false, name).clicked() {
                        let note = self.apply_preset(name);
                        self.status = format!("Preset “{name}”: {note}");
                    }
                }
            });
        });
        let c = &self.cfg;
        let onoff = |b: bool| if b { "On" } else { "Off" };
        let mut rows: Vec<(usize, String, bool)> = vec![
            (1, format!("Composition: {}", c.composition.name()), true),
            (
                1,
                format!(
                    "     {}",
                    if c.composition == Composition::StarTrails {
                        format!("{}{}", if c.trails.0 == Fade::Off { "Lighten" } else { c.trails.0.name() }, if c.trails.1 { ", gaps filled" } else { "" })
                    } else {
                        format!("{}{}", c.align_mode.name(), if c.selective { ", selective" } else { "" })
                    }
                ),
                true,
            ),
            (2, format!("Sky region: {}", c.sky.mode.name()), c.sky.mode != SkyMode::None),
            (3, format!("Sigma clipping: {} / {} σ, {}×", c.sigma.low, c.sigma.high, c.sigma.iters), true),
            (4, format!("Frame rejection: {}", if c.reject.auto { "Auto" } else { "Off" }), c.reject.auto),
            (
                5,
                format!(
                    "RAW conversion: {} WB, {}{}",
                    WB.iter().find(|w| w.0 == c.raw.wb).map(|w| w.1).unwrap_or("Camera"),
                    DEM.iter().find(|d| d.0 == c.raw.quality).map(|d| d.1).unwrap_or("AHD"),
                    if c.raw.half { ", half size" } else { "" }
                ),
                true,
            ),
            (6, format!("Reduce dynamic noises: {}", onoff(c.dynamic_noise)), c.dynamic_noise),
            (7, format!("Reduce distor. effects: {}", c.distortion.name()), c.distortion != Distortion::Off),
            (8, format!("Merge 4 pixels: {}", onoff(c.merge4)), c.merge4),
            (9, format!("Time-lapse: {}", if c.timelapse.0 { format!("every {} frames", c.timelapse.1) } else { "Off".into() }), c.timelapse.0),
        ];
        if self.simple {
            rows.retain(|r| r.0 <= 2); // simple mode: the essentials
        }
        let mut first = 0;
        for (k, text, active) in &rows {
            ui.horizontal_wrapped(|ui| {
                let dot = if *k != first {
                    if *active { Color32::from_rgb(65, 105, 225) } else { Color32::GRAY }
                } else {
                    Color32::TRANSPARENT
                };
                ui.colored_label(dot, "●");
                if ui.selectable_label(self.setting == Some(*k), text).clicked() {
                    self.setting = Some(*k);
                }
            });
            first = *k;
        }
        ui.separator();
        const TITLES: [&str; 10] = ["", "Composition", "Sky region", "Sigma clipping", "Frame rejection", "RAW conversion", "Reduce dynamic noises", "Reduce distor. effects", "Merge 4 pixels", "Time-lapse"];
        let title = match self.setting {
            Some(k) => TITLES[k],
            None => group_name(self.group, self.simple),
        };
        ui.label(egui::RichText::new(title).strong().size(16.0));
        let note = |ui: &mut egui::Ui, n: &str| {
            ui.label(egui::RichText::new(super::notes::note(n)).weak().size(12.5));
        };
        let nstars = self.files[0].len();
        match self.setting {
            None => {
                match self.group {
                    0 => {
                        ui.checkbox(&mut self.cfg.unify_exposure, "Unify exposure");
                    }
                    4 => {
                        ui.checkbox(&mut self.cfg.homogenize_vignetting, "Homogenize vignetting");
                    }
                    6 => {
                        ui.checkbox(&mut self.cfg.foreground.match_brightness, "Match brightness to the stacked ground");
                        ui.add(egui::Slider::new(&mut self.cfg.foreground.ev, -3.0..=3.0).step_by(0.1).text("Exposure (EV)"));
                    }
                    7 => {
                        ui.label("Saved as (edit the full path here, or use the Save as row on the left):");
                        ui.add(egui::TextEdit::singleline(&mut self.cfg.output).hint_text("output file (.tif / .png / .jpg / .fits / .xisf)").desired_width(ui.available_width().min(420.0)));
                        if !self.cfg.output.is_empty() && std::path::Path::new(&self.cfg.output).exists() {
                            ui.colored_label(Color32::from_rgb(200, 120, 0), "This file exists and will be replaced.");
                        }
                    }
                    _ => {}
                }
                note(ui, GROUPS[self.group]);
            }
            Some(1) => {
                let c = &mut self.cfg;
                egui::Grid::new("compo").num_columns(2).show(ui, |ui| {
                    combo(ui, "Composition", &mut c.composition, Composition::ALL, |v| v.name());
                    combo(ui, "Stacking", &mut c.align_mode, AlignMode::ALL, |v| v.name());
                    if !self.simple {
                        combo(ui, "Model", &mut c.model, &[Model::Homography, Model::Similarity], |v| v.name());
                        combo(ui, "Interpolation", &mut c.interpolation, Interpolation::ALL, |v| v.name());
                        ui.label("");
                        ui.checkbox(&mut c.selective, "Selective (sigma clipping in the sky)");
                        ui.end_row();
                    }
                    combo(ui, "Trail fade", &mut c.trails.0, Fade::ALL, |v| v.name());
                    ui.label("");
                    ui.checkbox(&mut c.trails.1, "Fill gaps between frames (star trails)");
                    ui.end_row();
                });
                note(ui, "Composition");
            }
            Some(2) => {
                egui::Grid::new("sky").num_columns(2).show(ui, |ui| {
                    combo(ui, "Sky region", &mut self.cfg.sky.mode, SkyMode::ALL, |v| v.name());
                    let mut g = self.cfg.sky.gradient * 100.0;
                    if slider(ui, "Gradient (% height)", &mut g, 1.0..=50.0, 1.0) {
                        self.cfg.sky.gradient = g / 100.0;
                    }
                    slider(ui, "Brush size (% width)", &mut self.brush, 0.5..=30.0, 0.5);
                    let mut f = self.cfg.sky.feather * 100.0;
                    if slider(ui, "Mask feather (% width)", &mut f, 0.0..=10.0, 0.5) {
                        self.cfg.sky.feather = f / 100.0;
                    }
                });
                if ui.checkbox(&mut self.aux, "Auxiliary highlight").changed() {
                    self.disp_tex = None;
                }
                if ui.checkbox(&mut self.show_stars, "Show detected stars").changed() && self.show_stars && self.result.is_none() {
                    self.status = "Stars are shown after a run (Quick preview or Start).".into();
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("◎ Detect sky").clicked() {
                        match (&self.disp, &self.shown) {
                            (Some(img), Some(sh)) if !self.loading => {
                                self.loading = true;
                                let (img, sh, rows, tx) = (img.clone(), sh.clone(), self.mask_size().0, self.tx.clone());
                                crate::sys::background(move || {
                                    let (mask, _) = detect_sky(&img, MASK_COLS, Some(rows), 0.01);
                                    let pct = 100 * mask.count() / mask.len().max(1);
                                    let _ = tx.send(Msg::Sky(mask, format!("Sky detected in {}: {pct} % of the frame (hatched = foreground). Approximate: correct it with the brush – left button paints sky, right erases, wheel = brush size.", basename(&sh))));
                                });
                            }
                            _ => self.status = "Add light frames first: the sky is detected in the shown frame.".into(),
                        }
                    }
                    if ui.button("Reset sky region").clicked() {
                        self.cfg.sky.line = None;
                        self.cfg.sky.mask = Bits::new(0, 0);
                    }
                });
                note(ui, "Sky region");
            }
            Some(3) => {
                let c = &mut self.cfg;
                egui::Grid::new("sig").num_columns(2).show(ui, |ui| {
                    slider(ui, "Sigma low", &mut c.sigma.low, 1.0..=5.0, 0.1);
                    slider(ui, "Sigma high", &mut c.sigma.high, 1.0..=5.0, 0.1);
                    slider(ui, "Iterations", &mut c.sigma.iters, 1..=10, 1.0);
                });
                note(ui, "Sigma clipping");
            }
            Some(4) => {
                let c = &mut self.cfg;
                ui.checkbox(&mut c.reject.auto, "Reject bad frames automatically");
                egui::Grid::new("rej").num_columns(2).show(ui, |ui| {
                    let (mut l, mut st) = (c.reject.min_light * 100.0, c.reject.min_stars * 100.0);
                    if slider(ui, "Min. light (%)", &mut l, 10.0..=95.0, 5.0) {
                        c.reject.min_light = l / 100.0;
                    }
                    if slider(ui, "Min. stars (%)", &mut st, 10.0..=95.0, 5.0) {
                        c.reject.min_stars = st / 100.0;
                    }
                    slider(ui, "Max. FWHM (×)", &mut c.reject.max_fwhm, 1.1..=3.0, 0.05);
                    slider(ui, "Max. elongation (×)", &mut c.reject.max_elong, 1.05..=2.0, 0.05);
                });
                note(ui, "Frame rejection");
            }
            Some(5) => {
                let c = &mut self.cfg;
                egui::Grid::new("raw").num_columns(2).show(ui, |ui| {
                    combo(ui, "White balance", &mut c.raw.wb, &WB.map(|w| w.0), |w| WB.iter().find(|x| x.0 == w).map(|x| x.1).unwrap_or("?"));
                    let mut decs = vec![io::RawDecoder::Auto];
                    decs.extend(io::available_raw_decoders());
                    combo(ui, "Decoder", &mut c.raw.decoder, &decs, |d| d.name());
                    combo(ui, "Demosaicing", &mut c.raw.quality, &DEM.map(|d| d.0), |q| DEM.iter().find(|d| d.0 == q).map(|d| d.1).unwrap_or("?"));
                    ui.label("");
                    ui.checkbox(&mut c.raw.half, "Half size");
                    ui.end_row();
                });
                note(ui, "RAW conversion");
            }
            Some(6) => {
                ui.checkbox(&mut self.cfg.dynamic_noise, "Reduce dynamic noises");
                note(ui, "Reduce dynamic noises");
            }
            Some(7) => {
                egui::Grid::new("dist").num_columns(2).show(ui, |ui| {
                    combo(ui, "Correction", &mut self.cfg.distortion, Distortion::ALL, |v| v.name());
                });
                note(ui, "Reduce distor. effects");
            }
            Some(8) => {
                ui.checkbox(&mut self.cfg.merge4, "Merge 4 pixels");
                note(ui, "Merge 4 pixels");
            }
            Some(_) => {
                let c = &mut self.cfg;
                ui.checkbox(&mut c.timelapse.0, "Time-lapse");
                ui.add(egui::Slider::new(&mut c.timelapse.1, 2..=200).text("Frames per stack"));
                ui.label(format!("{nstars} light frames → {} frames", (nstars + 1).saturating_sub(c.timelapse.1)));
                note(ui, "Time-lapse");
            }
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            let big = |t: &str| egui::Button::new(egui::RichText::new(t).strong().size(15.0)).min_size(egui::vec2(180.0, 36.0));
            // While a run is going, its own button cancels it; the other one waits.
            let (rp, rs) = (self.running && self.running_preview, self.running && !self.running_preview);
            if ui.add_enabled(!rs, big(if rp { "Cancel" } else { "◐ Quick preview" })).on_hover_text(if rp { "Esc" } else { "Ctrl+P" }).clicked() {
                self.run(true);
            }
            if ui.add_enabled(!rp, big(if rs { "Cancel" } else { "▶ Start" })).on_hover_text(if rs { "Esc" } else { "Ctrl+Enter" }).clicked() {
                self.run(false);
            }
        });
    }

    fn right(&mut self, ui: &mut egui::Ui) {
        ui.heading("Post-processing");
        ui.weak("Applied to the result at once. Start saves with these settings; Save result… saves again.");
        let simple = self.simple;
        let mut changed: Option<&'static str> = None;
        let c = &mut self.cfg;
        let mut ch = |r: egui::Response, n: &'static str| {
            if r.changed() {
                changed = Some(n);
            }
        };
        if !simple {
            ch(ui.checkbox(&mut c.bg_extraction.0, "Background extraction"), "Background extraction");
            if c.bg_extraction.0 {
                ch(ui.add(egui::Slider::new(&mut c.bg_extraction.1, 1..=4).text("Degree")), "Background extraction");
            }
            ch(ui.checkbox(&mut c.color_calibration, "Color calibration"), "Color calibration");
            ch(ui.checkbox(&mut c.color_noise.0, "Reduce colour noise"), "Reduce colour noise");
            if c.color_noise.0 {
                ch(ui.add(egui::Slider::new(&mut c.color_noise.1, 5.0..=100.0).step_by(5.0).text("Strength")), "Reduce colour noise");
            }
        }
        let before = c.light_pollution.filter;
        egui::Grid::new("lp").num_columns(2).show(ui, |ui| {
            combo(ui, "Light pollution", &mut c.light_pollution.filter, Pollution::ALL, |v| v.name());
        });
        if before != c.light_pollution.filter {
            changed = Some("Reduce light pollution");
        }
        let mut ch = |r: egui::Response, n: &'static str| {
            if r.changed() {
                changed = Some(n);
            }
        };
        if c.light_pollution.filter != Pollution::Off {
            ch(ui.add(egui::Slider::new(&mut c.light_pollution.strength, 0.0..=100.0).step_by(1.0).text("Strength")), "Reduce light pollution");
            if !simple {
                if c.light_pollution.filter == Pollution::DeepSky {
                    ch(ui.add(egui::Slider::new(&mut c.light_pollution.degree, 1..=4).text("Degree (deep sky)")), "Reduce light pollution");
                } else {
                    ch(ui.add(egui::Slider::new(&mut c.light_pollution.tiles, 6..=48).text("Tiles (uneven)")), "Reduce light pollution");
                    ch(ui.checkbox(&mut c.light_pollution.iaa, "I.A.A. (also the foreground)"), "Reduce light pollution");
                }
                ch(ui.checkbox(&mut self.show_bg, "Show removed background"), "Reduce light pollution");
            }
        }
        if !simple {
            ch(ui.checkbox(&mut c.enhance_stars.0, "Enhance star light"), "Enhance star light");
            if c.enhance_stars.0 {
                ch(ui.add(egui::Slider::new(&mut c.enhance_stars.1, 0.0..=100.0).step_by(1.0).text("Strength")), "Enhance star light");
            }
        }
        ch(ui.checkbox(&mut c.auto_brightness, "Auto brightness"), "Auto brightness");
        if !simple {
            ch(ui.checkbox(&mut c.hdr, "High dynamic range"), "High dynamic range");
        }
        let (t0, c0) = (c.tone, c.color_space);
        egui::Grid::new("tone").num_columns(2).show(ui, |ui| {
            combo(ui, "Tone", &mut c.tone, Tone::ALL, |v| v.name());
            combo(ui, "Color space", &mut c.color_space, ColorSpace::ALL, |v| v.name());
        });
        if t0 != c.tone {
            changed = Some("Tone");
        }
        if c0 != c.color_space {
            changed = Some("Color space");
        }
        if c.tone == Tone::Stretch && !simple {
            if let Some(p) = &self.post {
                super::stitch::histogram(ui, &p.linear, Some((c.stretch.black, c.stretch.mid, c.stretch.white)));
            }
            ui.checkbox(&mut c.stretch.auto, "Automatic stretch");
            let mut moved = false;
            moved |= ui.add(egui::Slider::new(&mut c.stretch.black, 0.0..=0.5).step_by(0.0005).text("Black point")).changed();
            moved |= ui.add(egui::Slider::new(&mut c.stretch.mid, 0.0005..=0.9995).step_by(0.0005).text("Midtones")).changed();
            moved |= ui.add(egui::Slider::new(&mut c.stretch.white, 0.5..=1.0).step_by(0.0005).text("White point")).changed();
            if moved {
                c.stretch.auto = false; // moving a stretch slider: by hand from now on
                changed = Some("Tone");
            }
        }
        if let Some(n) = changed {
            self.post_note = super::notes::note(n);
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Linear (for processing)").clicked() {
                (self.cfg.bg_extraction.0, self.cfg.color_calibration, self.cfg.color_noise.0, self.cfg.enhance_stars.0, self.cfg.auto_brightness, self.cfg.hdr) = (false, false, false, false, false, false);
                self.cfg.light_pollution.filter = Pollution::Off;
                self.cfg.color_space = ColorSpace::Srgb;
                self.cfg.tone = Tone::Linear;
                if !self.cfg.output.is_empty() && !io::is_astro(&self.cfg.output) {
                    self.cfg.output = std::path::Path::new(&self.cfg.output).with_extension("fits").to_string_lossy().to_string();
                }
                self.post_note = super::notes::note("Linear");
                self.status = format!("Linear output: no post-processing, no brightness change, no tone curve{}", if self.cfg.output.is_empty() { ".".to_string() } else { format!("; output {} (32-bit).", basename(&self.cfg.output)) });
            }
            if ui.button("⬇ Save result…").clicked() && self.dialog.is_none() {
                match &self.result {
                    None => self.status = "Nothing to save yet: press Start.".into(),
                    Some(r) if r.preview => self.status = "This is a quick preview (small); press Start for the full-size result.".into(),
                    _ => {
                        let d = if self.cfg.output.is_empty() { "stacked.tif".into() } else { self.cfg.output.clone() };
                        self.dialog = Some((dialogs::pick_save("Save result", &d), "result"));
                    }
                }
            }
        });
        if ui.button("⇄ Use in the Stitcher (panorama panel)").clicked() {
            match &self.result {
                Some(r) if !r.preview && r.saved.len() == 1 => {
                    self.to_stitcher.push(r.saved[0].clone());
                    self.status = format!("{} added to the Stitcher. Stack the next panel here (same settings, a new output name), add it too, then stitch them there.", basename(&r.saved[0]));
                }
                Some(r) if r.saved.len() > 1 => self.status = "This run saved several files (time-lapse / align only); the Stitcher takes one stacked panel.".into(),
                _ => self.status = "Press Start first: the saved result goes to the Stitcher.".into(),
            }
        }
        ui.label(egui::RichText::new(self.post_note).weak().size(12.5));
    }

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            for (k, l) in [(ViewKind::Base, group_name(1, self.simple)), (ViewKind::Result, "Result"), (ViewKind::Report, "Frame report")] {
                if ui.selectable_label(self.view == k, l).clicked() {
                    self.view = k;
                    self.vstate.fit = true;
                }
            }
            if ui.button("Fit").on_hover_text("F").clicked() {
                self.vstate.fit = true;
            }
            ui.add_space(30.0);
            if ui.button("⇪ Open project…").on_hover_text("Ctrl+O").clicked() {
                self.ask_open_project();
            }
            if ui.button("⬇ Save project…").on_hover_text("Ctrl+S saves to the open project at once").clicked() {
                self.ask_save_project(false);
            }
        });
        let avail = ui.available_size();
        ui.allocate_ui(egui::vec2(avail.x, (avail.y - 60.0).max(100.0)), |ui| match self.view {
            ViewKind::Base => self.show_base(ui, ctx),
            ViewKind::Result => self.show_result(ui, ctx),
            ViewKind::Report => self.show_report(ui),
        });
        let col = if ui.visuals().dark_mode { Color32::from_rgb(255, 160, 128) } else { Color32::from_rgb(140, 0, 0) };
        if self.running {
            let p = self.progress.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let f = if p.total > 0.0 { (p.done / p.total) as f32 } else { 0.0 };
            let left = match p.start {
                Some(t) if f > 0.03 && f < 1.0 => {
                    let s = t.elapsed().as_secs_f64() * (1.0 - f as f64) / f as f64;
                    if s < 60.0 { format!(" – about {:.0} s left", s.max(1.0)) } else { format!(" – about {} min {:02} s left", (s / 60.0) as u64, (s % 60.0) as u64) }
                }
                _ => String::new(),
            };
            ui.add(egui::ProgressBar::new(f).show_percentage().text(format!("{}{left}", p.msg)));
        } else {
            egui::ScrollArea::vertical().id_salt("kstatus").show(ui, |ui| ui.colored_label(col, &self.status));
        }
    }

    fn show_base(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(disp) = self.disp.clone() else {
            ui.centered_and_justified(|ui| ui.weak("Base frame preview: add light frames."));
            return;
        };
        ui.label(format!("{}{}", if self.shown == self.base() { "Base frame: " } else { "Preview: " }, self.shown.as_deref().map(basename).unwrap_or_default()));
        if self.disp_tex.is_none() {
            let img = if self.aux { disp.map(|c| [(3.0 * c[0].max(0.0).powf(0.7)).min(1.0), (3.0 * c[1].max(0.0).powf(0.7)).min(1.0), (3.0 * c[2].max(0.0).powf(0.7)).min(1.0)]) } else { (*disp).clone() };
            self.disp_tex = Some(texture(ctx, "base", &img));
        }
        // Overlay: hatching over everything that is not sky.
        let (mh, mw) = self.mask_size();
        let sky = &self.cfg.sky;
        let okey = format!("{:?}{:?}{}{}{}{}", sky.mode, sky.line, sky.gradient, sky.feather, hash_str(&format!("{:?}", sky.mask.v)), mh);
        if self.overlay_tex.as_ref().is_none_or(|(k, _)| *k != okey) {
            let wgt = if sky.mode == SkyMode::IrregularMask && (sky.mask.h, sky.mask.w) == (mh, mw) && sky.mask.any() {
                mask_weight(&sky.mask, sky.feather)
            } else if matches!(sky.mode, SkyMode::BoundaryLine | SkyMode::Gradient) {
                sky_weight(sky, mh, mw)
            } else if sky.mode == SkyMode::IrregularMask {
                Gray::new(mh, mw)
            } else {
                Gray::filled(mh, mw, 1.0)
            };
            let mut rgba = Vec::with_capacity(mh * mw * 4);
            for r in 0..mh {
                for cc in 0..mw {
                    let w = wgt.at(r, cc);
                    let hatch = w < 0.999 && (r + cc) % (if w > 0.0 { 18 } else { 9 }) < 2;
                    rgba.extend_from_slice(&if hatch { [204, 217, 255, 140] } else { [0, 0, 0, 0] });
                }
            }
            let t = ctx.load_texture("overlay", egui::ColorImage::from_rgba_unmultiplied([mw, mh], &rgba), egui::TextureOptions::NEAREST);
            self.overlay_tex = Some((okey, t));
        }
        let (w, h) = (disp.w as f64, disp.h as f64);
        let (bt, ot) = (self.disp_tex.clone().unwrap(), self.overlay_tex.as_ref().unwrap().1.clone());
        let b = [0.5, w + 0.5, 0.5, h + 0.5];
        let mode = self.cfg.sky.mode;
        let drawing = matches!(mode, SkyMode::BoundaryLine | SkyMode::Gradient | SkyMode::IrregularMask);
        // Irregular mask: the mouse wheel sets the brush size (instead of zooming).
        if mode == SkyMode::IrregularMask && ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                self.brush = (self.brush * 1.15f64.powf((dy / 50.0) as f64)).clamp(0.5, 30.0);
                ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
        let s = image_view(ui, &mut self.vstate, (w, h), &[(&bt, b), (&ot, b)], !drawing);
        let p = ui.painter().with_clip_rect(s.rect);
        let to_uv = |pos: Pos2| { let (x, y) = s.to_image(pos); ((x - 0.5) / w, (y - 0.5) / h) };
        if let Some((u1, v1, u2, v2)) = self.cfg.sky.line {
            if (u2 - u1).abs() >= 0.02 && matches!(mode, SkyMode::BoundaryLine | SkyMode::Gradient) {
                let vat = |u: f64| v1 + (u - u1) * (v2 - v1) / (u2 - u1);
                let offs: &[f64] = if mode == SkyMode::Gradient { &[0.0, self.cfg.sky.gradient] } else { &[0.0] };
                for dv in offs {
                    p.line_segment([s.to_screen(0.5, (vat(0.0) + dv) * h + 0.5), s.to_screen(w + 0.5, (vat(1.0) + dv) * h + 0.5)], Stroke::new(2.0, Color32::from_rgb(255, 165, 0)));
                }
            }
        }
        if let Some(pos) = s.resp.hover_pos() {
            if mode == SkyMode::IrregularMask {
                p.circle_stroke(pos, (self.brush / 100.0 * w) as f32 * s.zoom, Stroke::new(1.5, Color32::LIGHT_BLUE));
            }
            let (u, v) = to_uv(pos);
            let (pd, sd) = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down()));
            match mode {
                SkyMode::BoundaryLine | SkyMode::Gradient => {
                    if s.resp.drag_started_by(egui::PointerButton::Primary) {
                        self.cfg.sky.line = Some((u, v, u, v));
                    } else if s.resp.dragged_by(egui::PointerButton::Primary) {
                        if let Some(l) = &mut self.cfg.sky.line {
                            (l.2, l.3) = (u, v);
                        }
                    }
                }
                SkyMode::IrregularMask if (pd || sd) && s.resp.is_pointer_button_down_on() => {
                    if (self.cfg.sky.mask.h, self.cfg.sky.mask.w) != (mh, mw) {
                        self.cfg.sky.mask = Bits::new(mh, mw);
                    }
                    let (cx, cy) = (u * mw as f64, v * mh as f64);
                    let rr = self.brush / 100.0 * mw as f64;
                    let m = &mut self.cfg.sky.mask;
                    for r in ((cy - rr).floor().max(0.0) as usize)..((cy + rr).ceil() as usize).min(mh) {
                        for cc in ((cx - rr).floor().max(0.0) as usize)..((cx + rr).ceil() as usize).min(mw) {
                            if (cc as f64 + 0.5 - cx).powi(2) + (r as f64 + 0.5 - cy).powi(2) <= rr * rr {
                                m.set(r, cc, pd);
                            }
                        }
                    }
                }
                SkyMode::None if s.resp.clicked() => self.status = "Choose a sky region mode (Sky region) to draw on the preview.".into(),
                _ => {}
            }
        }
        if self.show_stars {
            if let (Some(r), Some(sh)) = (&self.result, &self.shown) {
                if let Some(fi) = r.report.iter().find(|f| &f.path == sh) {
                    let (rh, rw) = (r.image.h as f64, r.image.w as f64);
                    for st in &fi.detected {
                        p.circle_stroke(s.to_screen((st.x - 0.5) * w / rw + 0.5, (st.y - 0.5) * h / rh + 0.5), 6.0, Stroke::new(1.2, Color32::from_rgba_unmultiplied(0, 255, 0, 220)));
                    }
                }
            }
        }
        let _ = (self.aux, ());
        if ui.ctx().input(|i| i.pointer.any_released()) {
            // nothing: strokes end on release
        }
    }

    fn show_result(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(r) = &self.result else {
            ui.centered_and_justified(|ui| ui.weak("Result: press Quick preview or Start."));
            return;
        };
        let showing_bg = self.show_bg && self.post.as_ref().is_some_and(|p| p.background.is_some());
        ui.label(format!("{}{}: {}", if showing_bg { "Removed background" } else { "Result" }, if r.preview { " (quick preview)" } else { "" }, r.saved.last().map(|s| basename(s)).unwrap_or_else(|| "not saved".into())));
        let key = self.post_key ^ (showing_bg as u64);
        if self.res_tex.as_ref().is_none_or(|(k, _)| *k != key) {
            let img = match &self.post {
                Some(p) if showing_bg => p.background.clone().unwrap(),
                Some(p) => p.image.clone(),
                None => r.image.clone(),
            };
            self.res_tex = Some((key, texture(ctx, "result", &img)));
        }
        let t = self.res_tex.as_ref().unwrap().1.clone();
        let (w, h) = (r.image.w as f64, r.image.h as f64);
        image_view(ui, &mut self.vstate, (w, h), &[(&t, [0.5, w + 0.5, 0.5, h + 0.5])], true);
    }

    fn show_report(&mut self, ui: &mut egui::Ui) {
        let Some(r) = self.result.clone() else {
            ui.weak("No report yet: press Quick preview or Start.");
            return;
        };
        ui.label("Frame report – click a frame (chart or table) to exclude / include it in the next run");
        // Chart: every measure ÷ its median over the frames used; left-out frames shaded.
        // Frames left out of a quick preview were not measured: they are only counted.
        let shown: Vec<FrameInfo> = r.report.iter().filter(|f| f.status != Status::Skipped).cloned().collect();
        let skipped = r.report.len() - shown.len();
        let infos = &shown;
        // Not measured (excluded by you, or not loadable): no points in the chart.
        let measured = |f: &FrameInfo| !(matches!(f.status, Status::Excluded | Status::Skipped) || (f.status == Status::Failed && f.stars == 0));
        let n = infos.len().max(1);
        let (resp, p) = ui.allocate_painter(egui::vec2(ui.available_width(), (ui.available_height() * 0.34).max(120.0)), egui::Sense::click());
        let rect = resp.rect;
        p.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
        let xat = |k: usize| rect.left() + (k as f32 + 0.5) / n as f32 * rect.width();
        let yat = |v: f64| rect.bottom() - (v.clamp(0.0, 2.5) / 2.6) as f32 * rect.height();
        for (k, fi) in infos.iter().enumerate() {
            let shade = match fi.status {
                Status::Rejected | Status::Failed => Some(Color32::from_rgba_unmultiplied(220, 50, 50, 50)),
                Status::Excluded | Status::Skipped => Some(Color32::from_rgba_unmultiplied(128, 128, 128, 50)),
                Status::Base => Some(Color32::from_rgba_unmultiplied(70, 110, 230, 50)),
                _ if self.cfg.excluded.contains(&fi.path) => Some(Color32::from_rgba_unmultiplied(128, 128, 128, 50)),
                _ => None,
            };
            if let Some(c) = shade {
                let x0 = rect.left() + k as f32 / n as f32 * rect.width();
                p.rect_filled(egui::Rect::from_min_max(Pos2::new(x0, rect.top()), Pos2::new(x0 + rect.width() / n as f32, rect.bottom())), 0.0, c);
            }
        }
        p.line_segment([Pos2::new(rect.left(), yat(1.0)), Pos2::new(rect.right(), yat(1.0))], Stroke::new(1.0, Color32::GRAY));
        let series: [(&str, fn(&FrameInfo) -> f64, Color32); 6] = [
            ("stars", |f| f.stars as f64, Color32::from_rgb(30, 144, 255)),
            ("FWHM", |f| f.fwhm, Color32::from_rgb(255, 165, 0)),
            ("elongation", |f| f.elong, Color32::from_rgb(147, 112, 219)),
            ("light", |f| f.light, Color32::GOLD),
            ("sky level", |f| f.background, Color32::GRAY),
            ("align error", |f| if f.status == Status::Base { f64::NAN } else { f.err }, Color32::from_rgb(255, 99, 71)),
        ];
        let mut lx = rect.left() + 6.0;
        for (label, get, col) in series {
            let ok: Vec<f64> = infos.iter().filter(|f| f.used()).map(get).filter(|v| v.is_finite() && *v > 0.0).collect();
            if ok.is_empty() {
                continue;
            }
            let m = crate::image::median_f64(ok);
            if m <= 0.0 {
                continue;
            }
            let pts: Vec<Pos2> = infos.iter().enumerate().filter(|(_, f)| measured(f)).filter_map(|(k, f)| { let v = get(f); v.is_finite().then(|| Pos2::new(xat(k), yat(v / m))) }).collect();
            p.line(pts.clone(), Stroke::new(1.5, col));
            for q in pts {
                p.circle_filled(q, 3.0, col);
            }
            let g = p.text(Pos2::new(lx, rect.top() + 4.0), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(11.0), col);
            lx = g.right() + 10.0;
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let k = (((pos.x - rect.left()) / rect.width()) * n as f32).floor() as usize;
                if let Some(fi) = infos.get(k) {
                    let path = fi.path.clone();
                    self.toggle_exclude(&path);
                }
            }
        }
        let text = report_text(infos);
        let lines: Vec<&str> = text.lines().collect();
        egui::ScrollArea::vertical().id_salt("rep").show(ui, |ui| {
            ui.monospace(lines[0]);
            for (k, fi) in infos.iter().enumerate() {
                let col = match fi.status {
                    Status::Base => Color32::from_rgb(80, 120, 230),
                    Status::Used if !self.cfg.excluded.contains(&fi.path) => Color32::from_rgb(0, 150, 40),
                    Status::Rejected | Status::Failed => Color32::from_rgb(200, 40, 40),
                    _ => Color32::GRAY,
                };
                let mut t = lines[k + 1].to_string();
                let excl = self.cfg.excluded.contains(&fi.path);
                if excl && fi.status != Status::Excluded {
                    t += "  → excluded next run";
                } else if !excl && fi.status == Status::Excluded {
                    t += "  → included next run";
                }
                if ui.add(egui::Label::new(egui::RichText::new(t).monospace().color(col)).sense(egui::Sense::click())).clicked() {
                    let path = fi.path.clone();
                    self.toggle_exclude(&path);
                }
            }
            if skipped > 0 {
                ui.weak(format!("{skipped} more light frames were not in the quick preview (it uses up to {PREVIEW_FRAMES})."));
            }
            ui.add_space(8.0);
            ui.label("Process log (time per step):");
            for l in &r.log {
                ui.monospace(l);
            }
        });
    }
}

/// Hooks for the headless window tests (tests/gui.rs).
#[doc(hidden)]
impl StackTab {
    pub fn t_status(&self) -> &str {
        &self.status
    }
    pub fn t_add(&mut self, group: usize, ps: Vec<String>) {
        self.group = group;
        self.setting = None;
        self.add(ps);
    }
    pub fn t_cfg(&mut self) -> &mut StackConfig {
        &mut self.cfg
    }
    pub fn t_files(&self) -> &[Vec<String>; 7] {
        &self.files
    }
    pub fn t_run(&mut self, preview: bool) {
        self.run(preview);
    }
    pub fn t_result(&self) -> Option<&StackResult> {
        self.result.as_ref()
    }
    pub fn t_post_ready(&self) -> bool {
        self.post.is_some() && !self.post_busy
    }
    pub fn t_exclude(&mut self, p: &str) {
        self.toggle_exclude(p);
    }
    pub fn t_shown(&self) -> Option<&str> {
        self.shown.as_deref()
    }
    pub fn t_base(&self) -> Option<String> {
        self.base()
    }
    pub fn t_brush(&self) -> f64 {
        self.brush
    }
}

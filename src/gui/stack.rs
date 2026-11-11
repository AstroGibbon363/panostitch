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
use egui::{Color32, Pos2, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

const GROUPS: [&str; 8] = ["Star images", "Base image", "Noise images", "Bias images", "Vignetting images", "Flat-dark images", "Foreground image", "Output"];
const MASK_COLS: usize = 400;
const PREVIEW_MAX: usize = 1600;
const RESULT_MAX: usize = 3000;

enum Msg {
    Preview(String, Arc<Img>),
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
    progress: Arc<Mutex<String>>,
    status: String,
    pathbox: String,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    dialog: Option<(Receiver<Picked>, &'static str)>,
}

impl Default for StackTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        let mut cfg = StackConfig::default();
        cfg.sky.gradient = 0.1;
        Self {
            to_stitcher: vec![], files: Default::default(), group: 0, cfg, shown: None, disp: None, disp_tex: None, overlay_tex: None,
            aux: false, show_stars: false, brush: 5.0, view: ViewKind::Base, vstate: View::default(), result: None, raw_small: None,
            post: None, post_key: 0, post_busy: false, show_bg: false, res_tex: None, running: false, loading: false,
            cancel: Arc::new(AtomicBool::new(false)), progress: Arc::new(Mutex::new(String::new())),
            status: "Add star images to begin. Choose a group, then Add… (or type a path).".into(), pathbox: String::new(), tx, rx, dialog: None,
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
        self.status = format!("Added {n} file(s) to {}.", GROUPS[g]);
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
        std::thread::spawn(move || {
            let r = io::load_any(&p, &io::LoadOptions { half: true, ..Default::default() }).map(|a| {
                let f = PREVIEW_MAX as f64 / a.w.max(a.h) as f64;
                if f < 1.0 { resize(&a, f) } else { a }
            });
            let _ = tx.send(match r {
                Ok(img) => Msg::Preview(p, Arc::new(img)),
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
            self.status = "Add star images first.".into();
            return;
        }
        if !preview && self.cfg.output.is_empty() {
            self.status = "Set the output file first (Output).".into();
            return;
        }
        let cfg = self.settings();
        self.running = true;
        self.cancel.store(false, Ordering::Relaxed);
        let (tx, cancel, prog) = (self.tx.clone(), self.cancel.clone(), self.progress.clone());
        *prog.lock().unwrap() = "Starting…".into();
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let progress = |m: &str| -> Result<(), String> {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled.".into());
                }
                *prog.lock().unwrap() = m.to_string();
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

    fn post_fingerprint(&self) -> u64 {
        let c = &self.cfg;
        hash_str(&format!("{:?}{:?}{:?}{:?}{:?}{:?}{}{}{:?}{:?}{:?}{:?}", c.bg_extraction, c.color_calibration, c.color_noise, c.light_pollution, c.enhance_stars, c.auto_brightness, c.hdr, c.tone.name(), c.stretch, c.color_space, c.sky.mode, self.raw_small.as_ref().map(|r| r.0.w)))
    }

    fn poll(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Preview(p, img) => {
                    self.loading = false;
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
            self.status = self.progress.lock().unwrap().clone();
        }
        // Live post-processing of the screen-size stack.
        if let Some((raw, w, fw)) = &self.raw_small {
            let key = self.post_fingerprint();
            if key != self.post_key && !self.post_busy {
                self.post_busy = true;
                let (raw, w, fw, cfg, tx) = (raw.clone(), w.clone(), *fw, self.cfg.clone(), self.tx.clone());
                std::thread::spawn(move || {
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
                    Picked::Files(ps) => self.add(ps),
                    Picked::Save(Some(p)) if what == "output" => self.cfg.output = p,
                    Picked::Save(Some(p)) if what == "result" => self.save_result(p),
                    Picked::Save(_) => {}
                }
            }
        }
    }

    fn save_result(&mut self, p: String) {
        let Some(r) = self.result.clone() else { return };
        let mut cfg = self.settings();
        cfg.output = p.clone();
        let tx = self.tx.clone();
        self.status = format!("Saving {}…", basename(&p));
        std::thread::spawn(move || {
            let _ = tx.send(match save_result(&r, &cfg) {
                Ok(_) => Msg::Status(format!("Saved {p}{}", if io::is_astro(&p) { " (linear 32-bit)." } else { "." })),
                Err(e) => Msg::Error(e),
            });
        });
    }

    fn toggle_exclude(&mut self, path: &str) {
        if Some(path.to_string()) == self.base() || self.result.as_ref().is_some_and(|r| r.base == path) {
            self.status = "The base image cannot be excluded (choose another base first).".into();
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

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.poll();
        egui::Panel::left("stack_left").exact_size(390.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("kl").show(ui, |ui| self.left(ui));
        });
        egui::Panel::right("stack_right").exact_size(330.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("kr").show(ui, |ui| self.right(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.center(ui, ctx));
    }

    fn left(&mut self, ui: &mut egui::Ui) {
        ui.heading("Input");
        egui::ScrollArea::vertical().id_salt("tree").max_height(200.0).show(ui, |ui| {
            let base = self.base();
            let mut clicked: Option<String> = None;
            for g in 0..8 {
                let n = if g < 7 { self.files[g].len() } else { usize::from(!self.cfg.output.is_empty()) };
                let dot = if (g == 0 || g == 7) && n == 0 { Color32::RED } else if n > 0 { Color32::from_rgb(0, 170, 0) } else { Color32::GRAY };
                ui.horizontal(|ui| {
                    ui.colored_label(dot, "●");
                    let t = if [0, 2, 3, 4, 5].contains(&g) && n > 0 { format!("{} ({n})", GROUPS[g]) } else { GROUPS[g].to_string() };
                    if ui.selectable_label(self.group == g, egui::RichText::new(t).strong()).clicked() {
                        self.group = g;
                    }
                });
                let rows: Vec<String> = match g {
                    7 => if self.cfg.output.is_empty() { vec![] } else { vec![self.cfg.output.clone()] },
                    1 if self.files[1].is_empty() => base.iter().map(|b| format!("(auto) {}", basename(b))).collect(),
                    _ => self.files[g].clone(),
                };
                for f in rows {
                    let mut t = format!("      {}", if f.starts_with("(auto)") { f.clone() } else { basename(&f) });
                    if g == 0 && self.files[1].contains(&f) {
                        t += "  ← base";
                    }
                    if g == 0 && self.cfg.excluded.contains(&f) {
                        t += "  ✗ excluded";
                    }
                    let sel = self.shown.as_deref() == Some(f.as_str());
                    if ui.selectable_label(sel, t).clicked() {
                        self.group = g;
                        if g < 7 && !f.starts_with("(auto)") {
                            clicked = Some(f.clone());
                        } else if g == 1 {
                            clicked = base.clone();
                        }
                    }
                }
            }
            if let Some(f) = clicked {
                self.view = ViewKind::Base;
                self.show_preview(Some(f));
            }
        });
        ui.horizontal(|ui| {
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
                    self.status = "Click a file of the chosen group first (Clear removes the whole group).".into();
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
                        self.status = format!("Base image: {}.", basename(&s));
                    }
                    None => self.status = "Click one of the star images first.".into(),
                }
            }
        });
        let r = ui.add(egui::TextEdit::singleline(&mut self.pathbox).hint_text("…or type a file/folder path + Enter").desired_width(370.0));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.pathbox.trim().is_empty() {
            let ps = if self.group == 7 { vec![self.pathbox.trim().to_string()] } else { dialogs::expand_paths(&self.pathbox) };
            self.pathbox.clear();
            self.add(ps);
        }
        if self.group == 7 {
            ui.horizontal(|ui| {
                ui.label("Output");
                ui.add(egui::TextEdit::singleline(&mut self.cfg.output).desired_width(300.0));
            });
            ui.weak("TIFF: 16-bit, as shown. JPEG: 8-bit. FITS / XISF: 32-bit float, linear (for Siril, PixInsight …).");
        }
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
            _ => {}
        }
        ui.separator();
        let c = &mut self.cfg;
        egui::CollapsingHeader::new("Composition").default_open(true).show(ui, |ui| {
            egui::Grid::new("compo").num_columns(2).show(ui, |ui| {
                combo(ui, "Composition", &mut c.composition, Composition::ALL, |v| v.name());
                if c.composition == Composition::AlignStars {
                    combo(ui, "Stacking", &mut c.align_mode, AlignMode::ALL, |v| v.name());
                    combo(ui, "Model", &mut c.model, &[Model::Homography, Model::Similarity], |v| v.name());
                    combo(ui, "Interpolation", &mut c.interpolation, Interpolation::ALL, |v| v.name());
                    ui.label("");
                    ui.checkbox(&mut c.selective, "Selective (sigma clipping in the sky)");
                    ui.end_row();
                } else {
                    combo(ui, "Trail fade", &mut c.trails.0, Fade::ALL, |v| v.name());
                    ui.label("");
                    ui.checkbox(&mut c.trails.1, "Fill gaps between frames");
                    ui.end_row();
                }
            });
        });
        egui::CollapsingHeader::new("Sky region").default_open(true).show(ui, |ui| {
            egui::Grid::new("sky").num_columns(2).show(ui, |ui| {
                combo(ui, "Sky region", &mut self.cfg.sky.mode, SkyMode::ALL, |v| v.name());
                if self.cfg.sky.mode == SkyMode::Gradient {
                    let mut g = self.cfg.sky.gradient * 100.0;
                    if slider(ui, "Gradient (% height)", &mut g, 1.0..=50.0, 1.0) {
                        self.cfg.sky.gradient = g / 100.0;
                    }
                }
                if self.cfg.sky.mode == SkyMode::IrregularMask {
                    slider(ui, "Brush size (% width)", &mut self.brush, 0.5..=30.0, 0.5);
                    let mut f = self.cfg.sky.feather * 100.0;
                    if slider(ui, "Mask feather (% width)", &mut f, 0.0..=10.0, 0.5) {
                        self.cfg.sky.feather = f / 100.0;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.aux, "Auxiliary highlight");
                ui.checkbox(&mut self.show_stars, "Show detected stars");
            });
            ui.horizontal(|ui| {
                if ui.button("Detect sky").clicked() {
                    match (&self.disp, &self.shown) {
                        (Some(img), Some(sh)) if !self.loading => {
                            self.loading = true;
                            let (img, sh, rows, tx) = (img.clone(), sh.clone(), self.mask_size().0, self.tx.clone());
                            std::thread::spawn(move || {
                                let (mask, _) = detect_sky(&img, MASK_COLS, Some(rows), 0.01);
                                let pct = 100 * mask.count() / mask.len().max(1);
                                let _ = tx.send(Msg::Sky(mask, format!("Sky detected in {}: {pct} % of the frame (hatched = foreground). Approximate: correct it with the brush – left button paints sky, right erases.", basename(&sh))));
                            });
                        }
                        _ => self.status = "Add star images first: the sky is detected in the shown frame.".into(),
                    }
                }
                if ui.button("Reset sky region").clicked() {
                    self.cfg.sky.line = None;
                    self.cfg.sky.mask = Bits::new(0, 0);
                }
            });
        });
        let c = &mut self.cfg;
        egui::CollapsingHeader::new("Sigma clipping").show(ui, |ui| {
            egui::Grid::new("sig").num_columns(2).show(ui, |ui| {
                slider(ui, "Sigma low", &mut c.sigma.low, 1.0..=5.0, 0.1);
                slider(ui, "Sigma high", &mut c.sigma.high, 1.0..=5.0, 0.1);
                slider(ui, "Iterations", &mut c.sigma.iters, 1..=10, 1.0);
            });
        });
        egui::CollapsingHeader::new("Frame rejection").show(ui, |ui| {
            ui.checkbox(&mut c.reject.auto, "Reject bad frames automatically");
            egui::Grid::new("rej").num_columns(2).show(ui, |ui| {
                slider(ui, "Min. light (×)", &mut c.reject.min_light, 0.1..=0.95, 0.05);
                slider(ui, "Min. stars (×)", &mut c.reject.min_stars, 0.1..=0.95, 0.05);
                slider(ui, "Max. FWHM (×)", &mut c.reject.max_fwhm, 1.1..=3.0, 0.05);
                slider(ui, "Max. elongation (×)", &mut c.reject.max_elong, 1.05..=2.0, 0.05);
            });
        });
        egui::CollapsingHeader::new("RAW conversion").show(ui, |ui| {
            egui::Grid::new("raw").num_columns(2).show(ui, |ui| {
                combo(ui, "White balance", &mut c.raw.wb, &[io::WhiteBalance::Camera, io::WhiteBalance::Auto, io::WhiteBalance::Daylight], |w| w.name());
                let mut decs = vec![io::RawDecoder::Auto];
                decs.extend(io::available_raw_decoders());
                combo(ui, "Decoder", &mut c.raw.decoder, &decs, |d| d.name());
                const DEM: [(u32, &str); 7] = [(3, "AHD"), (11, "DHT"), (12, "AAHD"), (4, "DCB"), (2, "PPG"), (1, "VNG"), (0, "Bilinear")];
                combo(ui, "Demosaicing (dcraw_emu)", &mut c.raw.quality, &DEM.map(|d| d.0), |q| DEM.iter().find(|d| d.0 == q).map(|d| d.1).unwrap_or("?"));
                ui.label("");
                ui.checkbox(&mut c.raw.half, "Half size");
                ui.end_row();
            });
        });
        egui::CollapsingHeader::new("More").show(ui, |ui| {
            ui.checkbox(&mut c.dynamic_noise, "Reduce dynamic noises (hot pixels without darks)");
            egui::Grid::new("more").num_columns(2).show(ui, |ui| {
                combo(ui, "Reduce distortion", &mut c.distortion, Distortion::ALL, |v| v.name());
            });
            ui.checkbox(&mut c.merge4, "Merge 4 pixels");
            ui.checkbox(&mut c.timelapse.0, "Time-lapse");
            if c.timelapse.0 {
                ui.add(egui::Slider::new(&mut c.timelapse.1, 2..=200).text("Frames per stack"));
                ui.weak(format!("{} star images → {} frames", self.files[0].len(), (self.files[0].len() + 1).saturating_sub(c.timelapse.1)));
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            let big = |t: &str| egui::Button::new(egui::RichText::new(t).strong().size(15.0)).min_size(egui::vec2(180.0, 36.0));
            if ui.add(big(if self.running { "Cancel" } else { "◐ Quick preview" })).clicked() {
                self.run(true);
            }
            if ui.add(big(if self.running { "Cancel" } else { "▶ Start" })).clicked() {
                self.run(false);
            }
        });
    }

    fn right(&mut self, ui: &mut egui::Ui) {
        ui.heading("Post-processing");
        ui.weak("Applied to the result at once. Start saves with these settings; Save result… saves again.");
        let c = &mut self.cfg;
        ui.checkbox(&mut c.bg_extraction.0, "Background extraction");
        if c.bg_extraction.0 {
            ui.add(egui::Slider::new(&mut c.bg_extraction.1, 1..=4).text("Degree"));
        }
        ui.checkbox(&mut c.color_calibration, "Color calibration");
        ui.checkbox(&mut c.color_noise.0, "Reduce colour noise");
        if c.color_noise.0 {
            ui.add(egui::Slider::new(&mut c.color_noise.1, 5.0..=100.0).step_by(5.0).text("Strength"));
        }
        egui::Grid::new("lp").num_columns(2).show(ui, |ui| {
            combo(ui, "Light pollution", &mut c.light_pollution.filter, Pollution::ALL, |v| v.name());
        });
        if c.light_pollution.filter != Pollution::Off {
            ui.add(egui::Slider::new(&mut c.light_pollution.strength, 0.0..=100.0).step_by(1.0).text("Strength"));
            if c.light_pollution.filter == Pollution::DeepSky {
                ui.add(egui::Slider::new(&mut c.light_pollution.degree, 1..=4).text("Degree"));
            } else {
                ui.add(egui::Slider::new(&mut c.light_pollution.tiles, 6..=48).text("Tiles"));
                ui.checkbox(&mut c.light_pollution.iaa, "I.A.A. (also the foreground)");
            }
            ui.checkbox(&mut self.show_bg, "Show removed background");
        }
        ui.checkbox(&mut c.enhance_stars.0, "Enhance star light");
        if c.enhance_stars.0 {
            ui.add(egui::Slider::new(&mut c.enhance_stars.1, 0.0..=100.0).step_by(1.0).text("Strength"));
        }
        ui.checkbox(&mut c.auto_brightness, "Auto brightness");
        ui.checkbox(&mut c.hdr, "High dynamic range");
        egui::Grid::new("tone").num_columns(2).show(ui, |ui| {
            combo(ui, "Tone", &mut c.tone, Tone::ALL, |v| v.name());
            combo(ui, "Color space", &mut c.color_space, ColorSpace::ALL, |v| v.name());
        });
        if c.tone == Tone::Stretch {
            if let Some(p) = &self.post {
                super::stitch::histogram(ui, &p.linear, Some((c.stretch.black, c.stretch.mid, c.stretch.white)));
            }
            ui.checkbox(&mut c.stretch.auto, "Automatic stretch");
            let mut ch = false;
            ch |= ui.add(egui::Slider::new(&mut c.stretch.black, 0.0..=0.5).step_by(0.0005).text("Black point")).changed();
            ch |= ui.add(egui::Slider::new(&mut c.stretch.mid, 0.0005..=0.9995).step_by(0.0005).text("Midtones")).changed();
            ch |= ui.add(egui::Slider::new(&mut c.stretch.white, 0.5..=1.0).step_by(0.0005).text("White point")).changed();
            if ch {
                c.stretch.auto = false; // moving a stretch slider: by hand from now on
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Linear (for processing)").clicked() {
                (self.cfg.bg_extraction.0, self.cfg.color_calibration, self.cfg.color_noise.0, self.cfg.enhance_stars.0, self.cfg.auto_brightness, self.cfg.hdr) = (false, false, false, false, false, false);
                self.cfg.light_pollution.filter = Pollution::Off;
                self.cfg.color_space = ColorSpace::Srgb;
                self.cfg.tone = Tone::Linear;
                if !self.cfg.output.is_empty() && !io::is_astro(&self.cfg.output) {
                    self.cfg.output = std::path::Path::new(&self.cfg.output).with_extension("fits").to_string_lossy().to_string();
                }
                self.status = "Linear output: no post-processing, no brightness change, no tone curve; output as 32-bit FITS.".into();
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
    }

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            for (k, l) in [(ViewKind::Base, "Base image"), (ViewKind::Result, "Result"), (ViewKind::Report, "Frame report")] {
                if ui.selectable_label(self.view == k, l).clicked() {
                    self.view = k;
                    self.vstate.fit = true;
                }
            }
            if ui.button("Fit").clicked() {
                self.vstate.fit = true;
            }
        });
        let avail = ui.available_size();
        ui.allocate_ui(egui::vec2(avail.x, (avail.y - 60.0).max(100.0)), |ui| match self.view {
            ViewKind::Base => self.show_base(ui, ctx),
            ViewKind::Result => self.show_result(ui, ctx),
            ViewKind::Report => self.show_report(ui),
        });
        let col = if ui.visuals().dark_mode { Color32::from_rgb(255, 160, 128) } else { Color32::from_rgb(140, 0, 0) };
        egui::ScrollArea::vertical().id_salt("kstatus").show(ui, |ui| ui.colored_label(col, &self.status));
    }

    fn show_base(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(disp) = self.disp.clone() else {
            ui.centered_and_justified(|ui| ui.weak("Base image preview: add star images."));
            return;
        };
        ui.label(format!("{}{}", if self.shown == self.base() { "Base image preview: " } else { "Preview: " }, self.shown.as_deref().map(basename).unwrap_or_default()));
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
        let infos = &r.report;
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
            let pts: Vec<Pos2> = infos.iter().enumerate().filter_map(|(k, f)| { let v = get(f); v.is_finite().then(|| Pos2::new(xat(k), yat(v / m))) }).collect();
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
            ui.add_space(8.0);
            ui.label("Process log (time per step):");
            for l in &r.log {
                ui.monospace(l);
            }
        });
    }
}

//! The "Post-processing" tab: the last steps on a stacked or stitched image
//! (turn and crop, stretch, white balance, tone, curves, colour, noise, stars,
//! sharpening) and saving it at the size you want. A small copy gives a live
//! preview; the full image is processed when saving.

use crate::gui::i18n::tr;
use super::dialogs::{self, Picked};
use super::mask::MaskEditor;
use super::{image_view, texture, View};
use crate::finish::{self, identity_curve, FinishParams, Local, Measured, StretchMode, ViewStretch};
use crate::image::Img;
use crate::io::{self, basename};
use crate::sys::gb;
use egui::{Color32, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// Longest side of the preview copy.
const PREVIEW_EDGE: usize = 2000;
/// Working memory per pixel when saving (several float copies of the image).
const SAVE_BYTES_PER_PX: f64 = 12.0 * 6.0;

struct Loaded {
    path: String,
    full: Arc<Img>,
    proxy: Arc<Img>,
    scale: f64,
}

struct Preview {
    params: FinishParams,
    /// Turned and cropped, before white balance and stretch: white balance samples come from here.
    prepared: Img,
    /// The starting point (turned, cropped, stretched; nothing else) for "Compare".
    before: Img,
    result: Img,
    measured: Measured,
    /// Histograms of what goes into the curves (brightness, red, green, blue).
    curve_hist: [Vec<f32>; 4],
    /// Black point and symmetry point at the sky background (linear data).
    background: (f64, f64),
    /// The sky and foreground looks applied (None: not used).
    local: Option<Local>,
}

enum Msg {
    Loaded(u64, Result<Loaded, String>),
    Preview(u64, Result<Box<Preview>, String>),
    Saved(Result<(String, Box<Settings>), String>),
}

/// All settings of a result: the adjustments and the sky / foreground looks.
type Settings = (FinishParams, Option<Local>);
/// What undo and redo restore.
type Snap = (FinishParams, Local);

/// A saved look: everything but turning, cropping and the export size.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Preset {
    look: FinishParams,
    sky: FinishParams,
    ground: FinishParams,
    feather: f64,
}

/// Which part of the crop frame a drag moves.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum CropGrip {
    New,
    Move,
    /// Left, top, right, bottom edges.
    Edges([bool; 4]),
}

pub struct FinishTab {
    pub simple: bool,
    path: String,
    path_edit: String,
    full: Option<Arc<Img>>,
    proxy: Option<Arc<Img>>,
    scale: f64,
    linear: bool,
    pub p: FinishParams,
    /// Keep the settings when another image is opened (same look for several panels).
    keep: bool,
    display: ViewStretch,
    /// Save the linear data (geometry, background, colour and white balance only) instead of the finished image.
    save_linear: bool,
    shown: Option<Preview>,
    tex: Option<egui::TextureHandle>,
    before_tex: Option<egui::TextureHandle>,
    tex_display: ViewStretch,
    gen: u64,
    loading: bool,
    previewing: bool,
    saving: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    view: View,
    status: String,
    out: String,
    /// Settings of the last save.
    saved: Option<Settings>,
    /// The file being saved, and the last one saved (for "Show in folder").
    saving_to: Option<String>,
    last_saved: Option<String>,
    /// The sky / foreground mask and looks.
    mask: MaskEditor,
    /// Which of them the sliders show: 0 sky, 1 foreground.
    local_k: usize,
    // Undo / redo: the last recorded state, a change waiting to settle, the steps.
    committed: Option<Snap>,
    pending: Option<(Snap, std::time::Instant)>,
    undo: Vec<(Snap, String)>,
    redo: Vec<(Snap, String)>,
    // Presets.
    presets: Option<Vec<String>>,
    preset_name: String,
    // Cropping on the image: on, and the drag going on (grip, frame at its start, start point).
    cropping: bool,
    crop_drag: Option<(CropGrip, [f64; 4], (f64, f64))>,
    dialog: Option<(Receiver<Picked>, &'static str)>,
    /// White balance sampling: on, and the rectangle being dragged (image coordinates).
    sampling: bool,
    sample_from: Option<(f64, f64)>,
    curve_ch: usize,
}

impl Default for FinishTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            simple: false,
            path: String::new(),
            path_edit: String::new(),
            full: None,
            proxy: None,
            scale: 1.0,
            linear: false,
            p: FinishParams::default(),
            keep: false,
            display: ViewStretch::default(),
            save_linear: false,
            shown: None,
            tex: None,
            before_tex: None,
            tex_display: ViewStretch::default(),
            gen: 0,
            loading: false,
            previewing: false,
            saving: false,
            tx,
            rx,
            view: View::default(),
            status: tr("Open an image (Ctrl+O), or send a result here from the other tabs.").into(),
            out: String::new(),
            saved: None,
            saving_to: None,
            last_saved: None,
            dialog: None,
            sampling: false,
            sample_from: None,
            curve_ch: 0,
            mask: MaskEditor::new(false),
            local_k: 0,
            committed: None,
            pending: None,
            undo: Vec::new(),
            redo: Vec::new(),
            presets: None,
            preset_name: String::new(),
            cropping: false,
            crop_drag: None,
        }
    }
}

fn panic_text() -> String {
    crate::trf!("internal error (details in {})", super::crash_log().display())
}

impl FinishTab {
    pub fn busy(&self) -> bool {
        self.loading || self.previewing || self.saving
    }
    pub fn has_image(&self) -> bool {
        self.full.is_some()
    }
    pub fn saving(&self) -> bool {
        self.saving
    }
    /// Changes not saved to a file yet.
    pub fn unsaved(&self) -> bool {
        let now = (self.p.clone(), self.local());
        self.full.is_some() && self.saved.as_ref() != Some(&now) && now != (FinishParams::for_input(self.linear), None)
    }

    /// The sky and foreground looks, if they change anything.
    fn local(&self) -> Option<Local> {
        self.mask.local.used().then(|| self.mask.local.clone())
    }

    /// Load an image (in the background).
    /// Files dropped on the window: the first image opens.
    pub fn add_dropped(&mut self, ps: Vec<String>) {
        if let Some(p) = ps.into_iter().find(|p| io::is_image(p)) {
            self.open(&p);
        }
    }

    pub fn open(&mut self, path: &str) {
        let path = path.trim().to_string();
        if path.is_empty() {
            return;
        }
        if self.loading || self.saving {
            self.status = tr("Wait until the current image is loaded or saved.").into();
            return;
        }
        self.loading = true;
        self.gen += 1;
        self.status = crate::trf!("Loading {} …", basename(&path));
        let (tx, gen) = (self.tx.clone(), self.gen);
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(|| -> Result<Loaded, String> {
                let full = io::load_any(&path, &io::LoadOptions::default())?;
                let long = full.w.max(full.h);
                let scale = if long > PREVIEW_EDGE { PREVIEW_EDGE as f64 / long as f64 } else { 1.0 };
                let proxy = if scale < 1.0 { crate::image::resize(&full, scale) } else { full.clone() };
                Ok(Loaded { path, full: Arc::new(full), proxy: Arc::new(proxy), scale })
            })
            .unwrap_or_else(|_| Err(panic_text()));
            let _ = tx.send(Msg::Loaded(gen, r));
        });
    }

    fn default_out(path: &str, linear: bool) -> String {
        let p = std::path::Path::new(path);
        let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "image".into());
        dir.join(format!("{stem}_final.{}", if linear { "tif" } else { "jpg" })).to_string_lossy().to_string()
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Loaded(gen, r) if gen == self.gen => {
                    self.loading = false;
                    match r {
                        Ok(l) => {
                            self.linear = io::is_astro(&l.path);
                            if !self.keep || self.full.is_none() {
                                self.p = FinishParams::for_input(self.linear);
                                self.mask.local = Local::default();
                            }
                            // The mask lies on the image; another shape needs a new one.
                            self.mask.fit(l.proxy.w, l.proxy.h);
                            self.status = crate::trf!("Opened {} ({}×{} px){}. Adjust on the left, save on the right.", basename(&l.path), l.full.w, l.full.h, if self.linear { tr(", linear data: stretched automatically") } else { "" });
                            self.out = Self::default_out(&l.path, self.linear);
                            self.path_edit = l.path.clone();
                            self.path = l.path;
                            self.full = Some(l.full);
                            self.proxy = Some(l.proxy);
                            self.scale = l.scale;
                            self.shown = None;
                            self.tex = None;
                            self.before_tex = None;
                            self.saved = None;
                            self.view.fit = true;
                            self.undo.clear();
                            self.redo.clear();
                            self.committed = None;
                            self.pending = None;
                            self.cropping = false;
                        }
                        Err(e) => self.status = crate::trf!("Cannot open: {}", e),
                    }
                }
                Msg::Loaded(..) => {}
                Msg::Preview(gen, r) => {
                    self.previewing = false;
                    if gen != self.gen {
                        continue;
                    }
                    match r {
                        Ok(pv) => {
                            let resized = self.shown.as_ref().map(|s| (s.result.w, s.result.h)) != Some((pv.result.w, pv.result.h));
                            if resized {
                                self.view.fit = true;
                            }
                            self.tex = Some(texture(ctx, "finish", &finish::display_view(&pv.result, self.display, &pv.result)));
                            self.tex_display = self.display;
                            self.before_tex = None;
                            self.shown = Some(*pv);
                        }
                        Err(e) => self.status = crate::trf!("Preview failed: {}", e),
                    }
                }
                Msg::Saved(r) => {
                    self.saving = false;
                    match r {
                        Ok((s, p)) => {
                            self.status = s;
                            self.saved = Some(*p);
                            self.last_saved = self.saving_to.take();
                        }
                        Err(e) => self.status = crate::trf!("Not saved: {}", e),
                    }
                }
            }
        }
        self.mask.poll();
        if let Some(m) = self.mask.message.take() {
            self.status = m;
        }
        if let Some((rx, what)) = &self.dialog {
            if let Ok(r) = rx.try_recv() {
                let what = *what;
                self.dialog = None;
                match r {
                    Picked::NoTool => self.status = dialogs::NO_DIALOG.into(),
                    Picked::Files(f) if what == "open" => {
                        if let Some(f) = f.first() {
                            self.open(&f.clone());
                        }
                    }
                    Picked::Save(Some(p)) => {
                        self.out = p;
                        self.save();
                    }
                    _ => {}
                }
            }
        }
        // The display mode changed: show the same result differently.
        if self.tex_display != self.display {
            if let Some(s) = &self.shown {
                self.tex = Some(texture(ctx, "finish", &finish::display_view(&s.result, self.display, &s.result)));
                self.before_tex = None;
            }
            self.tex_display = self.display;
        }
    }

    /// Start a preview when the settings differ from the shown ones.
    fn kick(&mut self) {
        let Some(px) = self.proxy.clone() else { return };
        let local = self.local();
        let want = self.shown_params();
        if self.previewing || self.shown.as_ref().is_some_and(|s| s.params == want && s.local == local) {
            return;
        }
        self.previewing = true;
        let (p, scale, tx, gen) = (want, self.scale, self.tx.clone(), self.gen);
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(|| {
                let mut m = Measured::default();
                let prepared = finish::prepare(&px, &p);
                let balanced = finish::balance(&prepared, &p, &mut m);
                let background = finish::background_points(&balanced);
                let stretched = finish::stretch(&balanced, &p, &mut m);
                let curve_hist = finish::curve_input_histograms(&stretched, &p);
                let mut result = finish::finish_tone(stretched, &p, scale);
                if let Some(l) = &local {
                    result = finish::with_local(result, px.w, px.h, &p, l, scale);
                }
                let sp = finish::starting_point(&p);
                let before = if sp == p {
                    result.clone()
                } else {
                    let mut m0 = Measured::default();
                    finish::finish_tone(finish::stretch(&prepared, &sp, &mut m0), &sp, scale)
                };
                Box::new(Preview { params: p, prepared, before, result, measured: m, curve_hist, background, local })
            })
            .map_err(|_| panic_text());
            let _ = tx.send(Msg::Preview(gen, r));
        });
    }

    /// The settings the preview shows: while cropping, the whole image (no crop).
    fn shown_params(&self) -> FinishParams {
        if self.cropping {
            FinishParams { crop: [0.0; 4], ..self.p.clone() }
        } else {
            self.p.clone()
        }
    }

    // ----------------------------------------------------------------------
    // Undo / redo
    // ----------------------------------------------------------------------

    fn snap(&self) -> Snap {
        (self.p.clone(), self.mask.local.clone())
    }

    /// Record the state once a change has stayed the same for a moment (a slider
    /// drag or a brush stroke counts once).
    fn check_undo(&mut self) {
        if self.full.is_none() || self.loading {
            return;
        }
        let now = self.snap();
        let Some(c) = &self.committed else {
            self.committed = Some(now);
            return;
        };
        if *c == now {
            self.pending = None;
            return;
        }
        match &self.pending {
            Some((p, t)) if *p == now => {
                if t.elapsed() > std::time::Duration::from_millis(400) {
                    self.commit(now);
                }
            }
            _ => self.pending = Some((now, std::time::Instant::now())),
        }
    }

    fn commit(&mut self, new: Snap) {
        if let Some(old) = self.committed.take() {
            let label = change_label(&old, &new);
            self.undo.push((old, label));
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.committed = Some(new);
        self.pending = None;
    }

    fn step(&mut self, redo: bool) {
        if self.full.is_none() {
            return;
        }
        // A change still settling counts first.
        let now = self.snap();
        if self.committed.as_ref().is_some_and(|c| *c != now) {
            self.commit(now);
        }
        let from = if redo { &mut self.redo } else { &mut self.undo };
        let Some((state, label)) = from.pop() else { return };
        let cur = self.committed.take().unwrap_or_else(|| self.snap());
        if redo { self.undo.push((cur, label.clone())) } else { self.redo.push((cur, label.clone())) }
        self.p = state.0.clone();
        self.mask.set_local(state.1.clone());
        self.committed = Some(state);
        self.pending = None;
        self.status = crate::trf!("{}: {}. ({} undo / {} redo steps left)", if redo { tr("Redone") } else { tr("Undone") }, label, self.undo.len(), self.redo.len());
    }

    // ----------------------------------------------------------------------
    // Presets
    // ----------------------------------------------------------------------

    fn preset_list(&mut self) -> Vec<String> {
        self.presets.get_or_insert_with(list_presets).clone()
    }

    fn save_preset(&mut self) {
        let name = self.preset_name.trim().to_string();
        let geo = FinishParams::default();
        let pr = Preset {
            look: FinishParams { quarter: geo.quarter, flip_h: geo.flip_h, angle: geo.angle, auto_crop: geo.auto_crop, crop: geo.crop, long_edge: geo.long_edge, jpeg_quality: geo.jpeg_quality, ..self.p.clone() },
            sky: self.mask.local.sky.clone(),
            ground: self.mask.local.ground.clone(),
            feather: self.mask.local.feather,
        };
        let r = toml::to_string(&pr).map_err(|e| e.to_string()).and_then(|t| {
            let dir = presets_dir();
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join(format!("{}.toml", file_name(&name))), t).map_err(|e| e.to_string())
        });
        self.status = match r {
            Ok(()) => crate::trf!("Preset “{}” saved: the look without turning, cropping and size.", name),
            Err(e) => crate::trf!("Preset not saved: {}", e),
        };
        self.presets = None;
        self.preset_name.clear();
    }

    fn apply_preset(&mut self, name: &str) {
        let r = std::fs::read_to_string(presets_dir().join(format!("{name}.toml"))).map_err(|e| e.to_string()).and_then(|t| toml::from_str::<Preset>(&t).map_err(|e| e.to_string()));
        match r {
            Ok(pr) => {
                let c = &self.p;
                self.p = FinishParams { quarter: c.quarter, flip_h: c.flip_h, angle: c.angle, auto_crop: c.auto_crop, crop: c.crop, long_edge: c.long_edge, jpeg_quality: c.jpeg_quality, ..pr.look };
                let mut l = self.mask.local.clone();
                (l.sky, l.ground, l.feather) = (pr.sky, pr.ground, pr.feather);
                self.mask.set_local(l);
                self.status = crate::trf!("Preset “{}” applied (Ctrl+Z undoes it).", name);
            }
            Err(e) => self.status = crate::trf!("Cannot read preset “{}”: {}", name, e),
        }
    }

    fn ask_open(&mut self) {
        if self.dialog.is_none() {
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                let r = dialogs::open_files(tr("Open image"), false, Some((tr("Images"), io::image_exts())));
                let _ = tx.send(r.map(Picked::Files).unwrap_or(Picked::NoTool));
            });
            self.dialog = Some((rx, "open"));
        }
    }

    fn ask_save(&mut self) {
        if self.full.is_none() {
            self.status = tr("Open an image first.").into();
        } else if self.dialog.is_none() {
            self.dialog = Some((dialogs::pick_save(tr("Save result"), &self.out), "save"));
        }
    }

    /// Process the full image and save it to `self.out` (in the background).
    fn save(&mut self) {
        let (Some(full), Some(proxy)) = (self.full.clone(), self.proxy.clone()) else {
            self.status = tr("Open an image first.").into();
            return;
        };
        if self.saving {
            return;
        }
        let out = self.out.trim().to_string();
        if let Err(e) = crate::sys::check_writable(&out) {
            self.status = crate::trf!("Not saved: {}", e);
            return;
        }
        self.saving_to = Some(out.clone());
        let local = self.local();
        let need = self.save_memory(&full);
        let dir = std::path::Path::new(&out).parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let (mem, _) = crate::sys::free_cached(&dir);
        let avail = mem - crate::sys::memory_reserve();
        if need > avail {
            self.status = crate::trf!("Not enough free RAM to finish the full image: needs ≈ {}, {} available (free disk space does not help). Close other programs, or crop the image more.", gb(need), gb(avail.max(0.0)));
            return;
        }
        self.saving = true;
        self.status = crate::trf!("Saving {} …", basename(&out));
        let (p, scale, tx) = (self.p.clone(), self.scale, self.tx.clone());
        // Linear only makes sense for linear input (a stretch is chosen).
        let linear = self.save_linear && p.stretch != StretchMode::Off;
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(|| -> Result<(String, Box<Settings>), String> {
                // Measure on the preview copy, as shown, so the saved file looks the same.
                let mut m = Measured::default();
                let _ = finish::process(&proxy, &p, scale, &mut m);
                let mut img = if linear {
                    // The linear steps only: turn and crop, background, colour, white balance.
                    finish::balance(&finish::prepare(&full, &p), &p, &mut m)
                } else {
                    finish::process(&full, &p, 1.0, &mut m)
                };
                if let (Some(l), false) = (&local, linear) {
                    img = finish::with_local(img, full.w, full.h, &p, l, 1.0);
                }
                let img = finish::export_size(&img, &p);
                io::save_image(&out, &img, p.jpeg_quality)?;
                let extra = if linear { tr(", linear: stretch, light, curves, colour, detail and the sky / foreground looks left out") } else if local.is_some() { tr(", sky and foreground looks") } else { "" };
                Ok((crate::trf!("Saved {} ({}×{} px; {}{}).", out, img.w, img.h, finish::summary(&p, tr), extra), Box::new((p, local))))
            })
            .unwrap_or_else(|_| Err(panic_text()));
            let _ = tx.send(Msg::Saved(r));
        });
    }

    /// Working memory for saving the full image.
    /// Memory the save needs on top of the open image. Turning and cropping come
    /// first, so every later step works on the cropped size.
    fn save_memory(&self, full: &Img) -> f64 {
        let (w, h) = finish::cropped_size(full.w, full.h, &self.p);
        (w * h) as f64 * SAVE_BYTES_PER_PX * if self.mask.local.used() { 1.5 } else { 1.0 }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut as Ks, Modifiers as Mo};
        let (save_as, save, open) = ctx.input_mut(|i| {
            (i.consume_shortcut(&Ks::new(Mo::COMMAND | Mo::SHIFT, Key::S)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::S)), i.consume_shortcut(&Ks::new(Mo::COMMAND, Key::O)))
        });
        if save_as {
            self.ask_save();
        } else if save {
            if self.out.trim().is_empty() { self.ask_save() } else { self.save() }
        }
        if open {
            self.ask_open();
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::F) && i.modifiers.is_none()) {
            self.view.fit = true;
        }
        if !ctx.egui_wants_keyboard_input() {
            let (z, y, sh) = ctx.input(|i| (i.modifiers.command && i.key_pressed(Key::Z), i.modifiers.command && i.key_pressed(Key::Y), i.modifiers.shift));
            if z {
                self.step(sh);
            } else if y {
                self.step(true);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.poll(ctx);
        self.shortcuts(ctx);
        self.kick();
        self.check_undo();
        egui::Panel::left("finish_left").default_size(400.0).size_range(340.0..=650.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("fl").show(ui, |ui| self.left(ui));
        });
        egui::Panel::right("finish_right").default_size(330.0).size_range(300.0..=600.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("fr").show(ui, |ui| self.right(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.center(ui, ctx));
    }

    // ----------------------------------------------------------------------
    // Left: the image and the adjustments
    // ----------------------------------------------------------------------

    fn left(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Image"));
        ui.horizontal_wrapped(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.path_edit).desired_width(ui.available_width().min(260.0)).hint_text(tr("file path, Enter to open")));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let p = self.path_edit.clone();
                self.open(&p);
            }
            if ui.button(tr("Open…")).on_hover_text(tr("Open an image: TIFF, PNG, JPEG, FITS, XISF or camera RAW (Ctrl+O)")).clicked() {
                self.ask_open();
            }
        });
        ui.checkbox(&mut self.keep, tr("Keep settings for the next image")).on_hover_text(tr("Give several images (e.g. panorama panels) the same look: open the next one without resetting the adjustments"));
        if self.full.is_some() {
            let list = self.preset_list();
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Preset"));
                let mut chosen = None;
                egui::ComboBox::from_id_salt("fin_preset").selected_text(if list.is_empty() { tr("none saved yet") } else { tr("apply…") }).show_ui(ui, |ui| {
                    for n in &list {
                        if ui.selectable_label(false, n).clicked() {
                            chosen = Some(n.clone());
                        }
                    }
                }).response.on_hover_text(tr("Apply a saved look: stretch, white balance, light, curves, colour, detail and the sky / foreground looks (not the turn, crop or size)"));
                if let Some(n) = chosen {
                    self.apply_preset(&n);
                }
                let r = ui.add(egui::TextEdit::singleline(&mut self.preset_name).desired_width(110.0).hint_text(tr("name")));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let ok = !self.preset_name.trim().is_empty();
                if ui.add_enabled(ok, egui::Button::new(tr("Save"))).on_hover_text(tr("Save the current look under this name")).clicked() || (enter && ok) {
                    self.save_preset();
                }
                if list.iter().any(|n| *n == file_name(self.preset_name.trim())) && ui.small_button(tr("Delete")).on_hover_text(tr("Delete the preset with this name")).clicked() {
                    let _ = std::fs::remove_file(presets_dir().join(format!("{}.toml", file_name(self.preset_name.trim()))));
                    self.status = crate::trf!("Preset “{}” deleted.", self.preset_name.trim());
                    self.presets = None;
                    self.preset_name.clear();
                }
            });
        }
        if self.full.is_some() {
            ui.horizontal_wrapped(|ui| {
                let mut lin = self.p.stretch != StretchMode::Off;
                if ui.checkbox(&mut lin, tr("Linear data (needs stretching)")).on_hover_text(tr("On for stacks saved as FITS / XISF (dark, unstretched). Off for images that already look right")).changed() {
                    self.p.stretch = if lin { StretchMode::MtfAuto } else { StretchMode::Off };
                }
            });
        }
        ui.separator();
        if self.full.is_none() {
            ui.label(egui::RichText::new(tr("The adjustments appear once an image is open.")).weak());
            return;
        }
        let d = FinishParams::default();

        section(ui, tr("Turn and crop"), true, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("⟲ 90°").clicked() {
                    self.p.quarter = (self.p.quarter + 3) % 4;
                }
                if ui.button("⟳ 90°").clicked() {
                    self.p.quarter = (self.p.quarter + 1) % 4;
                }
                ui.checkbox(&mut self.p.flip_h, tr("Mirror"));
            });
            egui::Grid::new("fin_geo").num_columns(2).show(ui, |ui| {
                super::stitch::angle(ui, tr("Rotate (°)"), &mut self.p.angle, 45.0, true);
                ui.label("");
                ui.checkbox(&mut self.p.auto_crop, tr("Cut away the empty corners"));
                ui.end_row();
                for (k, l) in [tr("Crop left (%)"), tr("Crop top (%)"), tr("Crop right (%)"), tr("Crop bottom (%)")].iter().enumerate() {
                    let mut v = self.p.crop[k] * 100.0;
                    ui.label(*l);
                    if ui.add(egui::Slider::new(&mut v, 0.0..=45.0).step_by(0.5)).changed() {
                        self.p.crop[k] = v / 100.0;
                    }
                    ui.end_row();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Shape:"));
                for (l, r) in [("3:2", 1.5), ("4:3", 4.0 / 3.0), ("16:9", 16.0 / 9.0), ("4:5", 0.8), ("1:1", 1.0), ("2:1", 2.0)] {
                    if ui.small_button(l).on_hover_text(tr("Crop to this shape, centred (turned to the image's orientation)")).clicked() {
                        self.crop_to(r);
                    }
                }
                if ui.small_button(tr("Free")).on_hover_text(tr("No crop")).clicked() {
                    self.p.crop = [0.0; 4];
                }
            });
            if ui.small_button(tr("Reset turn and crop")).clicked() {
                self.p.quarter = 0;
                self.p.flip_h = false;
                self.p.angle = 0.0;
                self.p.crop = d.crop;
                self.p.auto_crop = true;
            }
        });

        if self.p.stretch != StretchMode::Off || self.linear {
            section(ui, tr("Stretch"), true, |ui| self.stretch_ui(ui));
        }

        section(ui, tr("White balance"), true, |ui| {
            ui.horizontal_wrapped(|ui| {
                let t = if self.sampling { tr("◎ Drag over a grey area…") } else { tr("◎ Sample grey") };
                if ui.selectable_label(self.sampling, t).on_hover_text(tr("Then drag a rectangle over something that should be neutral grey or white (sky background, a cloud, a wall). The colour multipliers are set from it")).clicked() {
                    self.sampling = !self.sampling;
                }
                if self.p.wb != [1.0; 3] {
                    ui.label(egui::RichText::new(format!("× {:.2} {:.2} {:.2}", self.p.wb[0], self.p.wb[1], self.p.wb[2])).weak());
                    if ui.small_button(tr("Clear")).clicked() {
                        self.p.wb = [1.0; 3];
                    }
                }
            });
            ui.checkbox(&mut self.p.neutral_bg, tr("Neutral background")).on_hover_text(tr("Make the darker half of the image (usually the sky background) neutral grey"));
            temperature_controls(ui, &mut self.p, "fin_wb");
        });

        section(ui, tr("Light"), true, |ui| {
            light_controls(ui, &mut self.p, "fin_tone");
        });

        section(ui, tr("Curves"), !self.simple, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (k, l) in [tr("RGB"), tr("Red"), tr("Green"), tr("Blue")].iter().enumerate() {
                    let edited = self.p.curves[k] != identity_curve();
                    if ui.selectable_label(self.curve_ch == k, if edited { format!("{l} •") } else { l.to_string() }).clicked() {
                        self.curve_ch = k;
                    }
                }
            });
            let k = self.curve_ch;
            let hist = self.shown.as_ref().map(|s| s.curve_hist[k].clone());
            curve_editor(ui, &mut self.p.curves[k], hist.as_deref(), CURVE_COLOURS[k]);
            ui.label(egui::RichText::new(tr("Click to add a point, drag to move it, right-click to remove it.")).weak().size(12.0));
            ui.horizontal_wrapped(|ui| {
                if ui.small_button(tr("Reset this curve")).clicked() {
                    self.p.curves[k] = identity_curve();
                }
                if ui.small_button(tr("Reset all curves")).clicked() {
                    self.p.curves = d.curves.clone();
                }
            });
        });

        section(ui, tr("Colour"), true, |ui| {
            colour_controls(ui, &mut self.p, "fin_col");
        });

        section(ui, tr("Detail"), !self.simple, |ui| {
            detail_controls(ui, &mut self.p, "fin_det", self.simple);
        });

        let edited = if self.mask.local.used() { " •" } else { "" };
        section(ui, &crate::trf!("Sky and foreground{}", edited), false, |ui| self.local_ui(ui));

        ui.add_space(6.0);
        if ui.button(tr("↺ Reset all adjustments")).clicked() {
            let keep = (self.p.long_edge, self.p.jpeg_quality);
            self.p = FinishParams::for_input(self.linear);
            (self.p.long_edge, self.p.jpeg_quality) = keep;
        }
    }

    /// The mask and the sky / foreground looks.
    fn local_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(tr("Adjust the sky and the foreground separately, on top of everything above: e.g. less noise and smaller stars in the sky, a brighter, warmer foreground. Paint or detect the mask, then set the looks.")).weak().size(12.0));
        if self.mask.painting {
            self.sampling = false;
        }
        self.mask.controls(ui, self.proxy.clone());
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Adjust:"));
            for (k, t) in [tr("Sky"), tr("Foreground")].iter().enumerate() {
                let q = if k == 0 { &self.mask.local.sky } else { &self.mask.local.ground };
                let edited = finish::look_used(q);
                if ui.selectable_label(self.local_k == k, if edited { format!("{t} •") } else { t.to_string() }).clicked() {
                    self.local_k = k;
                }
            }
        });
        if !self.mask.has_sky() {
            ui.colored_label(Color32::from_rgb(230, 150, 30), tr("Nothing is sky yet: paint the mask or detect the sky."));
        }
        let k = self.local_k;
        let id = ["fin_sky", "fin_fg"][k];
        let q = if k == 0 { &mut self.mask.local.sky } else { &mut self.mask.local.ground };
        light_controls(ui, q, &format!("{id}_light"));
        temperature_controls(ui, q, &format!("{id}_temp"));
        colour_controls(ui, q, &format!("{id}_col"));
        detail_controls(ui, q, &format!("{id}_det"), self.simple);
        if ui.small_button(if k == 0 { tr("Reset the sky look") } else { tr("Reset the foreground look") }).clicked() {
            *q = FinishParams::default();
        }
    }

    fn stretch_ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("fin_str").num_columns(2).show(ui, |ui| {
            ui.label(tr("Method"));
            egui::ComboBox::from_id_salt("fin_stretch").selected_text(tr(self.p.stretch.name())).show_ui(ui, |ui| {
                for m in StretchMode::ALL {
                    ui.selectable_value(&mut self.p.stretch, m, tr(m.name()));
                }
            });
            ui.end_row();
            match self.p.stretch {
                StretchMode::MtfAuto => {
                    slider(ui, tr("Background"), &mut self.p.target, 0.05..=0.5, tr("Brightness of the sky background after stretching"));
                    ui.label("");
                    ui.checkbox(&mut self.p.unlinked, tr("Each colour on its own")).on_hover_text(tr("Measures red, green and blue separately: removes a colour cast, but can shift star colours"));
                    ui.end_row();
                }
                StretchMode::MtfManual => {
                    log_slider(ui, tr("Black point"), &mut self.p.black, 0.0..=0.5, tr("Everything darker becomes black"));
                    slider(ui, tr("Midtones"), &mut self.p.mid, 0.0001..=0.9999, tr("Lower = brighter (the value that becomes 50 % grey)"));
                    slider(ui, tr("White point"), &mut self.p.white, 0.1..=1.0, tr("Everything brighter becomes white"));
                }
                StretchMode::Asinh => {
                    ui.label(tr("Stretch factor"));
                    ui.add(egui::Slider::new(&mut self.p.asinh, 1.0..=10000.0).logarithmic(true)).on_hover_text(tr("1 = almost none, 10000 = very strong. Long deep-sky stacks with a faint background need several thousand"));
                    ui.end_row();
                    log_slider(ui, tr("Black point"), &mut self.p.black, 0.0..=0.5, tr("Subtracted before stretching"));
                }
                StretchMode::Ghs => {
                    slider(ui, tr("Stretch ln(D+1)"), &mut self.p.ghs_d, 0.0..=10.0, tr("How strong the stretch is"));
                    slider(ui, tr("Local intensity b"), &mut self.p.ghs_b, -5.0..=15.0, tr("Concentrates the stretch around the symmetry point: higher = narrower, more contrast there"));
                    log_slider(ui, tr("Symmetry point"), &mut self.p.ghs_sp, 0.0..=1.0, tr("Brightness where the stretch is strongest (put it at the sky background: use the histogram)"));
                    if !self.simple {
                        log_slider(ui, tr("Protect shadows"), &mut self.p.ghs_lp, 0.0..=1.0, tr("Below this the curve is a straight line: keeps dark areas from being stretched"));
                        slider(ui, tr("Protect highlights"), &mut self.p.ghs_hp, 0.0..=1.0, tr("Above this the curve is a straight line: keeps stars from swelling"));
                    }
                    log_slider(ui, tr("Black point"), &mut self.p.black, 0.0..=0.5, tr("Subtracted before stretching"));
                }
                _ => {}
            }
        });
        if self.p.stretch.invertible() {
            ui.checkbox(&mut self.p.inverse, tr("Inverse")).on_hover_text(tr("The inverse curve: compresses the bright end instead of lifting the dark end (undoes too strong a stretch)"));
        }
        if matches!(self.p.stretch, StretchMode::Asinh | StretchMode::Ghs) {
            ui.checkbox(&mut self.p.keep_colour, tr("Keep star colours")).on_hover_text(tr("Stretch the brightness and keep each pixel's colour. Off: red, green and blue are stretched separately (more saturated sky, paler stars)"));
        }
        if self.p.stretch == StretchMode::MtfAuto {
            if let Some(bm) = self.shown.as_ref().and_then(|s| s.measured.stretch) {
                if ui.small_button(tr("Auto → manual")).on_hover_text(tr("Continue by hand from the automatic values (the average of the three colours)")).clicked() {
                    self.p.stretch = StretchMode::MtfManual;
                    self.p.black = (bm[0].0 + bm[1].0 + bm[2].0) / 3.0;
                    self.p.mid = (bm[0].1 + bm[1].1 + bm[2].1) / 3.0;
                    self.p.white = 1.0;
                }
            }
        }
        if matches!(self.p.stretch, StretchMode::Asinh | StretchMode::Ghs | StretchMode::MtfManual) {
            if let Some((black, sp)) = self.shown.as_ref().map(|s| s.background) {
                let tip = if self.p.stretch == StretchMode::Ghs { tr("Black point just below the sky background, symmetry point on it") } else { tr("Black point just below the sky background") };
                if ui.small_button(tr("Points from the background")).on_hover_text(tip).clicked() {
                    self.p.black = black;
                    if self.p.stretch == StretchMode::Ghs {
                        self.p.ghs_sp = sp;
                    }
                }
            }
        }
        if self.p.stretch != StretchMode::Off {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.p.bg_extraction, tr("Background extraction")).on_hover_text(tr("Before stretching: removes smooth gradients (light pollution, moon)"));
                ui.checkbox(&mut self.p.color_calibration, tr("Colour calibration")).on_hover_text(tr("Before stretching: neutral sky background, average star white"));
            });
        }
    }

    /// Crop to the shape `r` (width / height, turned to the image's orientation), centred.
    fn crop_to(&mut self, r: f64) {
        let Some(full) = &self.full else { return };
        let mut p = self.p.clone();
        p.crop = [0.0; 4];
        let (w, h) = finish::cropped_size(full.w, full.h, &p);
        let (w, h) = (w as f64, h as f64);
        let r = if (h > w) != (r < 1.0) && r != 1.0 { 1.0 / r } else { r };
        self.p.crop = if w / h > r {
            let m = (1.0 - h * r / w) / 2.0;
            [m, 0.0, m, 0.0]
        } else {
            let m = (1.0 - w / r / h) / 2.0;
            [0.0, m, 0.0, m]
        };
    }

    // ----------------------------------------------------------------------
    // Right: histogram and saving
    // ----------------------------------------------------------------------

    fn right(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Histogram"));
        match &self.shown {
            Some(s) => super::stitch::histogram(ui, &s.result, None),
            None => {
                ui.label(egui::RichText::new("—").weak());
            }
        }
        ui.separator();
        ui.heading(tr("Save"));
        let Some(full) = self.full.clone() else {
            ui.label(egui::RichText::new(tr("Open an image first.")).weak());
            return;
        };
        egui::Grid::new("fin_out").num_columns(2).show(ui, |ui| {
            ui.label(tr("Size"));
            let (cw, ch) = finish::cropped_size(full.w, full.h, &self.p);
            let long = cw.max(ch);
            let txt = if self.p.long_edge == 0 || self.p.long_edge >= long { tr("Full size").to_string() } else { crate::trf!("{} px long side", self.p.long_edge) };
            egui::ComboBox::from_id_salt("fin_size").selected_text(txt).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.p.long_edge, 0, tr("Full size"));
                for (v, l) in [(3840, tr("3840 px (4K screen)")), (2560, tr("2560 px")), (2048, tr("2048 px (web, forums)")), (1920, tr("1920 px (Full HD)")), (1600, tr("1600 px")), (1080, tr("1080 px (Instagram)"))] {
                    if v < long {
                        ui.selectable_value(&mut self.p.long_edge, v, l);
                    }
                }
            });
            ui.end_row();
            ui.label("");
            // 0 means full size: say so instead of showing "0 px".
            ui.add(egui::DragValue::new(&mut self.p.long_edge).range(0..=60000).speed(10)
                .custom_formatter(|v, _| if v < 0.5 { tr("full size").to_string() } else { format!("{v:.0} px") })
                .custom_parser(|s| s.trim().trim_end_matches("px").trim().parse::<f64>().ok().or(Some(0.0))))
                .on_hover_text(tr("Longest side of the saved image in pixels (drag, or double-click and type); 0 = full size"));
            ui.end_row();
            if matches!(io::ext_of(&self.out).as_str(), "jpg" | "jpeg") {
                ui.label(tr("JPEG quality"));
                ui.add(egui::Slider::new(&mut self.p.jpeg_quality, 50..=100));
                ui.end_row();
            }
        });
        let (ow, oh) = finish::output_size(full.w, full.h, &self.p);
        ui.label(format!("→ {ow} × {oh} px ({:.1} MP)", (ow * oh) as f64 / 1e6));
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.out).desired_width(ui.available_width().min(240.0)).hint_text(tr("output file"))).on_hover_text(self.out.as_str());
            let cur = io::ext_of(&self.out);
            egui::ComboBox::from_id_salt("fin_fmt").selected_text(if cur.is_empty() { tr("type").to_string() } else { cur.clone() }).show_ui(ui, |ui| {
                for (e, l) in io::OUTPUT_FORMATS {
                    if ui.selectable_label(cur == e || (cur == "tiff" && e == "tif") || (cur == "jpeg" && e == "jpg"), l).clicked() {
                        self.out = io::with_ext(&self.out, e);
                    }
                }
            });
        });
        if io::is_astro(&self.out) {
            ui.label(egui::RichText::new(tr("FITS / XISF get the finished (stretched) image here, as 32-bit float.")).weak().size(12.0));
        }
        if std::path::Path::new(&self.out).exists() {
            ui.colored_label(Color32::from_rgb(230, 150, 30), tr("This file exists and will be replaced."));
        }
        if self.p.stretch != StretchMode::Off {
            if let Some(v) = super::save_kind_ui(ui, self.save_linear, &crate::io::ext_of(&self.out)) {
                self.save_linear = v;
            }
            if self.save_linear {
                ui.weak(tr("Saved linear: only turn and crop, background, colour calibration and white balance are applied."));
            }
        }
        ui.horizontal_wrapped(|ui| {
            let save = ui.add_enabled(!self.saving, egui::Button::new(egui::RichText::new(tr("⬇ Save")).strong())).on_hover_text(tr("Process the full image and save it (Ctrl+S)"));
            if save.clicked() {
                self.save();
            }
            if ui.add_enabled(!self.saving, egui::Button::new(tr("Save as…"))).on_hover_text(tr("Choose the file (Ctrl+Shift+S)")).clicked() {
                self.ask_save();
            }
            if let Some(f) = self.last_saved.as_ref().filter(|f| std::path::Path::new(f).exists()) {
                if ui.button(tr("Show in folder")).on_hover_text(crate::trf!("Last saved: {}", f)).clicked() {
                    dialogs::show_in_folder(f);
                }
            }
        });
        let need = self.save_memory(&full);
        let dir = std::path::Path::new(&self.out).parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let (mem, _) = crate::sys::free_cached(&dir);
        let avail = mem - crate::sys::memory_reserve();
        let txt = crate::trf!("Saving needs ≈ {} of RAM ({} available).", gb(need), gb(avail.max(0.0)));
        if need > avail {
            ui.colored_label(Color32::from_rgb(230, 150, 30), crate::trf!("⚠ {} Close other programs or crop first.", txt));
        } else {
            ui.label(egui::RichText::new(txt).weak().size(12.0));
        }
        ui.separator();
        let extra = if self.mask.local.used() { tr(", sky and foreground looks") } else { "" };
        ui.label(egui::RichText::new(crate::trf!("In use: {}{}", finish::summary(&self.p, tr), extra)).weak().size(12.0));
    }

    // ----------------------------------------------------------------------
    // Centre: the preview
    // ----------------------------------------------------------------------

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut compare = false;
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Fit")).on_hover_text(tr("Whole image in view (F)")).clicked() {
                self.view.fit = true;
            }
            if ui.button("1:1").on_hover_text(tr("One preview pixel per screen pixel")).clicked() {
                self.view.zoom = 1.0;
            }
            let tip = |v: &Vec<(Snap, String)>, key: &str| format!("{} ({key})", v.last().map(|u| u.1.clone()).unwrap_or_else(|| tr("nothing").into()));
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new(tr("↶ Undo"))).on_hover_text(tip(&self.undo, "Ctrl+Z")).on_disabled_hover_text(tr("Nothing to undo")).clicked() {
                self.step(false);
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new(tr("↷ Redo"))).on_hover_text(tip(&self.redo, "Ctrl+Y")).on_disabled_hover_text(tr("Nothing to redo")).clicked() {
                self.step(true);
            }
            let crop_txt = if self.cropping { tr("✂ Crop: done") } else { tr("✂ Crop") };
            if ui.selectable_label(self.cropping, crop_txt).on_hover_text(tr("Crop on the image: drag a frame, drag its edges or corners to adjust it, drag inside it to move it. Click again when done")).clicked() {
                self.cropping = !self.cropping;
                self.crop_drag = None;
                if self.cropping {
                    self.mask.painting = false;
                    self.sampling = false;
                }
            }
            let c = ui.button(tr("◐ Compare")).on_hover_text(tr("Hold to see the image before white balance, light, curves, colour and detail (or hold B)"));
            compare = c.is_pointer_button_down_on() || (!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(egui::Key::B)));
            ui.separator();
            super::view_stretch_ui(ui, "fin_disp", &mut self.display, None);
            if self.mask.painting {
                self.cropping = false;
                ui.colored_label(Color32::from_rgb(120, 170, 255), tr("Painting the mask: left = sky, right = foreground, wheel = brush size"));
            }
            if self.cropping {
                ui.colored_label(Color32::from_rgb(255, 210, 0), tr("Cropping: the whole image is shown"));
            }
            if self.busy() {
                ui.spinner();
            }
        });
        ui.label(egui::RichText::new(&self.status).size(12.5));
        if self.scale < 1.0 && self.shown.is_some() {
            ui.label(egui::RichText::new(crate::trf!("Preview at {} % of full size; the full image is processed when saving.", format!("{:.0}", self.scale * 100.0))).weak().size(12.0));
        }
        let Some(s) = &self.shown else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(if self.loading { tr("Loading…") } else { tr("No image yet") }).size(20.0).weak());
                ui.label(egui::RichText::new(tr("Open an image (Ctrl+O), or use “Use in Post-processing” in the Star stacking tab or “Post-process” after saving a panorama.")).weak());
            });
            return;
        };
        let (w, h) = (s.result.w as f64, s.result.h as f64);
        if compare && self.before_tex.is_none() {
            self.before_tex = Some(texture(ctx, "finish_before", &finish::display_view(&s.before, self.display, &s.before)));
        }
        let tex = if compare { self.before_tex.as_ref() } else { self.tex.as_ref() };
        let Some(tex) = tex.cloned() else { return };
        // The mask on the shown (turned and cropped) image; it lies on the source image.
        let (src, geo, out) = (self.proxy.as_ref().map_or((1, 1), |p| (p.w, p.h)), s.params.clone(), (s.result.w, s.result.h));
        self.mask.wheel(ui);
        let ov = if compare { None } else { self.mask.overlay(ctx, src, &geo, out) };
        let b = [0.5, w + 0.5, 0.5, h + 0.5];
        let mut layers = vec![(&tex, b)];
        if let Some(t) = &ov {
            layers.push((t, b));
        }
        let shown = image_view(ui, &mut self.view, (w, h), &layers, !self.sampling && !self.mask.painting && !self.cropping);
        self.mask.input(ui, &shown, src, &geo, out);
        if self.cropping && s.params.crop == [0.0; 4] {
            self.crop_input(ui, &shown, w, h);
        }
        let Some(s) = &self.shown else { return };
        if compare {
            ui.painter().text(shown.rect.left_top() + egui::vec2(8.0, 8.0), egui::Align2::LEFT_TOP, "Before", egui::FontId::proportional(16.0), Color32::WHITE);
        }
        if self.sampling {
            let pos = shown.resp.interact_pointer_pos();
            if shown.resp.drag_started() {
                self.sample_from = pos.map(|p| shown.to_image(p));
            }
            if let (Some(a), Some(p)) = (self.sample_from, pos.or(shown.resp.hover_pos())) {
                let b = shown.to_image(p);
                let r = Rect::from_two_pos(shown.to_screen(a.0, a.1), shown.to_screen(b.0, b.1));
                ui.painter().rect_stroke(r, 0.0, Stroke::new(1.5, Color32::from_rgb(255, 210, 0)), egui::StrokeKind::Middle);
                if shown.resp.drag_stopped() {
                    let wb = finish::sample_wb(&s.prepared, a, b, self.p.stretch == StretchMode::Off);
                    self.sample_from = None;
                    self.sampling = false;
                    match wb {
                        Some(wb) => {
                            self.p.wb = wb;
                            (self.p.temperature, self.p.tint) = (0.0, 0.0);
                            self.status = tr("White balance set from the sampled area.").into();
                        }
                        None => self.status = tr("The sampled area is black or outside the image: try another one.").into(),
                    }
                }
            }
        }
    }
}

impl FinishTab {
    /// Crop on the shown (uncropped) image of `w × h`: draw, resize or move the frame.
    fn crop_input(&mut self, ui: &egui::Ui, s: &super::Shown, w: f64, h: f64) {
        let c = self.p.crop;
        let mut f = [c[0], c[1], 1.0 - c[2], 1.0 - c[3]]; // x0, y0, x1, y1 as fractions
        let to_frac = |p: Pos2| {
            let (x, y) = s.to_image(p);
            (((x - 0.5) / w).clamp(0.0, 1.0), ((y - 0.5) / h).clamp(0.0, 1.0))
        };
        let screen = |f: &[f64; 4]| Rect::from_min_max(s.to_screen(f[0] * w + 0.5, f[1] * h + 0.5), s.to_screen(f[2] * w + 0.5, f[3] * h + 0.5));
        let r = screen(&f);
        let grip_at = |p: Pos2| -> CropGrip {
            let near = 8.0;
            let inside_y = p.y > r.top() - near && p.y < r.bottom() + near;
            let inside_x = p.x > r.left() - near && p.x < r.right() + near;
            let e = [(p.x - r.left()).abs() < near && inside_y, (p.y - r.top()).abs() < near && inside_x, (p.x - r.right()).abs() < near && inside_y, (p.y - r.bottom()).abs() < near && inside_x];
            if e.iter().any(|v| *v) {
                CropGrip::Edges(e)
            } else if r.contains(p) && c != [0.0; 4] {
                CropGrip::Move
            } else {
                CropGrip::New
            }
        };
        if let Some(p) = s.resp.hover_pos() {
            let icon = match self.crop_drag.map(|d| d.0).unwrap_or_else(|| grip_at(p)) {
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
                self.crop_drag = Some((grip_at(p), f, to_frac(p)));
            }
        }
        if let (Some((grip, f0, a)), Some(p)) = (self.crop_drag, s.resp.interact_pointer_pos()) {
            let b = to_frac(p);
            let min = 0.02;
            match grip {
                CropGrip::New => f = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)],
                CropGrip::Move => {
                    let dx = (b.0 - a.0).clamp(-f0[0], 1.0 - f0[2]);
                    let dy = (b.1 - a.1).clamp(-f0[1], 1.0 - f0[3]);
                    f = [f0[0] + dx, f0[1] + dy, f0[2] + dx, f0[3] + dy];
                }
                CropGrip::Edges(e) => {
                    f = f0;
                    if e[0] {
                        f[0] = b.0.min(f0[2] - min);
                    }
                    if e[1] {
                        f[1] = b.1.min(f0[3] - min);
                    }
                    if e[2] {
                        f[2] = b.0.max(f0[0] + min);
                    }
                    if e[3] {
                        f[3] = b.1.max(f0[1] + min);
                    }
                }
            }
            if f[2] - f[0] >= min && f[3] - f[1] >= min {
                self.p.crop = [f[0], f[1], 1.0 - f[2], 1.0 - f[3]].map(|v| (v.clamp(0.0, 1.0) * 1e4).round() / 1e4);
            }
        }
        if s.resp.drag_stopped() {
            self.crop_drag = None;
        }
        // The frame: everything outside it dimmed.
        let c = self.p.crop;
        let r = screen(&[c[0], c[1], 1.0 - c[2], 1.0 - c[3]]);
        let all = Rect::from_min_max(s.to_screen(0.5, 0.5), s.to_screen(w + 0.5, h + 0.5));
        let p = ui.painter().with_clip_rect(s.rect);
        let dim = Color32::from_black_alpha(140);
        for part in [
            Rect::from_min_max(all.min, Pos2::new(all.max.x, r.top())),
            Rect::from_min_max(Pos2::new(all.min.x, r.bottom()), all.max),
            Rect::from_min_max(Pos2::new(all.min.x, r.top()), Pos2::new(r.left(), r.bottom())),
            Rect::from_min_max(Pos2::new(r.right(), r.top()), Pos2::new(all.max.x, r.bottom())),
        ] {
            if part.is_positive() {
                p.rect_filled(part, 0.0, dim);
            }
        }
        p.rect_stroke(r, 0.0, Stroke::new(1.5, Color32::from_rgb(255, 210, 0)), egui::StrokeKind::Middle);
        for k in 1..3 {
            let t = k as f32 / 3.0;
            let thin = Stroke::new(0.7, Color32::from_rgba_unmultiplied(255, 210, 0, 90));
            p.line_segment([Pos2::new(r.left() + r.width() * t, r.top()), Pos2::new(r.left() + r.width() * t, r.bottom())], thin);
            p.line_segment([Pos2::new(r.left(), r.top() + r.height() * t), Pos2::new(r.right(), r.top() + r.height() * t)], thin);
        }
    }
}

/// What changed between two states, for the undo labels.
fn change_label(a: &Snap, b: &Snap) -> String {
    let (p, q) = (&a.0, &b.0);
    let mut v: Vec<&str> = Vec::new();
    if (p.quarter, p.flip_h, p.angle, p.auto_crop) != (q.quarter, q.flip_h, q.angle, q.auto_crop) {
        v.push("turn");
    }
    if p.crop != q.crop {
        v.push("crop");
    }
    if (p.stretch, p.inverse, p.keep_colour, p.target, p.unlinked, p.black, p.mid, p.white, p.asinh, p.bg_extraction, p.color_calibration) != (q.stretch, q.inverse, q.keep_colour, q.target, q.unlinked, q.black, q.mid, q.white, q.asinh, q.bg_extraction, q.color_calibration)
        || (p.ghs_d, p.ghs_b, p.ghs_sp, p.ghs_lp, p.ghs_hp) != (q.ghs_d, q.ghs_b, q.ghs_sp, q.ghs_lp, q.ghs_hp)
    {
        v.push("stretch");
    }
    if (p.wb, p.neutral_bg, p.temperature, p.tint) != (q.wb, q.neutral_bg, q.temperature, q.tint) {
        v.push(tr("white balance"));
    }
    if (p.exposure, p.contrast, p.highlights, p.midtones, p.shadows, p.whites, p.blacks) != (q.exposure, q.contrast, q.highlights, q.midtones, q.shadows, q.whites, q.blacks) {
        v.push("light");
    }
    if p.curves != q.curves {
        v.push("curves");
    }
    if (p.saturation, p.vibrance) != (q.saturation, q.vibrance) {
        v.push("colour");
    }
    if (p.noise, p.color_noise, p.star_reduce, p.sharpen, p.sharpen_radius) != (q.noise, q.color_noise, q.star_reduce, q.sharpen, q.sharpen_radius) {
        v.push("detail");
    }
    if (p.long_edge, p.jpeg_quality) != (q.long_edge, q.jpeg_quality) {
        v.push("size");
    }
    if a.1.mask != b.1.mask || a.1.feather != b.1.feather {
        v.push("mask");
    }
    if a.1.sky != b.1.sky {
        v.push(tr("sky look"));
    }
    if a.1.ground != b.1.ground {
        v.push(tr("foreground look"));
    }
    if v.is_empty() { "a change".into() } else { v.join(", ") }
}

fn presets_dir() -> std::path::PathBuf {
    super::config_dir().join("presets")
}

/// Saved presets, by name.
fn list_presets() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(presets_dir())
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string())).collect())
        .unwrap_or_default();
    v.sort_by_key(|s| s.to_lowercase());
    v
}

/// A preset name made safe as a file name.
fn file_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect::<String>().trim_matches('.').to_string()
}

/// The light sliders (exposure, contrast, highlights, midtones, shadows, whites,
/// blacks) and their reset; shared by the Post-processing and Timelapse tabs.
pub(crate) fn light_controls(ui: &mut egui::Ui, p: &mut FinishParams, id: &str) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        ui.label(tr("Exposure (EV)"));
        ui.add(egui::Slider::new(&mut p.exposure, -3.0..=3.0).step_by(0.05));
        ui.end_row();
        slider(ui, tr("Contrast"), &mut p.contrast, -1.0..=1.0, tr("S-curve around the middle grey"));
        slider(ui, tr("Highlights"), &mut p.highlights, -1.0..=1.0, tr("Bright areas (around 75 %): lower to bring back detail in bright sky or lights"));
        slider(ui, tr("Midtones"), &mut p.midtones, -1.0..=1.0, tr("Middle brightness (around 50 %)"));
        slider(ui, tr("Shadows"), &mut p.shadows, -1.0..=1.0, tr("Dark areas (around 25 %): raise to see into the foreground"));
        slider(ui, tr("Whites"), &mut p.whites, -1.0..=1.0, tr("White point: raise to make the brightest parts white, lower to keep them from clipping"));
        slider(ui, tr("Blacks"), &mut p.blacks, -1.0..=1.0, tr("Black point: lower for deeper blacks, raise to lift them"));
    });
    if ui.small_button(tr("Reset light")).clicked() {
        (p.exposure, p.contrast, p.highlights, p.midtones, p.shadows, p.whites, p.blacks) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
}

/// The detail sliders (noise, colour noise, stars, sharpening).
pub(crate) fn detail_controls(ui: &mut egui::Ui, p: &mut FinishParams, id: &str, simple: bool) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        slider(ui, tr("Noise reduction"), &mut p.noise, 0.0..=1.0, tr("Smooths grain in flat areas (sky), keeps edges and stars"));
        slider(ui, tr("Colour noise"), &mut p.color_noise, 0.0..=1.0, tr("Removes coloured speckles from high-ISO images, keeps the brightness detail"));
        slider(ui, tr("Star reduction"), &mut p.star_reduce, 0.0..=1.0, tr("Makes stars smaller so the Milky Way or nebula stands out"));
        slider(ui, tr("Sharpen"), &mut p.sharpen, 0.0..=2.0, tr("Unsharp mask; check at 1:1 zoom"));
        if !simple {
            ui.label(tr("Sharpen radius (px)"));
            ui.add(egui::Slider::new(&mut p.sharpen_radius, 0.5..=5.0).step_by(0.1));
            ui.end_row();
        }
    });
}

/// Temperature and tint sliders.
pub(crate) fn temperature_controls(ui: &mut egui::Ui, p: &mut FinishParams, id: &str) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        slider(ui, tr("Temperature"), &mut p.temperature, -1.0..=1.0, tr("Cooler (blue) ← → warmer (yellow)"));
        slider(ui, tr("Tint"), &mut p.tint, -1.0..=1.0, tr("Greener ← → more magenta"));
    });
}

/// Saturation and vibrance sliders.
pub(crate) fn colour_controls(ui: &mut egui::Ui, p: &mut FinishParams, id: &str) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        slider(ui, tr("Saturation"), &mut p.saturation, 0.0..=2.0, tr("All colours: 0 = black and white, 1 = unchanged"));
        slider(ui, tr("Vibrance"), &mut p.vibrance, -1.0..=1.0, tr("Dull colours more than strong ones (skin, already strong colours stay natural)"));
    });
}

/// A collapsible group of controls.
pub(crate) fn section(ui: &mut egui::Ui, title: &str, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong()).default_open(open).show(ui, add);
}

/// A labelled slider (row of a grid), double-click the value to type it.
fn slider(ui: &mut egui::Ui, label: &str, v: &mut f64, r: std::ops::RangeInclusive<f64>, tip: &str) -> bool {
    ui.label(label).on_hover_text(tip);
    let c = ui.add(egui::Slider::new(v, r).step_by(0.01).max_decimals(3)).on_hover_text(tip).changed();
    ui.end_row();
    c
}

/// As `slider`, fine near the low end (black points and similar small values).
fn log_slider(ui: &mut egui::Ui, label: &str, v: &mut f64, r: std::ops::RangeInclusive<f64>, tip: &str) -> bool {
    ui.label(label).on_hover_text(tip);
    let c = ui.add(egui::Slider::new(v, r).logarithmic(true).smallest_positive(1e-4).max_decimals(4)).on_hover_text(tip).changed();
    ui.end_row();
    c
}

const CURVE_COLOURS: [Color32; 4] = [Color32::from_gray(225), Color32::from_rgb(235, 80, 80), Color32::from_rgb(90, 200, 90), Color32::from_rgb(90, 140, 245)];

/// The curve editor: click adds a point, drag moves it, right-click removes it.
fn curve_editor(ui: &mut egui::Ui, pts: &mut Vec<[f64; 2]>, hist: Option<&[f32]>, colour: Color32) -> bool {
    let side = ui.available_width().clamp(160.0, 300.0);
    let (resp, p) = ui.allocate_painter(egui::vec2(side, side), egui::Sense::click_and_drag());
    let r = resp.rect;
    let to_s = |x: f64, y: f64| Pos2::new(r.left() + x as f32 * r.width(), r.bottom() - y as f32 * r.height());
    let to_c = |q: Pos2| (((q.x - r.left()) / r.width()).clamp(0.0, 1.0) as f64, ((r.bottom() - q.y) / r.height()).clamp(0.0, 1.0) as f64);
    p.rect_filled(r, 2.0, Color32::from_gray(28));
    if let Some(h) = hist {
        let n = h.len() as f32;
        for (i, v) in h.iter().enumerate() {
            let x0 = r.left() + i as f32 / n * r.width();
            p.rect_filled(Rect::from_min_max(Pos2::new(x0, r.bottom() - v * r.height() * 0.9), Pos2::new(x0 + r.width() / n + 0.5, r.bottom())), 0.0, Color32::from_gray(55));
        }
    }
    for i in 1..4 {
        let t = i as f32 / 4.0;
        let g = Stroke::new(1.0, Color32::from_gray(50));
        p.line_segment([Pos2::new(r.left() + t * r.width(), r.top()), Pos2::new(r.left() + t * r.width(), r.bottom())], g);
        p.line_segment([Pos2::new(r.left(), r.top() + t * r.height()), Pos2::new(r.right(), r.top() + t * r.height())], g);
    }
    p.line_segment([to_s(0.0, 0.0), to_s(1.0, 1.0)], Stroke::new(1.0, Color32::from_gray(70)));

    let id = resp.id.with("drag_point");
    let near = |q: Pos2, pts: &[[f64; 2]]| pts.iter().enumerate().map(|(i, c)| (i, to_s(c[0], c[1]).distance(q))).filter(|(_, d)| *d < 10.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i);
    let mut changed = false;
    if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked() {
        if let Some(q) = resp.interact_pointer_pos() {
            let i = match near(q, pts) {
                Some(i) => i,
                None => {
                    let (x, y) = to_c(q);
                    if x <= 0.005 || x >= 0.995 {
                        usize::MAX
                    } else {
                        let i = pts.iter().position(|c| c[0] > x).unwrap_or(pts.len());
                        pts.insert(i, [x, y]);
                        changed = true;
                        i
                    }
                }
            };
            ui.data_mut(|d| d.insert_temp(id, i));
        }
    }
    if resp.dragged_by(egui::PointerButton::Primary) {
        let i: usize = ui.data(|d| d.get_temp(id)).unwrap_or(usize::MAX);
        if let (Some(q), true) = (resp.interact_pointer_pos(), i < pts.len()) {
            let (mut x, y) = to_c(q);
            let n = pts.len();
            if i == 0 {
                x = 0.0;
            } else if i == n - 1 {
                x = 1.0;
            } else {
                x = x.clamp(pts[i - 1][0] + 0.01, pts[i + 1][0] - 0.01);
            }
            if pts[i] != [x, y] {
                pts[i] = [x, y];
                changed = true;
            }
        }
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<usize>(id));
    }
    if resp.secondary_clicked() {
        if let Some(i) = resp.interact_pointer_pos().and_then(|q| near(q, pts)) {
            if i > 0 && i + 1 < pts.len() {
                pts.remove(i);
                changed = true;
            }
        }
    }
    let f = finish::curve_fn(pts);
    let line: Vec<Pos2> = (0..=128).map(|t| {
        let x = t as f64 / 128.0;
        to_s(x, f(x))
    }).collect();
    p.line(line, Stroke::new(2.0, colour));
    for c in pts.iter() {
        p.circle(to_s(c[0], c[1]), 4.0, Color32::from_gray(20), Stroke::new(1.5, colour));
    }
    changed
}

/// Hooks for the headless window tests (tests/gui.rs).
#[doc(hidden)]
impl FinishTab {
    pub fn t_status(&self) -> &str {
        &self.status
    }
    pub fn t_loaded(&self) -> Option<(usize, usize)> {
        self.full.as_ref().map(|f| (f.w, f.h))
    }
    pub fn t_shown(&self) -> Option<(usize, usize)> {
        self.shown.as_ref().map(|s| (s.result.w, s.result.h))
    }
    pub fn t_shown_current(&self) -> bool {
        self.shown.as_ref().is_some_and(|s| s.params == self.shown_params() && s.local == self.local())
    }
    pub fn t_set_out(&mut self, p: &str) {
        self.out = p.to_string();
    }
    pub fn t_local(&mut self) -> &mut Local {
        &mut self.mask.local
    }
    /// Paint at (x, y) of the shown image (continuous pixel coordinates).
    pub fn t_paint(&mut self, x: f64, y: f64, sky: bool) {
        if let (Some(s), Some(px)) = (&self.shown, &self.proxy) {
            let (src, geo, out) = ((px.w, px.h), s.params.clone(), (s.result.w, s.result.h));
            self.mask.paint(x, y, src, &geo, out, sky);
        }
    }
    pub fn t_step(&mut self, redo: bool) {
        self.step(redo);
    }
    pub fn t_undo_len(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    pub fn t_save_preset(&mut self, name: &str) {
        self.preset_name = name.into();
        self.save_preset();
    }
    pub fn t_presets(&mut self) -> Vec<String> {
        self.preset_list()
    }
    pub fn t_apply_preset(&mut self, name: &str) {
        self.apply_preset(name);
    }
    pub fn t_cropping(&mut self, on: bool) {
        self.cropping = on;
    }
    pub fn t_save(&mut self) {
        self.save();
    }
}

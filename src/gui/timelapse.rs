//! The "Timelapse" tab: a video from a sequence of frames, with a slow zoom
//! or a slider-like move across the frame if wanted. The frames are listed and
//! previewed; the look is set for the whole frame, the sky and the foreground
//! (through a brush mask) and shown on the selected frame; the video is made
//! in the background (src/timelapse.rs).

use super::dialogs::{self, Picked};
use super::finish::{colour_controls, light_controls, section, temperature_controls};
use super::mask::MaskEditor;
use super::{image_view, texture, JobReport, View};
use crate::finish::{self, FinishParams, Local};
use crate::image::Img;
use crate::io::{self, basename};
use crate::timelapse::{self as tl, Deflicker, Format, Pan, Resolution, Settings};
use egui::{Color32, Stroke};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Where a look applies.
const TARGETS: [&str; 3] = ["Whole frame", "Sky", "Foreground"];
/// Longest side of the frame preview.
const PREVIEW_EDGE: usize = 1600;

/// The selected frame: a preview copy, and the width of the frame as loaded.
struct Frame {
    path: String,
    decoder: io::RawDecoder,
    img: Arc<Img>,
    loaded_w: usize,
}

/// What the look preview was made from.
#[derive(Clone, PartialEq)]
struct LookKey {
    path: String,
    decoder: io::RawDecoder,
    look: FinishParams,
    local: Option<Local>,
    base: Option<FinishParams>,
}

impl LookKey {
    /// Nothing to apply: the frame is shown as it is.
    fn plain(&self) -> bool {
        self.look == FinishParams::default() && self.local.is_none() && self.base.is_none()
    }
}

enum Msg {
    Frame(String, io::RawDecoder, Result<(Img, usize), String>),
    Look(Box<LookKey>, Result<Img, String>),
    Done(Result<tl::Done, String>, f64),
}

#[derive(Clone, Default)]
struct Progress {
    msg: String,
    done: f64,
    total: f64,
    start: Option<Instant>,
}

pub struct TimelapseTab {
    pub simple: bool,
    /// The Post-processing tab's settings while it has an image (set by the window).
    pub finish_look: Option<FinishParams>,
    s: Settings,
    selected: Option<usize>,
    path_edit: String,
    dialog: Option<Receiver<Picked>>,
    status: String,
    use_finish: bool,
    target: usize,
    mask: MaskEditor,
    show_look: bool,
    // The selected frame and its look.
    shown: Option<Frame>,
    tex: Option<egui::TextureHandle>,
    look_tex: Option<(LookKey, egui::TextureHandle)>,
    look_busy: bool,
    loading: Option<String>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    view: View,
    // Making the video.
    running: Option<Settings>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Progress>>,
    report: Option<JobReport>,
    ffmpeg: Option<Option<PathBuf>>,
    /// RAW decoders found on this computer (looked up once).
    decoders: Option<Vec<io::RawDecoder>>,
    /// Frames that are camera RAW, and those of them with a RawTherapee .pp3 file.
    raw_count: (usize, usize),
}

impl Default for TimelapseTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            simple: false,
            finish_look: None,
            s: Settings::default(),
            selected: None,
            path_edit: String::new(),
            dialog: None,
            status: "Add the frames (left), set the look, then make the video (right).".into(),
            use_finish: false,
            target: 0,
            mask: MaskEditor::new(true),
            show_look: true,
            shown: None,
            tex: None,
            look_tex: None,
            look_busy: false,
            loading: None,
            tx,
            rx,
            view: View::default(),
            running: None,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(Mutex::new(Progress::default())),
            report: None,
            ffmpeg: None,
            decoders: None,
            raw_count: (0, 0),
        }
    }
}

fn panic_text() -> String {
    format!("internal error (details in {})", super::crash_log().display())
}

impl TimelapseTab {
    pub fn frames(&self) -> usize {
        self.s.frames.len()
    }
    /// A video is being made.
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    fn ffmpeg(&mut self) -> Option<PathBuf> {
        self.ffmpeg.get_or_insert_with(tl::find_ffmpeg).clone()
    }

    /// The settings as the video would be made now.
    fn settings(&self) -> Settings {
        let mut s = self.s.clone();
        s.local = self.mask.local.clone();
        s.base = if self.use_finish { self.finish_look.clone() } else { None };
        s
    }

    fn add(&mut self, mut ps: Vec<String>) {
        ps.retain(|p| io::is_image(p) && !self.s.frames.contains(p));
        let n = ps.len();
        self.s.frames.extend(ps);
        self.s.frames.sort();
        if self.s.out.is_empty() {
            if let Some(d) = self.s.frames.first().and_then(|f| std::path::Path::new(f).parent()) {
                self.s.out = d.join(format!("timelapse.{}", self.s.format.ext())).to_string_lossy().to_string();
            }
        }
        if self.selected.is_none() && !self.s.frames.is_empty() {
            self.selected = Some(0);
        }
        self.status = format!("Added {n} frame(s); {} in total.", self.s.frames.len());
        self.count_raw();
    }

    fn count_raw(&mut self) {
        let raw: Vec<&String> = self.s.frames.iter().filter(|f| io::is_raw(f)).collect();
        let pp3 = raw.iter().filter(|f| std::path::Path::new(&format!("{f}.pp3")).is_file()).count();
        self.raw_count = (raw.len(), pp3);
    }

    /// How RAW frames are decoded: built-in by default (fast), others when installed.
    fn decoder_ui(&mut self, ui: &mut egui::Ui) {
        let (raw, pp3) = self.raw_count;
        if raw == 0 {
            return;
        }
        let decs = self.decoders.get_or_insert_with(|| {
            let mut v = io::available_raw_decoders();
            v.push(io::RawDecoder::Auto);
            v
        });
        let name = |d: io::RawDecoder| match d {
            io::RawDecoder::Builtin => "built-in (fast)",
            io::RawDecoder::Auto => "auto (RawTherapee where edited)",
            d => d.name(),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label("RAW via");
            egui::ComboBox::from_id_salt("tl_raw_via").selected_text(name(self.s.decoder)).show_ui(ui, |ui| {
                for &d in decs.iter() {
                    ui.selectable_value(&mut self.s.decoder, d, name(d));
                }
            })
            .response
            .on_hover_text("How camera RAW frames are read. The built-in decoder is fast and treats every frame the same. \
                RawTherapee applies the edits saved in its .pp3 files, but takes several seconds per frame.");
        });
        let honoured = matches!(self.s.decoder, io::RawDecoder::RawTherapee | io::RawDecoder::Auto);
        if pp3 > 0 && !honoured {
            let has_rt = decs.contains(&io::RawDecoder::RawTherapee);
            let how = if has_rt { "choose rawtherapee to apply them (several seconds per frame)" } else { "RawTherapee is not installed, so they are not applied" };
            ui.label(egui::RichText::new(format!("{pp3} of {raw} RAW frames have RawTherapee edits (.pp3): {how}.")).weak().size(12.0));
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.dialog {
            if let Ok(r) = rx.try_recv() {
                self.dialog = None;
                match r {
                    Picked::NoTool => self.status = dialogs::NO_DIALOG.into(),
                    Picked::Files(f) => self.add(f),
                    Picked::Save(Some(p)) => self.s.out = p,
                    Picked::Save(None) => {}
                }
            }
        }
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Frame(path, decoder, r) => {
                    if self.loading.as_deref() == Some(path.as_str()) {
                        self.loading = None;
                    }
                    match r {
                        Ok((img, loaded_w)) => {
                            // The same shape keeps the mask (tripod: the foreground stays put).
                            self.mask.fit(img.w, img.h);
                            let fit = self.shown.as_ref().is_none_or(|f| (f.img.w, f.img.h) != (img.w, img.h));
                            self.view.fit |= fit;
                            self.tex = Some(texture(ctx, "timelapse_frame", &img));
                            self.shown = Some(Frame { path, decoder, img: Arc::new(img), loaded_w });
                        }
                        Err(e) => self.status = format!("Cannot show {}: {e}", basename(&path)),
                    }
                }
                Msg::Look(key, r) => {
                    self.look_busy = false;
                    match r {
                        Ok(img) => self.look_tex = Some((*key, texture(ctx, "timelapse_look", &img))),
                        Err(e) => self.status = format!("Preview failed: {e}"),
                    }
                }
                Msg::Done(r, secs) => {
                    let s = self.running.take().unwrap_or_default();
                    self.report = Some(Self::report(&s, &r, secs));
                    self.status = match &r {
                        Ok(d) if d.files > 1 => format!("Wrote {} images ({}).", d.files, super::duration(secs)),
                        Ok(d) => format!("Saved {} ({}).", d.out, super::duration(secs)),
                        Err(e) => format!("No video: {e}"),
                    };
                }
            }
        }
        self.mask.poll();
        if let Some(m) = self.mask.message.take() {
            self.status = m;
        }
        // Show the selected frame.
        let want = self.selected.and_then(|i| self.s.frames.get(i)).cloned();
        if let Some(p) = want {
            let decoder = self.s.decoder;
            let have = self.shown.as_ref().is_some_and(|f| f.path == p && (f.decoder == decoder || !io::is_raw(&p)));
            if !have && self.loading.is_none() {
                self.loading = Some(p.clone());
                let tx = self.tx.clone();
                crate::sys::background(move || {
                    let r = std::panic::catch_unwind(|| {
                        let opt = io::LoadOptions { decoder, half: true, ..Default::default() };
                        io::load_any(&p, &opt).map(|img| {
                            let (long, w) = (img.w.max(img.h), img.w);
                            (if long > PREVIEW_EDGE { crate::image::resize(&img, PREVIEW_EDGE as f64 / long as f64) } else { img }, w)
                        })
                    })
                    .unwrap_or_else(|_| Err(panic_text()));
                    let _ = tx.send(Msg::Frame(p, decoder, r));
                });
            }
        }
        self.kick_look();
    }

    fn look_key(&self) -> Option<LookKey> {
        let f = self.shown.as_ref()?;
        let s = self.settings();
        Some(LookKey { path: f.path.clone(), decoder: f.decoder, look: finish::look_part(&s.look), local: s.local.used().then(|| s.local.clone()), base: s.base })
    }

    /// Show the look on the selected frame (in the background) when it changed.
    fn kick_look(&mut self) {
        if !self.show_look || self.look_busy {
            return;
        }
        let (Some(key), Some(f)) = (self.look_key(), &self.shown) else { return };
        if key.plain() || self.look_tex.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        self.look_busy = true;
        let (img, scale, s, tx) = (f.img.clone(), f.img.w as f64 / f.loaded_w.max(1) as f64, self.settings(), self.tx.clone());
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tl::preview(&img, &s, scale))).map_err(|_| panic_text());
            let _ = tx.send(Msg::Look(Box::new(key), r));
        });
    }

    /// Make the video (in the background).
    fn start(&mut self) {
        if self.running.is_some() {
            return;
        }
        let s = self.settings();
        if s.frames.is_empty() {
            self.status = "Add frames first.".into();
            return;
        }
        if s.out.trim().is_empty() {
            self.status = "Choose where to save the video.".into();
            return;
        }
        if s.format.needs_ffmpeg() && self.ffmpeg().is_none() {
            self.status = tl::NO_FFMPEG.into();
            return;
        }
        if let Err(e) = crate::sys::check_writable(s.out.trim()) {
            self.status = format!("Cannot save: {e}");
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        let t0 = Instant::now();
        *self.progress.lock().unwrap_or_else(|e| e.into_inner()) = Progress { msg: "Starting…".into(), done: 0.0, total: 1.0, start: Some(t0) };
        self.report = None;
        self.running = Some(s.clone());
        let (tx, cancel, prog) = (self.tx.clone(), self.cancel.clone(), self.progress.clone());
        crate::sys::background(move || {
            let progress = |m: &str, done: f64, total: f64| -> Result<(), String> {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                *prog.lock().unwrap_or_else(|e| e.into_inner()) = Progress { msg: m.to_string(), done, total, start: Some(t0) };
                Ok(())
            };
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tl::make(&s, &progress))).unwrap_or_else(|_| Err(panic_text()));
            let _ = tx.send(Msg::Done(r, t0.elapsed().as_secs_f64()));
        });
    }

    fn report(s: &Settings, r: &Result<tl::Done, String>, secs: f64) -> JobReport {
        let n = s.frames.len();
        match r {
            Ok(d) => {
                let len = d.video_frames as f64 / s.fps.max(1) as f64;
                let rep = JobReport::new(if s.format == Format::Sequence { "Images written" } else { "Video made" }, true)
                    .row("Frames", n.to_string())
                    .row("Video", format!("{} × {} px, {} fps, {} ({} frames)", d.w, d.h, s.fps, super::duration(len), d.video_frames))
                    .row("Time", super::duration(secs));
                if s.format == Format::Sequence {
                    let first = format!("{}_0001.{}", std::path::Path::new(&d.out).with_extension("").to_string_lossy(), io::ext_of(&d.out));
                    rep.row("Files", format!("{} images, from {}", d.files, basename(&first)))
                } else {
                    rep.row("File", format!("{} ({})", basename(&d.out), super::file_size(&d.out)))
                }
            }
            Err(e) if e == "Cancelled" => JobReport::new("Video cancelled", false).row("Frames", n.to_string()).row("Time", super::duration(secs)).row("File", "nothing kept"),
            Err(e) => JobReport::new("Video failed", false).row("Frames", n.to_string()).row("Time", super::duration(secs)).row("Error", e.clone()),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.poll(ctx);
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.is_none()) {
            self.view.fit = true;
        }
        egui::Panel::left("tl_left").default_size(380.0).size_range(320.0..=650.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("tll").show(ui, |ui| self.left(ui));
        });
        egui::Panel::right("tl_right").default_size(360.0).size_range(320.0..=600.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("tlr").show(ui, |ui| self.right(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.center(ui, ctx));
        if self.running.is_some() || self.look_busy || self.loading.is_some() || self.mask.detecting() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    // ----------------------------------------------------------------------
    // Left: frames, look, mask
    // ----------------------------------------------------------------------

    fn left(&mut self, ui: &mut egui::Ui) {
        let idle = self.running.is_none();
        ui.heading("Frames");
        ui.add_enabled_ui(idle, |ui| {
            ui.horizontal_wrapped(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.path_edit).desired_width(ui.available_width().min(240.0)).hint_text("file or folder, Enter to add"));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let ps = dialogs::expand_paths(&self.path_edit);
                    self.add(ps);
                }
                if ui.button("Add…").on_hover_text("Add frames: JPEG, TIFF, PNG, camera RAW, FITS").clicked() && self.dialog.is_none() {
                    self.dialog = Some(dialogs::pick_images());
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(self.selected.is_some(), egui::Button::new("Remove")).clicked() {
                    if let Some(i) = self.selected {
                        if i < self.s.frames.len() {
                            self.s.frames.remove(i);
                        }
                        self.selected = if self.s.frames.is_empty() { None } else { Some(i.min(self.s.frames.len() - 1)) };
                    }
                    self.count_raw();
                }
                if ui.add_enabled(!self.s.frames.is_empty(), egui::Button::new("Clear")).clicked() {
                    self.s.frames.clear();
                    self.selected = None;
                    self.count_raw();
                }
                ui.label(format!("{} frame(s)", self.s.frames.len()));
            });
            self.decoder_ui(ui);
        });
        ui.label(egui::RichText::new("Frames play in file-name order (camera numbering). Click one to show it.").weak().size(12.0));
        egui::ScrollArea::vertical().id_salt("tl_list").max_height(180.0).show(ui, |ui| {
            for (i, f) in self.s.frames.iter().enumerate() {
                if ui.selectable_label(self.selected == Some(i), format!("{:>4}  {}", i + 1, basename(f))).clicked() {
                    self.selected = Some(i);
                }
            }
            if self.s.frames.is_empty() {
                ui.label(egui::RichText::new("No frames yet.").weak());
            }
        });
        ui.separator();

        section(ui, "Look", true, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Adjust:");
                for (k, t) in TARGETS.iter().enumerate() {
                    let edited = finish::look_used(self.look(k));
                    if ui.selectable_label(self.target == k, if edited { format!("{t} •") } else { t.to_string() }).clicked() {
                        self.target = k;
                    }
                }
            });
            let note = match self.target {
                0 => "Applies to every pixel of every frame.",
                1 => "On top of the whole-frame look, only where the mask is sky.",
                _ => "On top of the whole-frame look, only where the mask is foreground.",
            };
            ui.label(egui::RichText::new(note).weak().size(12.0));
            if self.target > 0 && !self.mask.has_sky() {
                ui.colored_label(Color32::from_rgb(230, 150, 30), "Paint the mask or detect the sky first (Mask below): nothing is sky yet.");
            }
            let id = ["tl_whole", "tl_sky", "tl_fg"][self.target];
            let k = self.target;
            let p = self.look_mut(k);
            light_controls(ui, p, &format!("{id}_light"));
            temperature_controls(ui, p, &format!("{id}_temp"));
            colour_controls(ui, p, &format!("{id}_col"));
            if ui.small_button(format!("Reset {}", TARGETS[k].to_lowercase())).clicked() {
                *p = FinishParams::default();
            }
        });

        section(ui, "Mask (sky / foreground)", true, |ui| {
            let src = self.shown.as_ref().map(|f| f.img.clone());
            self.mask.controls(ui, src);
            ui.label(egui::RichText::new("One mask serves every frame: on a tripod the foreground does not move.").weak().size(12.0));
        });
    }

    fn look(&self, k: usize) -> &FinishParams {
        match k {
            0 => &self.s.look,
            1 => &self.mask.local.sky,
            _ => &self.mask.local.ground,
        }
    }

    fn look_mut(&mut self, k: usize) -> &mut FinishParams {
        match k {
            0 => &mut self.s.look,
            1 => &mut self.mask.local.sky,
            _ => &mut self.mask.local.ground,
        }
    }

    // ----------------------------------------------------------------------
    // Right: video and output
    // ----------------------------------------------------------------------

    fn right(&mut self, ui: &mut egui::Ui) {
        if let Some(r) = &self.report {
            if r.ui(ui) {
                self.report = None;
            }
            ui.add_space(4.0);
        }
        let idle = self.running.is_none();
        ui.add_enabled_ui(idle, |ui| self.video_settings(ui));
        ui.add_space(6.0);
        if self.running.is_some() {
            let p = self.progress.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let f = if p.total > 0.0 { (p.done / p.total) as f32 } else { 0.0 };
            super::progress_bar(ui, &p.msg, f, p.start);
            if ui.button("✖ Cancel").on_hover_text("Stop; nothing is kept").clicked() {
                self.cancel.store(true, Ordering::Relaxed);
            }
        } else {
            let ff_missing = self.s.format.needs_ffmpeg() && self.ffmpeg().is_none();
            let can = !self.s.frames.is_empty() && !ff_missing;
            let b = ui.add_enabled(can, egui::Button::new(egui::RichText::new("▶ Make video").strong())).on_hover_text("Make the video from all frames, with the look and the settings above");
            let b = if self.s.frames.is_empty() { b.on_disabled_hover_text("Add frames first") } else { b.on_disabled_hover_text(tl::NO_FFMPEG) };
            if b.clicked() {
                self.start();
            }
        }
    }

    fn video_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Video");
        egui::Grid::new("tl_mode").num_columns(2).show(ui, |ui| {
            ui.label("Deflicker");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.s.deflicker, Deflicker::Off, "Off");
                ui.selectable_value(&mut self.s.deflicker, Deflicker::Light, "Light").on_hover_text("Evens out brightness and colour jumps between neighbouring frames");
                ui.selectable_value(&mut self.s.deflicker, Deflicker::Strong, "Strong").on_hover_text("Also evens out slower brightness changes (three times the smoothing)");
            });
            ui.end_row();
            if self.s.deflicker != Deflicker::Off && !self.simple {
                ui.label("Smoothing");
                ui.add(egui::Slider::new(&mut self.s.deflicker_window, 3..=31).suffix(" frames")).on_hover_text("Brightness is evened out over this many neighbouring frames");
                ui.end_row();
            }
        });
        if self.s.deflicker != Deflicker::Off {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.s.deflicker_colour, "Colour too").on_hover_text("Even out each colour on its own, so the colour cast does not jump between frames either (off: brightness only)");
                ui.checkbox(&mut self.s.deflicker_regions, "Region by region").on_hover_text("Even out large-scale changes across the frame (light pollution, passing lights, dawn on one side) on a coarse grid; off: the whole frame alike");
            });
        }
        let have_finish = self.finish_look.is_some();
        ui.add_enabled(have_finish, egui::Checkbox::new(&mut self.use_finish, "Start from the Post-processing tab's look"))
            .on_hover_text("Every frame first gets the stretch, white balance, light, curves, colour and detail set in Post-processing (not its turn and crop), then the look set here")
            .on_disabled_hover_text("Open an image in the Post-processing tab and set the look there first");
        if !have_finish {
            self.use_finish = false;
        }
        ui.separator();
        ui.heading("Camera move");
        egui::Grid::new("tl_motion").num_columns(2).show(ui, |ui| {
            ui.label("Slider move");
            ui.add(egui::Slider::new(&mut self.s.pan, 0.0..=20.0).step_by(0.5).suffix(" %")).on_hover_text("As if the camera rode on a slider: the video shows a part this much smaller than the frame and moves it across the frame (0 = no move)");
            ui.end_row();
            if self.s.pan > 0.0 {
                ui.label("Direction");
                egui::ComboBox::from_id_salt("tl_pan").selected_text(self.s.pan_dir.name()).show_ui(ui, |ui| {
                    for d in Pan::ALL {
                        ui.selectable_value(&mut self.s.pan_dir, d, d.name());
                    }
                });
                ui.end_row();
            }
            ui.label("Slow zoom in");
            ui.add(egui::Slider::new(&mut self.s.zoom, 0.0..=30.0).step_by(0.5).suffix(" %")).on_hover_text("Zoom in slowly over the whole video (0 = still)");
            ui.end_row();
        });
        if self.s.pan > 0.0 || self.s.zoom > 0.0 {
            ui.checkbox(&mut self.s.ease, "Start and stop gently").on_hover_text("The move speeds up at the start and slows down at the end, like a motorised slider; off: the same speed all the way");
            ui.label(egui::RichText::new("The preview outlines where the video starts (bright) and ends (faint).").weak().size(12.0));
        }
        ui.separator();
        ui.heading("Output");
        egui::Grid::new("tl_out").num_columns(2).show(ui, |ui| {
            ui.label("Format");
            let before = self.s.format;
            egui::ComboBox::from_id_salt("tl_fmt").selected_text(self.s.format.name()).show_ui(ui, |ui| {
                for f in Format::ALL {
                    ui.selectable_value(&mut self.s.format, f, f.name());
                }
            });
            if self.s.format != before && !self.s.out.is_empty() {
                self.s.out = io::with_ext(&self.s.out, self.s.format.ext());
            }
            ui.end_row();
            if self.s.format == Format::Sequence {
                ui.label("Image type");
                let cur = io::ext_of(&self.s.out);
                let name = match cur.as_str() {
                    "png" => "PNG (16-bit)",
                    "jpg" | "jpeg" => "JPEG",
                    _ => "TIFF (16-bit)",
                };
                egui::ComboBox::from_id_salt("tl_seq").selected_text(name).show_ui(ui, |ui| {
                    for (e, l) in [("tif", "TIFF (16-bit)"), ("png", "PNG (16-bit)"), ("jpg", "JPEG")] {
                        if ui.selectable_label(cur == e, l).clicked() {
                            self.s.out = io::with_ext(&self.s.out, e);
                        }
                    }
                });
                ui.end_row();
            }
            ui.label("Frame rate");
            egui::ComboBox::from_id_salt("tl_fps").selected_text(format!("{} fps", self.s.fps)).show_ui(ui, |ui| {
                for f in [12, 24, 25, 30, 50, 60] {
                    ui.selectable_value(&mut self.s.fps, f, format!("{f} fps"));
                }
            });
            ui.end_row();
            ui.label("Resolution");
            egui::ComboBox::from_id_salt("tl_res").selected_text(self.s.resolution.name()).show_ui(ui, |ui| {
                for r in Resolution::ALL {
                    ui.selectable_value(&mut self.s.resolution, r, r.name());
                }
            });
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut self.s.crop_16_9, "Crop to 16:9").on_hover_text("Cut the 3:2 camera frame to a widescreen video (the preview outlines the cut)");
            ui.end_row();
            let jpeg = self.s.format == Format::Sequence && matches!(io::ext_of(&self.s.out).as_str(), "jpg" | "jpeg");
            if self.s.format.needs_ffmpeg() || jpeg {
                ui.label("Quality");
                ui.add(egui::Slider::new(&mut self.s.quality, 30..=100)).on_hover_text("Higher = better picture, bigger file");
                ui.end_row();
            }
            ui.label("");
            ui.checkbox(&mut self.s.bounce, "Play forward, then back").on_hover_text("The frames again in reverse at the end: loops smoothly");
            ui.end_row();
            if !self.simple {
                ui.label("Hold last frame");
                ui.add(egui::Slider::new(&mut self.s.hold_last, 0.0..=5.0).suffix(" s").step_by(0.5));
                ui.end_row();
            }
        });
        if self.s.format.needs_ffmpeg() {
            match self.ffmpeg() {
                Some(p) => {
                    ui.label(egui::RichText::new(format!("Made with ffmpeg ({})", p.display())).weak().size(12.0));
                }
                None => {
                    ui.colored_label(Color32::from_rgb(230, 150, 30), tl::NO_FFMPEG);
                    if ui.small_button("Look for ffmpeg again").clicked() {
                        self.ffmpeg = None;
                    }
                }
            }
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Save as");
            ui.add(egui::TextEdit::singleline(&mut self.s.out).desired_width(ui.available_width().min(200.0)).hint_text("output file"));
            if ui.button("…").on_hover_text("Choose the file").clicked() && self.dialog.is_none() {
                self.dialog = Some(dialogs::pick_save("Save video", &self.s.out));
            }
        });
        if self.s.format == Format::Sequence {
            let stem = std::path::Path::new(&self.s.out).with_extension("");
            ui.label(egui::RichText::new(format!("Writes {}_0001.{}, _0002 … in that folder.", basename(&stem.to_string_lossy()), io::ext_of(&self.s.out))).weak().size(12.0));
        } else if std::path::Path::new(&self.s.out).exists() {
            ui.colored_label(Color32::from_rgb(230, 150, 30), "This file exists and will be replaced.");
        }
        ui.add_space(4.0);
        let n = self.s.frames.len();
        let total = if n == 0 { 0.0 } else { n as f64 + (self.s.hold_last * self.s.fps as f64).round() + if self.s.bounce && n > 2 { (n - 2) as f64 } else { 0.0 } };
        // The frame as the video loads it: the preview read RAW at half size, the video does so only up to 1080p.
        let size = self.shown.as_ref().map(|f| {
            let full = io::is_raw(&f.path) && !self.s.resolution.limit().is_some_and(|(lw, _)| lw <= 1920);
            let k = f.loaded_w as f64 / f.img.w as f64 * if full { 2.0 } else { 1.0 };
            tl::video_size((f.img.w as f64 * k).round() as usize, (f.img.h as f64 * k).round() as usize, &self.s)
        });
        let size_txt = size.map(|(w, h)| format!(", {w} × {h} px")).unwrap_or_default();
        ui.label(format!("{n} frames at {} fps → {} of video{size_txt}", self.s.fps, super::duration(total / self.s.fps.max(1) as f64)));
        if self.s.format == Format::Gif {
            if size.is_some_and(|(w, h)| w.max(h) > 1280) {
                ui.colored_label(Color32::from_rgb(230, 150, 30), "GIFs get very large above 720p: choose 720p or smaller.");
            }
            ui.label(egui::RichText::new("GIF suits short clips for forums and chats. Star fields compress badly: about 1 MB per frame at 720p. MP4 is 5–10 times smaller.").weak().size(12.0));
        }
    }

    // ----------------------------------------------------------------------
    // Centre: the selected frame with the look and the mask
    // ----------------------------------------------------------------------

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Fit").on_hover_text("Whole frame in view (F)").clicked() {
                self.view.fit = true;
            }
            let n = self.s.frames.len();
            if ui.add_enabled(n > 0, egui::Button::new("◀")).on_hover_text("Previous frame").clicked() {
                self.selected = Some(self.selected.unwrap_or(0).saturating_sub(1));
            }
            if ui.add_enabled(n > 0, egui::Button::new("▶")).on_hover_text("Next frame").clicked() {
                self.selected = Some((self.selected.map_or(0, |i| i + 1)).min(n.saturating_sub(1)));
            }
            if let (Some(i), Some(f)) = (self.selected, &self.shown) {
                ui.label(format!("Frame {} of {n}: {}", i + 1, basename(&f.path)));
            }
            ui.checkbox(&mut self.show_look, "Show the look").on_hover_text("The selected frame with the look and the mask applied (off: as it is). Deflicker and the camera move show only in the video");
            if self.loading.is_some() || self.look_busy {
                ui.spinner();
            }
            if self.mask.painting {
                ui.colored_label(Color32::from_rgb(120, 170, 255), "Painting: left = sky, right = foreground, wheel = brush size");
            }
        });
        ui.label(egui::RichText::new(&self.status).size(12.5));
        let (Some(f), Some(raw)) = (&self.shown, &self.tex) else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Timelapse").size(22.0).weak());
                ui.add_space(6.0);
                ui.label(egui::RichText::new(if self.s.frames.is_empty() { "Add frames on the left; the selected frame shows here with its look, for painting the mask." } else { "Loading the frame…" }).weak());
            });
            return;
        };
        let (iw, ih) = (f.img.w, f.img.h);
        let (w, h) = (iw as f64, ih as f64);
        let key = if self.show_look { self.look_key() } else { None };
        let tex = match (&key, &self.look_tex) {
            (Some(k), Some((lk, t))) if !k.plain() && lk.path == k.path => t.clone(),
            _ => raw.clone(),
        };
        let geo = FinishParams::default();
        self.mask.wheel(ui);
        let ov = self.mask.overlay(ctx, (iw, ih), &geo, (iw, ih));
        let b = [0.5, w + 0.5, 0.5, h + 0.5];
        let mut layers = vec![(&tex, b)];
        if let Some(t) = &ov {
            layers.push((t, b));
        }
        let s = image_view(ui, &mut self.view, (w, h), &layers, !self.mask.painting);
        // The part of the frame the video shows: at the start, and at the end of the move or zoom.
        if self.s.crop_16_9 || self.s.zoom > 0.0 || self.s.pan > 0.0 {
            let p = ui.painter().with_clip_rect(s.rect);
            let ends: &[f64] = if self.s.zoom > 0.0 || self.s.pan > 0.0 { &[0.0, 1.0] } else { &[0.0] };
            for &t in ends {
                let c = tl::frame_crop(iw, ih, &self.s, t).crop;
                let r = egui::Rect::from_min_max(s.to_screen(c[0] * w + 0.5, c[1] * h + 0.5), s.to_screen((1.0 - c[2]) * w + 0.5, (1.0 - c[3]) * h + 0.5));
                p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, if t == 0.0 { 160 } else { 90 })), egui::StrokeKind::Middle);
            }
        }
        self.mask.input(ui, &s, (iw, ih), &geo, (iw, ih));
    }
}

/// Hooks for the headless window tests (tests/gui.rs).
#[doc(hidden)]
impl TimelapseTab {
    pub fn t_add(&mut self, ps: Vec<String>) {
        self.add(ps);
    }
    pub fn t_shown(&self) -> Option<(usize, usize)> {
        self.shown.as_ref().map(|f| (f.img.w, f.img.h))
    }
    pub fn t_mask(&self) -> (usize, usize, usize) {
        let m = &self.mask.local.mask;
        (m.h, m.w, m.count())
    }
    pub fn t_paint(&mut self, x: f64, y: f64, sky: bool) {
        if let Some(f) = &self.shown {
            let d = (f.img.w, f.img.h);
            self.mask.paint(x, y, d, &FinishParams::default(), d, sky);
        }
    }
    pub fn t_look(&mut self, k: usize) -> &mut FinishParams {
        self.look_mut(k)
    }
    /// The look preview is up to date with the settings.
    pub fn t_look_shown(&self) -> bool {
        match (self.look_key(), &self.look_tex) {
            (Some(k), _) if k.plain() => true,
            (Some(k), Some((lk, _))) => *lk == k,
            _ => false,
        }
    }
    pub fn t_settings(&mut self) -> &mut Settings {
        &mut self.s
    }
    pub fn t_make(&mut self) {
        self.start();
    }
    pub fn t_running(&self) -> bool {
        self.running.is_some()
    }
    pub fn t_report(&self) -> Option<&JobReport> {
        self.report.as_ref()
    }
    pub fn t_status(&self) -> &str {
        &self.status
    }
}

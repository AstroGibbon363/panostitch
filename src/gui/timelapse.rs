//! The "Timelapse" tab: making a video from a sequence of frames (plain
//! time-lapse, growing star trails, comet-style trails). For now only the
//! controls: the frames can be listed and previewed, the look set for the whole
//! frame, the sky and the foreground (with a brush mask), and the output
//! chosen, but no video is made yet.

use super::dialogs::{self, Picked};
use super::finish::{colour_controls, light_controls, section, temperature_controls};
use super::{image_view, texture, View};
use crate::finish::FinishParams;
use crate::image::{Bits, Img};
use crate::io::{self, basename};
use egui::{Color32, Stroke};
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Plain,
    TrailsGrowing,
    TrailsComet,
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::Plain, Mode::TrailsGrowing, Mode::TrailsComet];
    fn name(self) -> &'static str {
        match self {
            Mode::Plain => "Time-lapse (frames as they are)",
            Mode::TrailsGrowing => "Star trails, growing",
            Mode::TrailsComet => "Star trails, comet-style (fading tails)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Mp4,
    WebM,
    Gif,
    Sequence,
}

impl Format {
    const ALL: [Format; 4] = [Format::Mp4, Format::WebM, Format::Gif, Format::Sequence];
    fn name(self) -> &'static str {
        match self {
            Format::Mp4 => "MP4 video (H.264)",
            Format::WebM => "WebM video (VP9)",
            Format::Gif => "Animated GIF",
            Format::Sequence => "Numbered images (TIFF / PNG / JPEG)",
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::WebM => "webm",
            Format::Gif => "gif",
            Format::Sequence => "tif",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolution {
    Frames,
    Uhd,
    Qhd,
    FullHd,
    Hd,
}

impl Resolution {
    const ALL: [Resolution; 5] = [Resolution::Frames, Resolution::Uhd, Resolution::Qhd, Resolution::FullHd, Resolution::Hd];
    fn name(self) -> &'static str {
        match self {
            Resolution::Frames => "As the frames",
            Resolution::Uhd => "4K UHD (3840 × 2160)",
            Resolution::Qhd => "1440p (2560 × 1440)",
            Resolution::FullHd => "1080p (1920 × 1080)",
            Resolution::Hd => "720p (1280 × 720)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Deflicker {
    Off,
    Light,
    Strong,
}

/// Where a look applies.
const TARGETS: [&str; 3] = ["Whole frame", "Sky", "Foreground"];
/// Longest side of the frame preview.
const PREVIEW_EDGE: usize = 1600;
/// Width of the mask grid (its height follows the frame's shape).
const MASK_W: usize = 480;

pub struct TimelapseTab {
    pub simple: bool,
    frames: Vec<String>,
    selected: Option<usize>,
    path_edit: String,
    dialog: Option<Receiver<Picked>>,
    status: String,
    mode: Mode,
    fade: u32,
    deflicker: Deflicker,
    deflicker_window: u32,
    stabilize: bool,
    use_finish: bool,
    zoom: f64,
    // The look: whole frame, sky, foreground.
    looks: [FinishParams; 3],
    target: usize,
    // The mask (true = sky), painted on the frame preview.
    mask: Bits,
    mask_rev: u64,
    brush: f64,
    painting: bool,
    show_mask: bool,
    feather: f64,
    // Frame preview.
    shown: Option<(String, Img)>,
    tex: Option<egui::TextureHandle>,
    overlay: Option<(u64, egui::TextureHandle)>,
    loading: Option<String>,
    tx: Sender<(String, Result<Img, String>)>,
    rx: Receiver<(String, Result<Img, String>)>,
    view: View,
    // Output.
    format: Format,
    fps: u32,
    resolution: Resolution,
    crop_16_9: bool,
    quality: u32,
    bounce: bool,
    hold_last: f64,
    out: String,
}

impl Default for TimelapseTab {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            simple: false,
            frames: Vec::new(),
            selected: None,
            path_edit: String::new(),
            dialog: None,
            status: "Video export is not available yet: this tab collects the frames and settings for now.".into(),
            mode: Mode::Plain,
            fade: 30,
            deflicker: Deflicker::Light,
            deflicker_window: 7,
            stabilize: false,
            use_finish: false,
            zoom: 0.0,
            looks: [FinishParams::default(), FinishParams::default(), FinishParams::default()],
            target: 0,
            mask: Bits::new(0, 0),
            mask_rev: 0,
            brush: 4.0,
            painting: false,
            show_mask: true,
            feather: 2.0,
            shown: None,
            tex: None,
            overlay: None,
            loading: None,
            tx,
            rx,
            view: View::default(),
            format: Format::Mp4,
            fps: 25,
            resolution: Resolution::FullHd,
            crop_16_9: true,
            quality: 80,
            bounce: false,
            hold_last: 0.0,
            out: String::new(),
        }
    }
}

const NOT_YET: &str = "Video export comes in a later version. For now this tab only collects the frames and settings.";

impl TimelapseTab {
    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    fn add(&mut self, mut ps: Vec<String>) {
        ps.retain(|p| io::is_image(p) && !self.frames.contains(p));
        let n = ps.len();
        self.frames.extend(ps);
        self.frames.sort();
        if self.out.is_empty() {
            if let Some(d) = self.frames.first().and_then(|f| std::path::Path::new(f).parent()) {
                self.out = d.join(format!("timelapse.{}", self.format.ext())).to_string_lossy().to_string();
            }
        }
        if self.selected.is_none() && !self.frames.is_empty() {
            self.selected = Some(0);
        }
        self.status = format!("Added {n} frame(s); {} in total.", self.frames.len());
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.dialog {
            if let Ok(r) = rx.try_recv() {
                self.dialog = None;
                match r {
                    Picked::NoTool => self.status = dialogs::NO_DIALOG.into(),
                    Picked::Files(f) => self.add(f),
                    Picked::Save(_) => {}
                }
            }
        }
        while let Ok((path, r)) = self.rx.try_recv() {
            if self.loading.as_deref() == Some(path.as_str()) {
                self.loading = None;
            }
            match r {
                Ok(img) => {
                    // A new frame shape needs a new mask; the same shape keeps it (tripod: the foreground stays put).
                    let mh = ((MASK_W as f64 * img.h as f64 / img.w as f64).round() as usize).max(1);
                    if (self.mask.h, self.mask.w) != (mh, MASK_W) {
                        self.mask = Bits::new(mh, MASK_W);
                        self.mask_rev += 1;
                    }
                    let fit = self.shown.as_ref().is_none_or(|(_, s)| (s.w, s.h) != (img.w, img.h));
                    self.view.fit |= fit;
                    self.tex = Some(texture(ctx, "timelapse_frame", &img));
                    self.shown = Some((path, img));
                }
                Err(e) => self.status = format!("Cannot show {}: {e}", basename(&path)),
            }
        }
        // Show the selected frame.
        let want = self.selected.and_then(|i| self.frames.get(i)).cloned();
        if let Some(p) = want {
            let have = self.shown.as_ref().map(|(s, _)| s.as_str()) == Some(p.as_str());
            if !have && self.loading.is_none() {
                self.loading = Some(p.clone());
                let tx = self.tx.clone();
                crate::sys::background(move || {
                    let r = std::panic::catch_unwind(|| {
                        let opt = io::LoadOptions { half: true, ..Default::default() };
                        io::load_any(&p, &opt).map(|img| {
                            let long = img.w.max(img.h);
                            if long > PREVIEW_EDGE { crate::image::resize(&img, PREVIEW_EDGE as f64 / long as f64) } else { img }
                        })
                    })
                    .unwrap_or_else(|_| Err("internal error".into()));
                    let _ = tx.send((p, r));
                });
            }
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
    }

    // ----------------------------------------------------------------------
    // Left: frames, look, mask
    // ----------------------------------------------------------------------

    fn left(&mut self, ui: &mut egui::Ui) {
        ui.heading("Frames");
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
                    if i < self.frames.len() {
                        self.frames.remove(i);
                    }
                    self.selected = if self.frames.is_empty() { None } else { Some(i.min(self.frames.len() - 1)) };
                }
            }
            if ui.add_enabled(!self.frames.is_empty(), egui::Button::new("Clear")).clicked() {
                self.frames.clear();
                self.selected = None;
            }
            ui.label(format!("{} frame(s)", self.frames.len()));
        });
        ui.label(egui::RichText::new("Frames play in file-name order (camera numbering). Click one to show it.").weak().size(12.0));
        egui::ScrollArea::vertical().id_salt("tl_list").max_height(180.0).show(ui, |ui| {
            for (i, f) in self.frames.iter().enumerate() {
                if ui.selectable_label(self.selected == Some(i), format!("{:>4}  {}", i + 1, basename(f))).clicked() {
                    self.selected = Some(i);
                }
            }
            if self.frames.is_empty() {
                ui.label(egui::RichText::new("No frames yet.").weak());
            }
        });
        ui.separator();

        section(ui, "Look", true, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Adjust:");
                for (k, t) in TARGETS.iter().enumerate() {
                    let edited = k > 0 && self.looks[k] != FinishParams::default();
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
            if self.target > 0 && !self.mask.any() {
                ui.colored_label(Color32::from_rgb(230, 150, 30), "Paint the mask first (Mask below): nothing is sky yet.");
            }
            let id = ["tl_whole", "tl_sky", "tl_fg"][self.target];
            let p = &mut self.looks[self.target];
            light_controls(ui, p, &format!("{id}_light"));
            temperature_controls(ui, p, &format!("{id}_temp"));
            colour_controls(ui, p, &format!("{id}_col"));
            if ui.small_button(format!("Reset {}", TARGETS[self.target].to_lowercase())).clicked() {
                *p = FinishParams::default();
            }
        });

        section(ui, "Mask (sky / foreground)", true, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.toggle_value(&mut self.painting, "✎ Paint").on_hover_text("Paint on the frame: left button = sky, right button = foreground, mouse wheel = brush size");
                ui.checkbox(&mut self.show_mask, "Show mask");
            });
            egui::Grid::new("tl_mask").num_columns(2).show(ui, |ui| {
                ui.label("Brush size (% width)");
                ui.add(egui::Slider::new(&mut self.brush, 0.5..=30.0).logarithmic(true));
                ui.end_row();
                ui.label("Soft edge (% width)");
                ui.add(egui::Slider::new(&mut self.feather, 0.0..=10.0).step_by(0.5)).on_hover_text("How softly sky and foreground adjustments blend at the mask edge (used when the video is made)");
                ui.end_row();
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("All sky").clicked() {
                    self.fill_mask(true);
                }
                if ui.button("Invert").clicked() {
                    let (h, w) = (self.mask.h, self.mask.w);
                    let old = self.mask.clone();
                    self.mask = Bits::from_fn(h, w, |y, x| !old.at(y, x));
                    self.mask_rev += 1;
                }
                if ui.button("Clear").on_hover_text("Everything foreground").clicked() {
                    self.fill_mask(false);
                }
                ui.add_enabled(false, egui::Button::new("Detect sky")).on_disabled_hover_text("Comes with the video export (it will use the stacking tab's sky detection)");
            });
            if self.mask.w > 0 {
                let pct = 100.0 * self.mask.count() as f64 / self.mask.len().max(1) as f64;
                ui.label(egui::RichText::new(format!("Sky: {pct:.0} % of the frame. One mask serves every frame: on a tripod the foreground does not move.")).weak().size(12.0));
            } else {
                ui.label(egui::RichText::new("Add frames to paint the mask on.").weak().size(12.0));
            }
        });
    }

    fn fill_mask(&mut self, sky: bool) {
        let (h, w) = (self.mask.h, self.mask.w);
        self.mask = Bits::from_fn(h, w, |_, _| sky);
        self.mask_rev += 1;
    }

    // ----------------------------------------------------------------------
    // Right: video and output
    // ----------------------------------------------------------------------

    fn right(&mut self, ui: &mut egui::Ui) {
        ui.heading("Video");
        egui::Grid::new("tl_mode").num_columns(2).show(ui, |ui| {
            ui.label("Kind");
            egui::ComboBox::from_id_salt("tl_kind").selected_text(self.mode.name()).show_ui(ui, |ui| {
                for m in Mode::ALL {
                    ui.selectable_value(&mut self.mode, m, m.name());
                }
            });
            ui.end_row();
            if self.mode == Mode::TrailsComet {
                ui.label("Tail length");
                ui.add(egui::Slider::new(&mut self.fade, 2..=200).suffix(" frames")).on_hover_text("How many frames a star's tail lasts before it fades out");
                ui.end_row();
            }
            ui.label("Deflicker");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.deflicker, Deflicker::Off, "Off");
                ui.selectable_value(&mut self.deflicker, Deflicker::Light, "Light");
                ui.selectable_value(&mut self.deflicker, Deflicker::Strong, "Strong");
            });
            ui.end_row();
            if self.deflicker != Deflicker::Off && !self.simple {
                ui.label("Smoothing");
                ui.add(egui::Slider::new(&mut self.deflicker_window, 3..=31).suffix(" frames")).on_hover_text("Brightness is evened out over this many neighbouring frames");
                ui.end_row();
            }
        });
        ui.checkbox(&mut self.stabilize, "Steady the camera").on_hover_text("Line up the foreground of every frame (wind, a bumped tripod)");
        ui.checkbox(&mut self.use_finish, "Start from the Post-processing tab's look").on_hover_text("Every frame first gets the stretch, curves and the rest set in Post-processing, then the look set here");
        if !self.simple {
            egui::Grid::new("tl_motion").num_columns(2).show(ui, |ui| {
                ui.label("Slow zoom in");
                ui.add(egui::Slider::new(&mut self.zoom, 0.0..=30.0).suffix(" %")).on_hover_text("Zoom in slowly over the whole video (0 = still)");
                ui.end_row();
            });
        }
        ui.separator();
        ui.heading("Output");
        egui::Grid::new("tl_out").num_columns(2).show(ui, |ui| {
            ui.label("Format");
            let before = self.format;
            egui::ComboBox::from_id_salt("tl_fmt").selected_text(self.format.name()).show_ui(ui, |ui| {
                for f in Format::ALL {
                    ui.selectable_value(&mut self.format, f, f.name());
                }
            });
            if self.format != before && !self.out.is_empty() {
                self.out = io::with_ext(&self.out, self.format.ext());
            }
            ui.end_row();
            ui.label("Frame rate");
            egui::ComboBox::from_id_salt("tl_fps").selected_text(format!("{} fps", self.fps)).show_ui(ui, |ui| {
                for f in [12, 24, 25, 30, 50, 60] {
                    ui.selectable_value(&mut self.fps, f, format!("{f} fps"));
                }
            });
            ui.end_row();
            ui.label("Resolution");
            egui::ComboBox::from_id_salt("tl_res").selected_text(self.resolution.name()).show_ui(ui, |ui| {
                for r in Resolution::ALL {
                    ui.selectable_value(&mut self.resolution, r, r.name());
                }
            });
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut self.crop_16_9, "Crop to 16:9").on_hover_text("Cut the 3:2 camera frame to a widescreen video");
            ui.end_row();
            if matches!(self.format, Format::Mp4 | Format::WebM) {
                ui.label("Quality");
                ui.add(egui::Slider::new(&mut self.quality, 30..=100)).on_hover_text("Higher = better picture, bigger file");
                ui.end_row();
            }
            if self.format == Format::Gif {
                ui.label("");
                ui.checkbox(&mut self.bounce, "Play forward, then back");
                ui.end_row();
            }
            if !self.simple {
                ui.label("Hold last frame");
                ui.add(egui::Slider::new(&mut self.hold_last, 0.0..=5.0).suffix(" s").step_by(0.5));
                ui.end_row();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Save as");
            ui.add(egui::TextEdit::singleline(&mut self.out).desired_width(ui.available_width().min(240.0)).hint_text("output file"));
        });
        ui.add_space(4.0);
        // How long the video runs: a plain calculation, the one part that already works.
        let n = self.frames.len() as f64 * if self.bounce && self.format == Format::Gif { 2.0 } else { 1.0 };
        let secs = n / self.fps.max(1) as f64 + if n > 0.0 { self.hold_last } else { 0.0 };
        ui.label(format!("{} frames at {} fps → {} of video", self.frames.len(), self.fps, super::duration(secs)));
        ui.add_space(6.0);
        ui.add_enabled(false, egui::Button::new(egui::RichText::new("▶ Make video").strong())).on_disabled_hover_text(NOT_YET);
        ui.colored_label(Color32::from_rgb(230, 150, 30), "Not available yet: the look and mask are not applied and no video is made.");
    }

    // ----------------------------------------------------------------------
    // Centre: the selected frame with the mask
    // ----------------------------------------------------------------------

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Fit").on_hover_text("Whole frame in view (F)").clicked() {
                self.view.fit = true;
            }
            let n = self.frames.len();
            if ui.add_enabled(n > 0, egui::Button::new("◀")).on_hover_text("Previous frame").clicked() {
                self.selected = Some(self.selected.unwrap_or(0).saturating_sub(1));
            }
            if ui.add_enabled(n > 0, egui::Button::new("▶")).on_hover_text("Next frame").clicked() {
                self.selected = Some((self.selected.map_or(0, |i| i + 1)).min(n.saturating_sub(1)));
            }
            if let (Some(i), Some((p, _))) = (self.selected, &self.shown) {
                ui.label(format!("Frame {} of {n}: {}", i + 1, basename(p)));
            }
            if self.loading.is_some() {
                ui.spinner();
            }
            if self.painting {
                ui.colored_label(Color32::from_rgb(120, 170, 255), "Painting: left = sky, right = foreground, wheel = brush size");
            }
        });
        ui.label(egui::RichText::new(&self.status).size(12.5));
        let (Some((_, img)), Some(tex)) = (&self.shown, &self.tex) else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Timelapse").size(22.0).weak());
                ui.add_space(6.0);
                ui.label(egui::RichText::new(if self.frames.is_empty() { "Add frames on the left; the selected frame shows here for painting the mask." } else { "Loading the frame…" }).weak());
                ui.label(egui::RichText::new("Planned: a timeline preview, deflicker, growing or fading star trails,\nthe look on every frame (sky and foreground separately), and MP4 / WebM / GIF output.").weak());
            });
            return;
        };
        let (w, h) = (img.w as f64, img.h as f64);
        let tex = tex.clone();
        // The mask overlay: sky tinted blue, foreground orange.
        if self.show_mask && self.mask.w > 0 && self.overlay.as_ref().is_none_or(|(r, _)| *r != self.mask_rev) {
            let (mh, mw) = (self.mask.h, self.mask.w);
            let mut rgba = Vec::with_capacity(mh * mw * 4);
            for y in 0..mh {
                for x in 0..mw {
                    rgba.extend_from_slice(if self.mask.at(y, x) { &[60, 120, 255, 70] } else { &[255, 150, 40, 45] });
                }
            }
            self.overlay = Some((self.mask_rev, ctx.load_texture("timelapse_mask", egui::ColorImage::from_rgba_unmultiplied([mw, mh], &rgba), egui::TextureOptions::NEAREST)));
        }
        let b = [0.5, w + 0.5, 0.5, h + 0.5];
        // While painting, the mouse wheel sets the brush size instead of zooming.
        if self.painting && ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                self.brush = (self.brush * 1.15f64.powf((dy / 50.0) as f64)).clamp(0.5, 30.0);
                ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
        let mut layers = vec![(&tex, b)];
        let ov = self.overlay.as_ref().map(|(_, t)| t.clone());
        if self.show_mask {
            if let Some(t) = &ov {
                layers.push((t, b));
            }
        }
        let s = image_view(ui, &mut self.view, (w, h), &layers, !self.painting);
        if !self.painting {
            return;
        }
        let Some(pos) = s.resp.hover_pos() else { return };
        ui.painter().with_clip_rect(s.rect).circle_stroke(pos, (self.brush / 100.0 * w) as f32 * s.zoom, Stroke::new(1.5, Color32::LIGHT_BLUE));
        let (pd, sd) = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down()));
        if (pd || sd) && s.resp.is_pointer_button_down_on() && self.mask.w > 0 {
            self.paint(&s.to_image(pos), w, h, pd);
        }
    }

    /// Paint a round brush stroke at image position `at` (sky if `sky`, else foreground).
    fn paint(&mut self, at: &(f64, f64), w: f64, h: f64, sky: bool) {
        let (mh, mw) = (self.mask.h, self.mask.w);
        let (cx, cy) = ((at.0 - 0.5) / w * mw as f64, (at.1 - 0.5) / h * mh as f64);
        let rr = self.brush / 100.0 * mw as f64;
        let mut changed = false;
        for r in ((cy - rr).floor().max(0.0) as usize)..((cy + rr).ceil().max(0.0) as usize).min(mh) {
            for c in ((cx - rr).floor().max(0.0) as usize)..((cx + rr).ceil().max(0.0) as usize).min(mw) {
                if (c as f64 + 0.5 - cx).powi(2) + (r as f64 + 0.5 - cy).powi(2) <= rr * rr && self.mask.at(r, c) != sky {
                    self.mask.set(r, c, sky);
                    changed = true;
                }
            }
        }
        if changed {
            self.mask_rev += 1;
        }
    }
}

/// Hooks for the headless window tests (tests/gui.rs).
#[doc(hidden)]
impl TimelapseTab {
    pub fn t_add(&mut self, ps: Vec<String>) {
        self.add(ps);
    }
    pub fn t_shown(&self) -> Option<(usize, usize)> {
        self.shown.as_ref().map(|(_, i)| (i.w, i.h))
    }
    pub fn t_mask(&self) -> (usize, usize, usize) {
        (self.mask.h, self.mask.w, self.mask.count())
    }
    pub fn t_paint(&mut self, x: f64, y: f64, sky: bool) {
        if let Some((_, img)) = &self.shown {
            let (w, h) = (img.w as f64, img.h as f64);
            self.paint(&(x, y), w, h, sky);
        }
    }
    pub fn t_look(&mut self, k: usize) -> &mut FinishParams {
        &mut self.looks[k]
    }
}

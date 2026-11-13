//! The "Post-processing" tab: the last steps on a stacked or stitched image
//! (turn and crop, stretch, white balance, tone, curves, colour, noise, stars,
//! sharpening) and saving it at the size you want. A small copy gives a live
//! preview; the full image is processed when saving.

use super::dialogs::{self, Picked};
use super::{image_view, texture, View};
use crate::finish::{self, identity_curve, Display, FinishParams, Measured, StretchMode};
use crate::image::Img;
use crate::io::{self, basename};
use crate::sys::gb;
use egui::{Color32, Pos2, Rect, Stroke};
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
}

enum Msg {
    Loaded(u64, Result<Loaded, String>),
    Preview(u64, Result<Box<Preview>, String>),
    Saved(Result<(String, Box<FinishParams>), String>),
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
    display: Display,
    shown: Option<Preview>,
    tex: Option<egui::TextureHandle>,
    before_tex: Option<egui::TextureHandle>,
    tex_display: Display,
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
    saved: Option<FinishParams>,
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
            display: Display::Linear,
            shown: None,
            tex: None,
            before_tex: None,
            tex_display: Display::Linear,
            gen: 0,
            loading: false,
            previewing: false,
            saving: false,
            tx,
            rx,
            view: View::default(),
            status: "Open an image (Ctrl+O), or send a result here from the other tabs.".into(),
            out: String::new(),
            saved: None,
            dialog: None,
            sampling: false,
            sample_from: None,
            curve_ch: 0,
        }
    }
}

fn panic_text() -> String {
    format!("internal error (details in {})", super::crash_log().display())
}

impl FinishTab {
    pub fn busy(&self) -> bool {
        self.loading || self.previewing || self.saving
    }
    pub fn saving(&self) -> bool {
        self.saving
    }
    /// Changes not saved to a file yet.
    pub fn unsaved(&self) -> bool {
        self.full.is_some() && self.saved.as_ref() != Some(&self.p) && self.p != FinishParams::for_input(self.linear)
    }

    /// Load an image (in the background).
    pub fn open(&mut self, path: &str) {
        let path = path.trim().to_string();
        if path.is_empty() {
            return;
        }
        if self.loading || self.saving {
            self.status = "Wait until the current image is loaded or saved.".into();
            return;
        }
        self.loading = true;
        self.gen += 1;
        self.status = format!("Loading {} …", basename(&path));
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
                            }
                            self.status = format!(
                                "Opened {} ({}×{} px){}. Adjust on the left, save on the right.",
                                basename(&l.path),
                                l.full.w,
                                l.full.h,
                                if self.linear { ", linear data: stretched automatically" } else { "" }
                            );
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
                        }
                        Err(e) => self.status = format!("Cannot open: {e}"),
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
                            self.tex = Some(texture(ctx, "finish", &finish::display(&pv.result, self.display)));
                            self.tex_display = self.display;
                            self.before_tex = None;
                            self.shown = Some(*pv);
                        }
                        Err(e) => self.status = format!("Preview failed: {e}"),
                    }
                }
                Msg::Saved(r) => {
                    self.saving = false;
                    match r {
                        Ok((s, p)) => {
                            self.status = s;
                            self.saved = Some(*p);
                        }
                        Err(e) => self.status = format!("Not saved: {e}"),
                    }
                }
            }
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
                self.tex = Some(texture(ctx, "finish", &finish::display(&s.result, self.display)));
                self.before_tex = None;
            }
            self.tex_display = self.display;
        }
    }

    /// Start a preview when the settings differ from the shown ones.
    fn kick(&mut self) {
        let Some(px) = self.proxy.clone() else { return };
        if self.previewing || self.shown.as_ref().is_some_and(|s| s.params == self.p) {
            return;
        }
        self.previewing = true;
        let (p, scale, tx, gen) = (self.p.clone(), self.scale, self.tx.clone(), self.gen);
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(|| {
                let mut m = Measured::default();
                let prepared = finish::prepare(&px, &p);
                let balanced = finish::balance(&prepared, &p, &mut m);
                let background = finish::background_points(&balanced);
                let stretched = finish::stretch(&balanced, &p, &mut m);
                let curve_hist = finish::curve_input_histograms(&stretched, &p);
                let result = finish::finish_tone(stretched, &p, scale);
                let sp = finish::starting_point(&p);
                let before = if sp == p {
                    result.clone()
                } else {
                    let mut m0 = Measured::default();
                    finish::finish_tone(finish::stretch(&prepared, &sp, &mut m0), &sp, scale)
                };
                Box::new(Preview { params: p, prepared, before, result, measured: m, curve_hist, background })
            })
            .map_err(|_| panic_text());
            let _ = tx.send(Msg::Preview(gen, r));
        });
    }

    fn ask_open(&mut self) {
        if self.dialog.is_none() {
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                let r = dialogs::open_files("Open image", false, Some(("Images", io::image_exts())));
                let _ = tx.send(r.map(Picked::Files).unwrap_or(Picked::NoTool));
            });
            self.dialog = Some((rx, "open"));
        }
    }

    fn ask_save(&mut self) {
        if self.full.is_none() {
            self.status = "Open an image first.".into();
        } else if self.dialog.is_none() {
            self.dialog = Some((dialogs::pick_save("Save result", &self.out), "save"));
        }
    }

    /// Process the full image and save it to `self.out` (in the background).
    fn save(&mut self) {
        let (Some(full), Some(proxy)) = (self.full.clone(), self.proxy.clone()) else {
            self.status = "Open an image first.".into();
            return;
        };
        if self.saving {
            return;
        }
        let out = self.out.trim().to_string();
        if let Err(e) = crate::sys::check_writable(&out) {
            self.status = format!("Not saved: {e}");
            return;
        }
        let need = (full.w * full.h) as f64 * SAVE_BYTES_PER_PX;
        let dir = std::path::Path::new(&out).parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let (mem, _) = crate::sys::free_cached(&dir);
        let avail = mem - crate::sys::memory_reserve();
        if need > avail {
            self.status = format!("Not enough free memory to finish the full image: needs ≈ {}, {} free. Close other programs, or crop the image first.", gb(need), gb(mem.max(0.0)));
            return;
        }
        self.saving = true;
        self.status = format!("Saving {} …", basename(&out));
        let (p, scale, tx) = (self.p.clone(), self.scale, self.tx.clone());
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(|| -> Result<(String, Box<FinishParams>), String> {
                // Measure on the preview copy, as shown, so the saved file looks the same.
                let mut m = Measured::default();
                let _ = finish::process(&proxy, &p, scale, &mut m);
                let img = finish::export_size(&finish::process(&full, &p, 1.0, &mut m), &p);
                io::save_image(&out, &img, p.jpeg_quality)?;
                Ok((format!("Saved {out} ({}×{} px; {}).", img.w, img.h, finish::summary(&p)), Box::new(p)))
            })
            .unwrap_or_else(|_| Err(panic_text()));
            let _ = tx.send(Msg::Saved(r));
        });
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
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.poll(ctx);
        self.shortcuts(ctx);
        self.kick();
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
        ui.heading("Image");
        ui.horizontal_wrapped(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.path_edit).desired_width(ui.available_width().min(260.0)).hint_text("file path, Enter to open"));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let p = self.path_edit.clone();
                self.open(&p);
            }
            if ui.button("Open…").on_hover_text("Open an image: TIFF, PNG, JPEG, FITS, XISF or camera RAW (Ctrl+O)").clicked() {
                self.ask_open();
            }
        });
        ui.checkbox(&mut self.keep, "Keep settings for the next image").on_hover_text("Give several images (e.g. panorama panels) the same look: open the next one without resetting the adjustments");
        if self.full.is_some() {
            ui.horizontal_wrapped(|ui| {
                let mut lin = self.p.stretch != StretchMode::Off;
                if ui.checkbox(&mut lin, "Linear data (needs stretching)").on_hover_text("On for stacks saved as FITS / XISF (dark, unstretched). Off for images that already look right").changed() {
                    self.p.stretch = if lin { StretchMode::MtfAuto } else { StretchMode::Off };
                }
            });
        }
        ui.separator();
        if self.full.is_none() {
            ui.label(egui::RichText::new("The adjustments appear once an image is open.").weak());
            return;
        }
        let d = FinishParams::default();

        section(ui, "Turn and crop", true, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("⟲ 90°").clicked() {
                    self.p.quarter = (self.p.quarter + 3) % 4;
                }
                if ui.button("⟳ 90°").clicked() {
                    self.p.quarter = (self.p.quarter + 1) % 4;
                }
                ui.checkbox(&mut self.p.flip_h, "Mirror");
            });
            egui::Grid::new("fin_geo").num_columns(2).show(ui, |ui| {
                super::stitch::angle(ui, "Rotate (°)", &mut self.p.angle, 45.0, true);
                ui.label("");
                ui.checkbox(&mut self.p.auto_crop, "Cut away the empty corners");
                ui.end_row();
                for (k, l) in ["Crop left (%)", "Crop top (%)", "Crop right (%)", "Crop bottom (%)"].iter().enumerate() {
                    let mut v = self.p.crop[k] * 100.0;
                    ui.label(*l);
                    if ui.add(egui::Slider::new(&mut v, 0.0..=45.0).step_by(0.5)).changed() {
                        self.p.crop[k] = v / 100.0;
                    }
                    ui.end_row();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Shape:");
                for (l, r) in [("3:2", 1.5), ("4:3", 4.0 / 3.0), ("16:9", 16.0 / 9.0), ("4:5", 0.8), ("1:1", 1.0), ("2:1", 2.0)] {
                    if ui.small_button(l).on_hover_text("Crop to this shape, centred (turned to the image's orientation)").clicked() {
                        self.crop_to(r);
                    }
                }
                if ui.small_button("Free").on_hover_text("No crop").clicked() {
                    self.p.crop = [0.0; 4];
                }
            });
            if ui.small_button("Reset turn and crop").clicked() {
                self.p.quarter = 0;
                self.p.flip_h = false;
                self.p.angle = 0.0;
                self.p.crop = d.crop;
                self.p.auto_crop = true;
            }
        });

        if self.p.stretch != StretchMode::Off || self.linear {
            section(ui, "Stretch", true, |ui| self.stretch_ui(ui));
        }

        section(ui, "White balance", true, |ui| {
            ui.horizontal_wrapped(|ui| {
                let t = if self.sampling { "◎ Drag over a grey area…" } else { "◎ Sample grey" };
                if ui.selectable_label(self.sampling, t).on_hover_text("Then drag a rectangle over something that should be neutral grey or white (sky background, a cloud, a wall). The colour multipliers are set from it").clicked() {
                    self.sampling = !self.sampling;
                }
                if self.p.wb != [1.0; 3] {
                    ui.label(egui::RichText::new(format!("× {:.2} {:.2} {:.2}", self.p.wb[0], self.p.wb[1], self.p.wb[2])).weak());
                    if ui.small_button("Clear").clicked() {
                        self.p.wb = [1.0; 3];
                    }
                }
            });
            ui.checkbox(&mut self.p.neutral_bg, "Neutral background").on_hover_text("Make the darker half of the image (usually the sky background) neutral grey");
            temperature_controls(ui, &mut self.p, "fin_wb");
        });

        section(ui, "Light", true, |ui| {
            light_controls(ui, &mut self.p, "fin_tone");
        });

        section(ui, "Curves", !self.simple, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (k, l) in ["RGB", "Red", "Green", "Blue"].iter().enumerate() {
                    let edited = self.p.curves[k] != identity_curve();
                    if ui.selectable_label(self.curve_ch == k, if edited { format!("{l} •") } else { l.to_string() }).clicked() {
                        self.curve_ch = k;
                    }
                }
            });
            let k = self.curve_ch;
            let hist = self.shown.as_ref().map(|s| s.curve_hist[k].clone());
            curve_editor(ui, &mut self.p.curves[k], hist.as_deref(), CURVE_COLOURS[k]);
            ui.label(egui::RichText::new("Click to add a point, drag to move it, right-click to remove it.").weak().size(12.0));
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Reset this curve").clicked() {
                    self.p.curves[k] = identity_curve();
                }
                if ui.small_button("Reset all curves").clicked() {
                    self.p.curves = d.curves.clone();
                }
            });
        });

        section(ui, "Colour", true, |ui| {
            colour_controls(ui, &mut self.p, "fin_col");
        });

        section(ui, "Detail", !self.simple, |ui| {
            egui::Grid::new("fin_det").num_columns(2).show(ui, |ui| {
                slider(ui, "Noise reduction", &mut self.p.noise, 0.0..=1.0, "Smooths grain in flat areas (sky), keeps edges and stars");
                slider(ui, "Colour noise", &mut self.p.color_noise, 0.0..=1.0, "Removes coloured speckles from high-ISO images, keeps the brightness detail");
                slider(ui, "Star reduction", &mut self.p.star_reduce, 0.0..=1.0, "Makes stars smaller so the Milky Way or nebula stands out");
                slider(ui, "Sharpen", &mut self.p.sharpen, 0.0..=2.0, "Unsharp mask; check at 1:1 zoom");
                if !self.simple {
                    ui.label("Sharpen radius (px)");
                    ui.add(egui::Slider::new(&mut self.p.sharpen_radius, 0.5..=5.0).step_by(0.1));
                    ui.end_row();
                }
            });
        });

        ui.add_space(6.0);
        if ui.button("↺ Reset all adjustments").clicked() {
            let keep = (self.p.long_edge, self.p.jpeg_quality);
            self.p = FinishParams::for_input(self.linear);
            (self.p.long_edge, self.p.jpeg_quality) = keep;
        }
    }

    fn stretch_ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("fin_str").num_columns(2).show(ui, |ui| {
            ui.label("Method");
            egui::ComboBox::from_id_salt("fin_stretch").selected_text(self.p.stretch.name()).show_ui(ui, |ui| {
                for m in StretchMode::ALL {
                    ui.selectable_value(&mut self.p.stretch, m, m.name());
                }
            });
            ui.end_row();
            match self.p.stretch {
                StretchMode::MtfAuto => {
                    slider(ui, "Background", &mut self.p.target, 0.05..=0.5, "Brightness of the sky background after stretching");
                    ui.label("");
                    ui.checkbox(&mut self.p.unlinked, "Each colour on its own").on_hover_text("Measures red, green and blue separately: removes a colour cast, but can shift star colours");
                    ui.end_row();
                }
                StretchMode::MtfManual => {
                    log_slider(ui, "Black point", &mut self.p.black, 0.0..=0.5, "Everything darker becomes black");
                    slider(ui, "Midtones", &mut self.p.mid, 0.0001..=0.9999, "Lower = brighter (the value that becomes 50 % grey)");
                    slider(ui, "White point", &mut self.p.white, 0.1..=1.0, "Everything brighter becomes white");
                }
                StretchMode::Asinh => {
                    ui.label("Stretch factor");
                    ui.add(egui::Slider::new(&mut self.p.asinh, 1.0..=10000.0).logarithmic(true)).on_hover_text("1 = almost none, 10000 = very strong. Long deep-sky stacks with a faint background need several thousand");
                    ui.end_row();
                    log_slider(ui, "Black point", &mut self.p.black, 0.0..=0.5, "Subtracted before stretching");
                }
                StretchMode::Ghs => {
                    slider(ui, "Stretch ln(D+1)", &mut self.p.ghs_d, 0.0..=10.0, "How strong the stretch is");
                    slider(ui, "Local intensity b", &mut self.p.ghs_b, -5.0..=15.0, "Concentrates the stretch around the symmetry point: higher = narrower, more contrast there");
                    log_slider(ui, "Symmetry point", &mut self.p.ghs_sp, 0.0..=1.0, "Brightness where the stretch is strongest (put it at the sky background: use the histogram)");
                    if !self.simple {
                        log_slider(ui, "Protect shadows", &mut self.p.ghs_lp, 0.0..=1.0, "Below this the curve is a straight line: keeps dark areas from being stretched");
                        slider(ui, "Protect highlights", &mut self.p.ghs_hp, 0.0..=1.0, "Above this the curve is a straight line: keeps stars from swelling");
                    }
                    log_slider(ui, "Black point", &mut self.p.black, 0.0..=0.5, "Subtracted before stretching");
                }
                _ => {}
            }
        });
        if self.p.stretch.invertible() {
            ui.checkbox(&mut self.p.inverse, "Inverse").on_hover_text("The inverse curve: compresses the bright end instead of lifting the dark end (undoes too strong a stretch)");
        }
        if matches!(self.p.stretch, StretchMode::Asinh | StretchMode::Ghs) {
            ui.checkbox(&mut self.p.keep_colour, "Keep star colours").on_hover_text("Stretch the brightness and keep each pixel's colour. Off: red, green and blue are stretched separately (more saturated sky, paler stars)");
        }
        if self.p.stretch == StretchMode::MtfAuto {
            if let Some(bm) = self.shown.as_ref().and_then(|s| s.measured.stretch) {
                if ui.small_button("Auto → manual").on_hover_text("Continue by hand from the automatic values (the average of the three colours)").clicked() {
                    self.p.stretch = StretchMode::MtfManual;
                    self.p.black = (bm[0].0 + bm[1].0 + bm[2].0) / 3.0;
                    self.p.mid = (bm[0].1 + bm[1].1 + bm[2].1) / 3.0;
                    self.p.white = 1.0;
                }
            }
        }
        if matches!(self.p.stretch, StretchMode::Asinh | StretchMode::Ghs | StretchMode::MtfManual) {
            if let Some((black, sp)) = self.shown.as_ref().map(|s| s.background) {
                let tip = if self.p.stretch == StretchMode::Ghs { "Black point just below the sky background, symmetry point on it" } else { "Black point just below the sky background" };
                if ui.small_button("Points from the background").on_hover_text(tip).clicked() {
                    self.p.black = black;
                    if self.p.stretch == StretchMode::Ghs {
                        self.p.ghs_sp = sp;
                    }
                }
            }
        }
        if self.p.stretch != StretchMode::Off {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.p.bg_extraction, "Background extraction").on_hover_text("Before stretching: removes smooth gradients (light pollution, moon)");
                ui.checkbox(&mut self.p.color_calibration, "Colour calibration").on_hover_text("Before stretching: neutral sky background, average star white");
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
        ui.heading("Histogram");
        match &self.shown {
            Some(s) => super::stitch::histogram(ui, &s.result, None),
            None => {
                ui.label(egui::RichText::new("—").weak());
            }
        }
        ui.separator();
        ui.heading("Save");
        let Some(full) = self.full.clone() else {
            ui.label(egui::RichText::new("Open an image first.").weak());
            return;
        };
        egui::Grid::new("fin_out").num_columns(2).show(ui, |ui| {
            ui.label("Size");
            let (cw, ch) = finish::cropped_size(full.w, full.h, &self.p);
            let long = cw.max(ch);
            let txt = if self.p.long_edge == 0 || self.p.long_edge >= long { "Full size".to_string() } else { format!("{} px long side", self.p.long_edge) };
            egui::ComboBox::from_id_salt("fin_size").selected_text(txt).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.p.long_edge, 0, "Full size");
                for (v, l) in [(3840, "3840 px (4K screen)"), (2560, "2560 px"), (2048, "2048 px (web, forums)"), (1920, "1920 px (Full HD)"), (1600, "1600 px"), (1080, "1080 px (Instagram)")] {
                    if v < long {
                        ui.selectable_value(&mut self.p.long_edge, v, l);
                    }
                }
            });
            ui.end_row();
            ui.label("");
            ui.add(egui::DragValue::new(&mut self.p.long_edge).range(0..=60000).suffix(" px").speed(10)).on_hover_text("Longest side of the saved image; 0 = full size");
            ui.end_row();
            if matches!(io::ext_of(&self.out).as_str(), "jpg" | "jpeg") {
                ui.label("JPEG quality");
                ui.add(egui::Slider::new(&mut self.p.jpeg_quality, 50..=100));
                ui.end_row();
            }
        });
        let (ow, oh) = finish::output_size(full.w, full.h, &self.p);
        ui.label(format!("→ {ow} × {oh} px ({:.1} MP)", (ow * oh) as f64 / 1e6));
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.out).desired_width(ui.available_width().min(240.0)).hint_text("output file"));
            let cur = io::ext_of(&self.out);
            egui::ComboBox::from_id_salt("fin_fmt").selected_text(if cur.is_empty() { "type".to_string() } else { cur.clone() }).show_ui(ui, |ui| {
                for (e, l) in io::OUTPUT_FORMATS {
                    if ui.selectable_label(cur == e || (cur == "tiff" && e == "tif") || (cur == "jpeg" && e == "jpg"), l).clicked() {
                        self.out = io::with_ext(&self.out, e);
                    }
                }
            });
        });
        if io::is_astro(&self.out) {
            ui.label(egui::RichText::new("FITS / XISF get the finished (stretched) image here, as 32-bit float.").weak().size(12.0));
        }
        if std::path::Path::new(&self.out).exists() {
            ui.colored_label(Color32::from_rgb(230, 150, 30), "This file exists and will be replaced.");
        }
        ui.horizontal_wrapped(|ui| {
            let save = ui.add_enabled(!self.saving, egui::Button::new(egui::RichText::new("⬇ Save").strong())).on_hover_text("Process the full image and save it (Ctrl+S)");
            if save.clicked() {
                self.save();
            }
            if ui.add_enabled(!self.saving, egui::Button::new("Save as…")).on_hover_text("Choose the file (Ctrl+Shift+S)").clicked() {
                self.ask_save();
            }
        });
        let need = (full.w * full.h) as f64 * SAVE_BYTES_PER_PX;
        let dir = std::path::Path::new(&self.out).parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let (mem, _) = crate::sys::free_cached(&dir);
        let txt = format!("Saving needs ≈ {} memory ({} free).", gb(need), gb(mem.max(0.0)));
        if need > mem - crate::sys::memory_reserve() {
            ui.colored_label(Color32::from_rgb(230, 150, 30), format!("⚠ {txt} Close other programs or crop first."));
        } else {
            ui.label(egui::RichText::new(txt).weak().size(12.0));
        }
        ui.separator();
        ui.label(egui::RichText::new(format!("In use: {}", finish::summary(&self.p))).weak().size(12.0));
    }

    // ----------------------------------------------------------------------
    // Centre: the preview
    // ----------------------------------------------------------------------

    fn center(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut compare = false;
        ui.horizontal_wrapped(|ui| {
            if ui.button("Fit").on_hover_text("Whole image in view (F)").clicked() {
                self.view.fit = true;
            }
            if ui.button("1:1").on_hover_text("One preview pixel per screen pixel").clicked() {
                self.view.zoom = 1.0;
            }
            let c = ui.button("◐ Compare").on_hover_text("Hold to see the image before white balance, light, curves, colour and detail (or hold B)");
            compare = c.is_pointer_button_down_on() || (!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(egui::Key::B)));
            ui.separator();
            ui.label("Display");
            egui::ComboBox::from_id_salt("fin_disp").selected_text(self.display.name()).show_ui(ui, |ui| {
                for m in Display::ALL {
                    ui.selectable_value(&mut self.display, m, m.name());
                }
            }).response.on_hover_text("How the preview is shown. Only the view changes, never the saved image (as Siril's display modes)");
            if self.display != Display::Linear {
                ui.colored_label(Color32::from_rgb(230, 150, 30), "view only");
            }
            if self.busy() {
                ui.spinner();
            }
        });
        ui.label(egui::RichText::new(&self.status).size(12.5));
        if self.scale < 1.0 && self.shown.is_some() {
            ui.label(egui::RichText::new(format!("Preview at {:.0} % of full size; the full image is processed when saving.", self.scale * 100.0)).weak().size(12.0));
        }
        let Some(s) = &self.shown else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(if self.loading { "Loading…" } else { "No image yet" }).size(20.0).weak());
                ui.label(egui::RichText::new("Open an image (Ctrl+O), or use “Use in Post-processing” in the Star stacking tab or “Post-process” after saving a panorama.").weak());
            });
            return;
        };
        let (w, h) = (s.result.w as f64, s.result.h as f64);
        if compare && self.before_tex.is_none() {
            self.before_tex = Some(texture(ctx, "finish_before", &finish::display(&s.before, self.display)));
        }
        let tex = if compare { self.before_tex.as_ref() } else { self.tex.as_ref() };
        let Some(tex) = tex else { return };
        let shown = image_view(ui, &mut self.view, (w, h), &[(tex, [0.5, w + 0.5, 0.5, h + 0.5])], !self.sampling);
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
                            self.status = "White balance set from the sampled area.".into();
                        }
                        None => self.status = "The sampled area is black or outside the image: try another one.".into(),
                    }
                }
            }
        }
    }
}

/// The light sliders (exposure, contrast, highlights, midtones, shadows, whites,
/// blacks) and their reset; shared by the Post-processing and Timelapse tabs.
pub(crate) fn light_controls(ui: &mut egui::Ui, p: &mut FinishParams, id: &str) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        ui.label("Exposure (EV)");
        ui.add(egui::Slider::new(&mut p.exposure, -3.0..=3.0).step_by(0.05));
        ui.end_row();
        slider(ui, "Contrast", &mut p.contrast, -1.0..=1.0, "S-curve around the middle grey");
        slider(ui, "Highlights", &mut p.highlights, -1.0..=1.0, "Bright areas (around 75 %): lower to bring back detail in bright sky or lights");
        slider(ui, "Midtones", &mut p.midtones, -1.0..=1.0, "Middle brightness (around 50 %)");
        slider(ui, "Shadows", &mut p.shadows, -1.0..=1.0, "Dark areas (around 25 %): raise to see into the foreground");
        slider(ui, "Whites", &mut p.whites, -1.0..=1.0, "White point: raise to make the brightest parts white, lower to keep them from clipping");
        slider(ui, "Blacks", &mut p.blacks, -1.0..=1.0, "Black point: lower for deeper blacks, raise to lift them");
    });
    if ui.small_button("Reset light").clicked() {
        (p.exposure, p.contrast, p.highlights, p.midtones, p.shadows, p.whites, p.blacks) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
}

/// Temperature and tint sliders.
pub(crate) fn temperature_controls(ui: &mut egui::Ui, p: &mut FinishParams, id: &str) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        slider(ui, "Temperature", &mut p.temperature, -1.0..=1.0, "Cooler (blue) ← → warmer (yellow)");
        slider(ui, "Tint", &mut p.tint, -1.0..=1.0, "Greener ← → more magenta");
    });
}

/// Saturation and vibrance sliders.
pub(crate) fn colour_controls(ui: &mut egui::Ui, p: &mut FinishParams, id: &str) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        slider(ui, "Saturation", &mut p.saturation, 0.0..=2.0, "All colours: 0 = black and white, 1 = unchanged");
        slider(ui, "Vibrance", &mut p.vibrance, -1.0..=1.0, "Dull colours more than strong ones (skin, already strong colours stay natural)");
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
        self.shown.as_ref().is_some_and(|s| s.params == self.p)
    }
    pub fn t_set_out(&mut self, p: &str) {
        self.out = p.to_string();
    }
    pub fn t_save(&mut self) {
        self.save();
    }
}

//! The sky / foreground mask with its two looks, painted with a brush on the
//! shown image or detected automatically. Shared by the Post-processing and
//! Timelapse tabs.
//!
//! The mask lies on the source image (before turning and cropping); painting
//! and the overlay go through `finish::source_point`, so the mask stays on the
//! same part of the picture when the image is turned or cropped.

use crate::gui::i18n::tr;
use super::Shown;
use crate::finish::{self, FinishParams, Local};
use crate::image::{Bits, Img};
use egui::{Color32, Stroke};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// The overlay is drawn at most this wide.
const OVERLAY_W: usize = 800;

/// What the overlay was made for: mask revision, turn and crop, sizes.
type OverlayKey = (u64, [u64; 8], (usize, usize), (usize, usize));

pub struct MaskEditor {
    pub local: Local,
    /// Painting with the mouse (the view does not pan with the left button then).
    pub painting: bool,
    /// Show the mask over the image (it is always shown while painting).
    pub show: bool,
    /// Brush radius, % of the shown image's width.
    pub brush: f64,
    rev: u64,
    overlay: Option<(OverlayKey, egui::TextureHandle)>,
    detecting: Option<Receiver<Bits>>,
    /// A message for the tab's status line (taken by the tab).
    pub message: Option<String>,
    /// A fisheye's image circle (centre x, y and radius, fractions of the width) and
    /// the sky's share of its radius, in percent.
    circle: Option<(f64, f64, f64)>,
    sky_radius: f64,
}

impl MaskEditor {
    pub fn new(show: bool) -> Self {
        Self { local: Local::default(), painting: false, show, brush: 4.0, rev: 0, overlay: None, detecting: None, message: None, circle: None, sky_radius: 85.0 }
    }

    /// Make the mask fit an image of `w × h` (a new, empty one if the shape changed).
    pub fn fit(&mut self, w: usize, h: usize) {
        if !self.local.fits(w, h) {
            self.local.mask = Local::empty_mask(w, h);
            self.rev += 1;
        }
    }

    /// Replace the mask and looks (undo, presets).
    pub fn set_local(&mut self, l: Local) {
        if l.mask != self.local.mask {
            self.circle = None;
        }
        self.local = l;
        self.rev += 1;
    }

    /// The fisheye circle the sky disc came from, and the sky's share of its radius (%).
    pub fn circle_state(&self) -> (Option<(f64, f64, f64)>, f64) {
        (self.circle, self.sky_radius)
    }

    /// Restore [`MaskEditor::circle_state`] (projects), after [`MaskEditor::set_local`].
    pub fn set_circle(&mut self, circle: Option<(f64, f64, f64)>, sky_radius: f64) {
        self.circle = circle;
        self.sky_radius = sky_radius;
    }

    pub fn has_sky(&self) -> bool {
        self.local.mask.any()
    }

    pub fn detecting(&self) -> bool {
        self.detecting.is_some()
    }

    fn set_mask(&mut self, m: Bits) {
        self.local.mask = m;
        self.rev += 1;
    }

    fn fill(&mut self, sky: bool) {
        self.circle = None;
        let (h, w) = (self.local.mask.h, self.local.mask.w);
        self.set_mask(Bits::from_fn(h, w, |_, _| sky));
    }

    /// Detect the sky in `img` (the source image, any size) in the background.
    pub fn detect(&mut self, img: Arc<Img>) {
        if self.detecting.is_some() {
            return;
        }
        self.fit(img.w, img.h);
        let (rows, cols) = (self.local.mask.h, self.local.mask.w);
        let (tx, rx) = channel();
        crate::sys::background(move || {
            let r = std::panic::catch_unwind(|| crate::stacker::detect_sky(&img, cols, Some(rows), 0.01).0);
            let _ = tx.send(r.unwrap_or_else(|_| Bits::new(rows, cols)));
        });
        self.detecting = Some(rx);
    }

    /// Sky = a disc inside a fisheye's image circle (found in `img`, the source image).
    pub fn fisheye(&mut self, img: &Img) {
        self.fit(img.w, img.h);
        match crate::finish::fisheye_circle(img) {
            Some(c) => {
                self.circle = Some(c);
                self.apply_circle(img.w, img.h);
                self.message = Some(crate::trf!("Image circle found. Sky = the inner {} % of its radius; the ring outside (horizon, trees) is foreground. Adjust “Sky circle”, raise “Soft edge” for a gradual change, and correct the rest with the brush.", format!("{:.0}", self.sky_radius)));
            }
            None => self.message = Some(tr("No dark corners around a lit circle: this is not a circular fisheye image. Paint the sky with the brush instead.").into()),
        }
    }

    fn apply_circle(&mut self, w: usize, h: usize) {
        if let Some((cx, cy, r)) = self.circle {
            let (rows, cols) = (self.local.mask.h, self.local.mask.w);
            self.set_mask(crate::finish::circle_mask(rows, cols, w, h, cx, cy, r * self.sky_radius / 100.0));
        }
    }

    /// Take a finished detection.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.detecting {
            if let Ok(m) = rx.try_recv() {
                self.detecting = None;
                if (m.h, m.w) == (self.local.mask.h, self.local.mask.w) {
                    let pct = 100 * m.count() / m.len().max(1);
                    self.circle = None;
                    self.set_mask(m);
                    self.message = Some(crate::trf!("Sky detected: {} % of the image. It is approximate: correct it with the brush (✎ Paint: left button = sky, right button = foreground).", pct));
                }
            }
        }
    }

    /// The mask controls. `source` is the image the sky is detected in (None: not open yet).
    pub fn controls(&mut self, ui: &mut egui::Ui, source: Option<Arc<Img>>) {
        ui.horizontal_wrapped(|ui| {
            ui.toggle_value(&mut self.painting, tr("✎ Paint")).on_hover_text(tr("Paint on the image: left button = sky, right button = foreground, mouse wheel = brush size"));
            ui.checkbox(&mut self.show, tr("Show mask")).on_hover_text(tr("Sky tinted blue, foreground orange (always shown while painting)"));
        });
        egui::Grid::new(ui.scope_id().with("mask_grid")).num_columns(2).show(ui, |ui| {
            ui.label(tr("Brush size (% width)"));
            ui.add(egui::Slider::new(&mut self.brush, 0.5..=30.0).logarithmic(true));
            ui.end_row();
            ui.label(tr("Soft edge (% width)"));
            let mut f = self.local.feather * 100.0;
            if ui.add(egui::Slider::new(&mut f, 0.0..=25.0).step_by(0.5)).on_hover_text(tr("How softly the sky and foreground looks blend at the mask's edge. Large values give a gradual change (with the fisheye circle: a radial gradient)")).changed() {
                self.local.feather = f / 100.0;
            }
            ui.end_row();
            if self.circle.is_some() {
                ui.label(tr("Sky circle (% radius)"));
                if ui.add(egui::Slider::new(&mut self.sky_radius, 30.0..=100.0).step_by(1.0)).on_hover_text(tr("How much of the fisheye's image circle is sky; the ring outside it is foreground. Changing it redraws the mask (brush corrections are lost)")).changed() {
                    if let Some(img) = &source {
                        self.apply_circle(img.w, img.h);
                    }
                }
                ui.end_row();
            }
        });
        let have = self.local.mask.w > 0;
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(have, egui::Button::new(tr("All sky"))).clicked() {
                self.fill(true);
            }
            if ui.add_enabled(have, egui::Button::new(tr("Invert"))).clicked() {
                let old = self.local.mask.clone();
                self.circle = None;
                self.set_mask(Bits::from_fn(old.h, old.w, |y, x| !old.at(y, x)));
            }
            if ui.add_enabled(have, egui::Button::new(tr("Clear"))).on_hover_text(tr("Everything foreground")).clicked() {
                self.fill(false);
            }
            if ui.add_enabled(source.is_some(), egui::Button::new(tr("◯ Fisheye circle"))).on_hover_text(tr("For a circular fisheye pointing up (all-sky): sky = a disc inside the image circle, the ring outside it foreground")).clicked() {
                if let Some(img) = &source {
                    self.fisheye(img);
                }
            }
            let t = if self.detecting.is_some() { tr("Detecting…") } else { tr("◎ Detect sky") };
            if ui.add_enabled(source.is_some() && self.detecting.is_none(), egui::Button::new(t)).on_hover_text(tr("Find the sky automatically (sky at the top), then correct it with the brush")).clicked() {
                if let Some(img) = source {
                    self.detect(img);
                }
            }
        });
        if self.local.mask.w > 0 {
            let pct = 100.0 * self.local.mask.count() as f64 / self.local.mask.len().max(1) as f64;
            ui.label(egui::RichText::new(crate::trf!("Sky: {} % of the image.", format!("{pct:.0}"))).weak().size(12.0));
        }
    }

    /// While painting, the mouse wheel over the view sets the brush size instead of
    /// zooming. Call before drawing the view.
    pub fn wheel(&mut self, ui: &mut egui::Ui) {
        if self.painting && ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                self.brush = (self.brush * 1.15f64.powf((dy / 50.0) as f64)).clamp(0.5, 30.0);
                ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
    }

    /// The overlay for a shown image of `out` (w, h) showing `geometry(source, geo)`,
    /// the source being `src` (w, h). None when the mask is not shown.
    pub fn overlay(&mut self, ctx: &egui::Context, src: (usize, usize), geo: &FinishParams, out: (usize, usize)) -> Option<egui::TextureHandle> {
        if !(self.show || self.painting) || self.local.mask.is_empty() {
            return None;
        }
        let g = [geo.quarter as u64, geo.flip_h as u64, geo.angle.to_bits(), geo.auto_crop as u64, geo.crop[0].to_bits(), geo.crop[1].to_bits(), geo.crop[2].to_bits(), geo.crop[3].to_bits()];
        let key = (self.rev, g, src, out);
        if self.overlay.as_ref().is_none_or(|(k, _)| *k != key) {
            let ow = out.0.clamp(1, OVERLAY_W);
            let oh = ((out.1 as f64 * ow as f64 / out.0.max(1) as f64).round() as usize).max(1);
            let (gw, gh) = finish::geometry_size(src.0, src.1, geo);
            let m = &self.local.mask;
            let mut rgba = vec![0u8; ow * oh * 4];
            for y in 0..oh {
                for x in 0..ow {
                    let (sx, sy) = finish::source_point(src.0, src.1, geo, (x as f64 + 0.5) * gw as f64 / ow as f64, (y as f64 + 0.5) * gh as f64 / oh as f64);
                    let mx = ((sx / src.0 as f64 * m.w as f64) as usize).min(m.w - 1);
                    let my = ((sy / src.1 as f64 * m.h as f64) as usize).min(m.h - 1);
                    let c: [u8; 4] = if m.at(my, mx) { [60, 120, 255, 70] } else { [255, 150, 40, 45] };
                    rgba[(y * ow + x) * 4..][..4].copy_from_slice(&c);
                }
            }
            let t = ctx.load_texture("mask_overlay", egui::ColorImage::from_rgba_unmultiplied([ow, oh], &rgba), egui::TextureOptions::NEAREST);
            self.overlay = Some((key, t));
        }
        self.overlay.as_ref().map(|(_, t)| t.clone())
    }

    /// Brush outline and painting on the view `s` (see `overlay` for the sizes).
    pub fn input(&mut self, ui: &egui::Ui, s: &Shown, src: (usize, usize), geo: &FinishParams, out: (usize, usize)) {
        if !self.painting || self.local.mask.is_empty() {
            return;
        }
        let Some(pos) = s.resp.hover_pos() else { return };
        ui.painter().with_clip_rect(s.rect).circle_stroke(pos, (self.brush / 100.0 * out.0 as f64) as f32 * s.zoom, Stroke::new(1.5, Color32::LIGHT_BLUE));
        let (pd, sd) = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down()));
        if (pd || sd) && s.resp.is_pointer_button_down_on() {
            let (x, y) = s.to_image(pos);
            self.paint(x - 0.5, y - 0.5, src, geo, out, pd);
        }
    }

    /// Paint a round brush stroke at (x, y) of the shown image (continuous pixel
    /// coordinates of an `out`-sized image): sky if `sky`, else foreground.
    pub fn paint(&mut self, x: f64, y: f64, src: (usize, usize), geo: &FinishParams, out: (usize, usize), sky: bool) {
        let (gw, gh) = finish::geometry_size(src.0, src.1, geo);
        let (sx, sy) = finish::source_point(src.0, src.1, geo, x * gw as f64 / out.0.max(1) as f64, y * gh as f64 / out.1.max(1) as f64);
        let m = &mut self.local.mask;
        let (mh, mw) = (m.h, m.w);
        let k = mw as f64 / src.0.max(1) as f64;
        let (cx, cy) = (sx * k, sy * mh as f64 / src.1.max(1) as f64);
        let rr = self.brush / 100.0 * gw as f64 * k;
        let mut changed = false;
        for r in ((cy - rr).floor().max(0.0) as usize)..((cy + rr).ceil().max(0.0) as usize).min(mh) {
            for c in ((cx - rr).floor().max(0.0) as usize)..((cx + rr).ceil().max(0.0) as usize).min(mw) {
                if (c as f64 + 0.5 - cx).powi(2) + (r as f64 + 0.5 - cy).powi(2) <= rr * rr && m.at(r, c) != sky {
                    m.set(r, c, sky);
                    changed = true;
                }
            }
        }
        if changed {
            self.rev += 1;
        }
    }
}

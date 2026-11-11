//! The window (egui): the "Panorama stitcher" and "Star stacking" tabs, a
//! zoomable image view, and native file dialogs.

pub mod dialogs;
pub mod notes;
pub mod stack;
pub mod stitch;

use crate::image::Img;
use egui::{Color32, Pos2, Rect, Sense, Vec2};

/// Upload an image as a texture (8-bit, clamped).
pub fn texture(ctx: &egui::Context, name: &str, img: &Img) -> egui::TextureHandle {
    let bytes = crate::io::to_rgb8(img);
    ctx.load_texture(name, egui::ColorImage::from_rgb([img.w, img.h], &bytes), egui::TextureOptions::LINEAR)
}

/// Zoom and pan of an image view. Image coordinates are 1-based pixels of the
/// "full" image; `size` is that full size (w, h).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub zoom: f32,       // screen px per image px
    pub center: Vec2,    // image coords at the view centre
    pub fit: bool,
}

impl Default for View {
    fn default() -> Self {
        Self { zoom: 1.0, center: Vec2::ZERO, fit: true }
    }
}

pub struct Shown {
    pub rect: Rect,
    pub resp: egui::Response,
    pub zoom: f32,
    pub center: Vec2,
}

impl Shown {
    /// Image (1-based, continuous) → screen.
    pub fn to_screen(&self, x: f64, y: f64) -> Pos2 {
        let c = self.rect.center();
        Pos2::new(c.x + (x as f32 - self.center.x) * self.zoom, c.y + (y as f32 - self.center.y) * self.zoom)
    }
    pub fn to_image(&self, p: Pos2) -> (f64, f64) {
        let c = self.rect.center();
        (((p.x - c.x) / self.zoom + self.center.x) as f64, ((p.y - c.y) / self.zoom + self.center.y) as f64)
    }
    /// The visible part in image coordinates (x0, x1, y0, y1).
    pub fn visible(&self) -> (f64, f64, f64, f64) {
        let (a, b) = (self.to_image(self.rect.min), self.to_image(self.rect.max));
        (a.0, b.0, a.1, b.1)
    }
}

/// Draw `tex` covering the image box `[x0, x1] × [y0, y1]` (image coordinates)
/// inside a view of an image of `size`. Left-drag pans (unless `pan` is false), wheel zooms.
pub fn image_view(ui: &mut egui::Ui, view: &mut View, size: (f64, f64), layers: &[(&egui::TextureHandle, [f64; 4])], pan: bool) -> Shown {
    let avail = ui.available_size().max(Vec2::splat(50.0));
    let (resp, painter) = ui.allocate_painter(avail, Sense::click_and_drag());
    let rect = resp.rect;
    painter.rect_filled(rect, 0.0, Color32::from_gray(32));
    let (w, h) = (size.0.max(1.0) as f32, size.1.max(1.0) as f32);
    if view.fit {
        view.zoom = (rect.width() / w).min(rect.height() / h);
        view.center = Vec2::new(w / 2.0 + 0.5, h / 2.0 + 0.5);
        view.fit = false;
    }
    if resp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            if let Some(p) = resp.hover_pos() {
                let k = (dy / 200.0).exp();
                let before = Vec2::new((p.x - rect.center().x) / view.zoom, (p.y - rect.center().y) / view.zoom) + view.center;
                view.zoom = (view.zoom * k).clamp(0.005, 64.0);
                let after = Vec2::new((p.x - rect.center().x) / view.zoom, (p.y - rect.center().y) / view.zoom) + view.center;
                view.center += before - after;
            }
        }
    }
    if pan && resp.dragged_by(egui::PointerButton::Primary) && !ui.input(|i| i.modifiers.ctrl) {
        view.center -= resp.drag_delta() / view.zoom;
    }
    if resp.dragged_by(egui::PointerButton::Middle) {
        view.center -= resp.drag_delta() / view.zoom;
    }
    let shown = Shown { rect, resp, zoom: view.zoom, center: view.center };
    let p = painter.with_clip_rect(rect);
    for (tex, b) in layers {
        let r = Rect::from_min_max(shown.to_screen(b[0], b[2]), shown.to_screen(b[1], b[3]));
        p.image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    shown
}

pub fn zoom_by(view: &mut View, k: f32) {
    view.zoom = (view.zoom * k).clamp(0.005, 64.0);
}

/// A slider with a label (row of a grid).
pub fn slider<N: egui::emath::Numeric>(ui: &mut egui::Ui, label: &str, v: &mut N, r: std::ops::RangeInclusive<N>, step: f64) -> bool {
    ui.label(label);
    let c = ui.add(egui::Slider::new(v, r).step_by(step)).changed();
    ui.end_row();
    c
}

/// A combo box over named values (row of a grid).
pub fn combo<T: Copy + PartialEq>(ui: &mut egui::Ui, label: &str, v: &mut T, all: &[T], name: impl Fn(T) -> &'static str) -> bool {
    ui.label(label);
    let before = *v;
    egui::ComboBox::from_id_salt(label).selected_text(name(*v)).show_ui(ui, |ui| {
        for &o in all {
            ui.selectable_value(v, o, name(o));
        }
    });
    ui.end_row();
    *v != before
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Stitch,
    Stack,
}

/// Appearance: light or dark colours, classic or modern look (rounder, roomier),
/// full or simple layout (advanced controls left out, for beginners).
#[derive(Clone, Copy, PartialEq)]
pub struct Prefs {
    pub dark: bool,
    pub modern: bool,
    pub simple: bool,
}

pub fn apply_style(ctx: &egui::Context, p: Prefs) {
    ctx.set_visuals(if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
    ctx.all_styles_mut(|s| {
        let r = if p.modern { 8 } else { 2 };
        for w in [&mut s.visuals.widgets.noninteractive, &mut s.visuals.widgets.inactive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active, &mut s.visuals.widgets.open] {
            w.corner_radius = egui::CornerRadius::same(r);
        }
        s.visuals.window_corner_radius = egui::CornerRadius::same(if p.modern { 10 } else { 6 });
        if p.modern {
            s.spacing.item_spacing = egui::vec2(8.0, 6.0);
            s.spacing.button_padding = egui::vec2(9.0, 4.0);
            s.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
            s.visuals.selection.bg_fill = egui::Color32::from_rgb(51, 120, 217);
        } else {
            s.spacing.item_spacing = egui::vec2(8.0, 3.0);
            s.spacing.button_padding = egui::vec2(4.0, 1.0);
        }
    });
}

/// The DejaVu fonts shipped with the program (assets/fonts, license included) as
/// fallbacks: egui's own fonts lack the symbols on the buttons (▶ ◐ ⬇ ← ↶ ☾ …).
pub fn install_fonts(ctx: &egui::Context) {
    use std::sync::Arc;
    let mut f = egui::FontDefinitions::default();
    f.font_data.insert("dejavu".into(), Arc::new(egui::FontData::from_static(include_bytes!("../../assets/fonts/DejaVuSans.ttf"))));
    f.font_data.insert("dejavu-mono".into(), Arc::new(egui::FontData::from_static(include_bytes!("../../assets/fonts/DejaVuSansMono.ttf"))));
    if let Some(v) = f.families.get_mut(&egui::FontFamily::Proportional) {
        v.push("dejavu".into());
    }
    if let Some(v) = f.families.get_mut(&egui::FontFamily::Monospace) {
        v.push("dejavu-mono".into());
        v.push("dejavu".into());
    }
    ctx.set_fonts(f);
}

/// Path of `p` relative to the folder `dir` (for project files).
pub fn relpath(p: &std::path::Path, dir: &std::path::Path) -> String {
    let a: Vec<_> = p.components().collect();
    let b: Vec<_> = dir.components().collect();
    let k = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut out = std::path::PathBuf::new();
    for _ in k..b.len() {
        out.push("..");
    }
    for c in &a[k..] {
        out.push(c);
    }
    out.to_string_lossy().to_string()
}

pub struct App {
    tab: Tab,
    prefs: Prefs,
    pub stitch: stitch::StitchTab,
    pub stack: stack::StackTab,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, paths: Vec<String>) -> Self {
        Self::with_context(&cc.egui_ctx, paths)
    }

    /// The app on an egui context (also used by the headless tests).
    pub fn with_context(ctx: &egui::Context, paths: Vec<String>) -> Self {
        install_fonts(ctx);
        let prefs = load_prefs();
        apply_style(ctx, prefs);
        let mut app = Self { tab: Tab::Stitch, prefs, stitch: stitch::StitchTab::default(), stack: stack::StackTab::default() };
        // Project files open in their tab; images go to the stitcher.
        let (stackp, rest): (Vec<String>, Vec<String>) = paths.into_iter().partition(|p| p.to_lowercase().ends_with(stack::PROJECT_EXT));
        let (panop, imgs): (Vec<String>, Vec<String>) = rest.into_iter().partition(|p| p.to_lowercase().ends_with(stitch::PROJECT_EXT));
        if let Some(p) = panop.first() {
            app.stitch.open_project(p);
        } else if !imgs.is_empty() {
            app.stitch.add_paths(imgs);
        }
        if let Some(p) = stackp.first() {
            app.stack.open_project(p);
            app.tab = Tab::Stack;
        }
        app
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

impl App {
    pub fn busy(&self) -> bool {
        self.stitch.busy() || self.stack.busy()
    }
    pub fn show_stack(&mut self, on: bool) {
        self.tab = if on { Tab::Stack } else { Tab::Stitch };
    }
    pub fn set_simple(&mut self, ctx: &egui::Context, simple: bool, modern: bool) {
        self.prefs.simple = simple;
        self.prefs.modern = modern;
        apply_style(ctx, self.prefs);
    }

    /// One frame of the window.
    pub fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Stitch, egui::RichText::new("Panorama stitcher").strong());
                ui.selectable_value(&mut self.tab, Tab::Stack, egui::RichText::new("Star stacking (landscape / trails)").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let before = self.prefs;
                    ui.checkbox(&mut self.prefs.simple, "✓ Simple").on_hover_text("Leave out the advanced controls");
                    ui.checkbox(&mut self.prefs.modern, "◆ Modern");
                    ui.checkbox(&mut self.prefs.dark, "☾ Dark");
                    if self.prefs != before {
                        apply_style(&ctx, self.prefs);
                        save_prefs(self.prefs);
                    }
                });
            });
        });
        // Stacked panels handed to the stitcher.
        let handed = std::mem::take(&mut self.stack.to_stitcher);
        if !handed.is_empty() {
            self.stitch.add_paths(handed);
            self.tab = Tab::Stitch;
        }
        self.stitch.simple = self.prefs.simple;
        self.stack.simple = self.prefs.simple;
        match self.tab {
            Tab::Stitch => self.stitch.ui(ui, &ctx),
            Tab::Stack => self.stack.ui(ui, &ctx),
        }
        if self.stitch.busy() || self.stack.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// Folder for the program's settings.
pub fn config_dir() -> std::path::PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("panostitch")
}
fn load_prefs() -> Prefs {
    let t: toml::Table = std::fs::read_to_string(config_dir().join("ui.toml")).ok().and_then(|s| s.parse().ok()).unwrap_or_default();
    let b = |k: &str| t.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    Prefs { dark: b("dark"), modern: b("modern"), simple: b("simple") }
}
fn save_prefs(p: Prefs) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("ui.toml"), format!("dark = {}\nmodern = {}\nsimple = {}\n", p.dark, p.modern, p.simple));
}

/// Open the window with `paths` loaded into the stitcher.
pub fn run(paths: Vec<String>) -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1700.0, 1000.0]).with_title("PanoStitch"),
        ..Default::default()
    };
    eframe::run_native("PanoStitch", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, paths)))))
}

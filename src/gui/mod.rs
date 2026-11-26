//! The window (egui): the "Panorama stitcher" and "Star stacking" tabs, a
//! zoomable image view, and native file dialogs.

pub mod dialogs;
pub mod finish;
pub mod help;
pub mod i18n;
mod i18n_es;
mod i18n_fr;
mod i18n_pl;
mod i18n_table;
pub mod mask;
pub mod notes;
pub mod stack;
pub mod stitch;
pub mod timelapse;

use i18n::tr;
use crate::image::Img;
use egui::{Color32, Pos2, Rect, Sense, Vec2};

/// Upload an image as a texture (8-bit, clamped).
/// A texture of `img`, reduced if it is larger than the graphics card allows
/// (often 4096 or 8192 px on older or built-in graphics). It is drawn into the
/// same rectangle either way, so only the on-screen sharpness changes.
pub fn texture(ctx: &egui::Context, name: &str, img: &Img) -> egui::TextureHandle {
    let max = ctx.input(|i| i.max_texture_side).clamp(1024, 16384);
    let small;
    let img = if img.w.max(img.h) > max {
        small = crate::image::resize(img, (max as f64 - 1.0) / img.w.max(img.h) as f64);
        &small
    } else {
        img
    };
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

/// The display mode of a preview with Low / High cut-offs, as Siril's (only the
/// view changes). `processed`: offer "As processed" first, which shows the
/// preview with the tab's own stretch (`*processed == true`). Returns true when
/// something changed.
pub fn view_stretch_ui(ui: &mut egui::Ui, id: &str, v: &mut crate::finish::ViewStretch, mut processed: Option<&mut bool>) -> bool {
    use crate::finish::Display;
    let before = (*v, processed.as_deref().copied());
    let as_processed = processed.as_deref().copied().unwrap_or(false);
    let offers_processed = processed.is_some();
    let name = |m: Display| if offers_processed && m == Display::Linear { tr("Linear") } else { tr(m.name()) };
    // Compound widgets (a slider with its value, the combo box) are laid out in
    // a row of their own, so a wrapping toolbar does not move them to the next
    // line: they would run past the panel's edge, hidden under the side panel.
    // Start a new line when the rest of this one is too short.
    let room = |ui: &mut egui::Ui, w: f32| {
        if ui.available_size_before_wrap().x < w {
            ui.end_row();
        }
    };
    let text_w = |ui: &egui::Ui, t: &str| egui::WidgetText::from(t).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button).size().x;
    let combo_w = ui.spacing().combo_width.max(text_w(ui, if as_processed { tr("As processed") } else { name(v.mode) }) + 30.0);
    room(ui, text_w(ui, tr("Display")) + combo_w + 2.0 * ui.spacing().item_spacing.x);
    ui.label(tr("Display"));
    egui::ComboBox::from_id_salt(id).selected_text(if as_processed { tr("As processed") } else { name(v.mode) }).show_ui(ui, |ui| {
        if let Some(p) = processed.as_deref_mut() {
            if ui.selectable_label(*p, tr("As processed")).on_hover_text(tr("The preview with the stretch set in the panel")).clicked() {
                *p = true;
            }
        }
        for m in Display::ALL {
            if ui.selectable_label(!as_processed && v.mode == m, name(m)).clicked() {
                v.mode = m;
                if let Some(p) = processed.as_deref_mut() {
                    *p = false;
                }
            }
        }
    }).response.on_hover_text(tr("How the preview is shown, as Siril's display modes: only the view changes, never the saved image. Low and High set the range of values shown."));
    let as_processed = processed.as_deref().copied().unwrap_or(false);
    if !as_processed && v.has_range() {
        ui.spacing_mut().slider_width = 90.0;
        let tip = tr("Low: values below it are shown black. High: values above it are shown white. For faint linear data try High around 0.01–0.1.");
        // slider + its value (up to "0.00001") + its name
        let slider_w = |ui: &egui::Ui, t: &str| 90.0 + text_w(ui, "0.000001") + text_w(ui, t) + 4.0 * ui.spacing().item_spacing.x;
        room(ui, slider_w(ui, tr("Low")));
        ui.add(egui::Slider::new(&mut v.low, 0.0..=1.0).logarithmic(true).smallest_positive(1e-5).max_decimals(5).text(tr("Low"))).on_hover_text(tip);
        room(ui, slider_w(ui, tr("High")) + 30.0);
        ui.add(egui::Slider::new(&mut v.high, 0.0..=1.0).logarithmic(true).smallest_positive(1e-4).max_decimals(5).text(tr("High"))).on_hover_text(tip);
        if v.low >= v.high {
            v.low = (v.high * 0.5).max(0.0);
        }
        if (v.low != 0.0 || v.high != 1.0) && ui.small_button("↺").on_hover_text(tr("Low 0, High 1")).clicked() {
            (v.low, v.high) = (0.0, 1.0);
        }
    }
    if !as_processed && !v.plain() {
        room(ui, text_w(ui, tr("view only")) + ui.spacing().item_spacing.x);
        ui.colored_label(Color32::from_rgb(230, 150, 30), tr("view only"));
    }
    (*v, processed.as_deref().copied()) != before
}

/// What saving linear or stretched means (the same words in every tab).
pub fn save_kind_tip(linear: bool) -> &'static str {
    if linear {
        tr("Linear: the light as measured, for further processing (Post-processing tab, Siril, PixInsight). It looks dark in ordinary viewers. Use FITS, XISF or TIFF.")
    } else {
        tr("Stretched: the image as shown, brightened and ready to view and share. Further processing works less well on it.")
    }
}

/// The choice between saving the linear data and the stretched image, with
/// its explanation. Returns the new choice when it was changed.
pub fn save_kind_ui(ui: &mut egui::Ui, linear: bool, ext: &str) -> Option<bool> {
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Save"));
        if ui.selectable_label(linear, tr("Linear (for processing)")).on_hover_text(save_kind_tip(true)).clicked() {
            pick = Some(true);
        }
        if ui.selectable_label(!linear, tr("Stretched (for viewing)")).on_hover_text(save_kind_tip(false)).clicked() {
            pick = Some(false);
        }
    });
    ui.label(egui::RichText::new(save_kind_tip(linear)).weak().size(12.0));
    if linear && matches!(ext, "jpg" | "jpeg" | "png") {
        ui.colored_label(Color32::from_rgb(230, 150, 30), tr("JPEG and PNG have too few levels for linear data: the faint detail is lost. Save linear data as FITS, XISF or TIFF."));
    }
    pick
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
    // The choices' names are translated here (they come from the engine in English).
    egui::ComboBox::from_id_salt(label).selected_text(tr(name(*v))).show_ui(ui, |ui| {
        for &o in all {
            ui.selectable_value(v, o, tr(name(o)));
        }
    });
    ui.end_row();
    *v != before
}

/// A short summary of a finished job (panorama saved, stack finished, failed …),
/// shown in the tab until it is closed or the next job finishes.
#[derive(Clone, Debug)]
pub struct JobReport {
    pub title: String,
    pub ok: bool,
    pub rows: Vec<(String, String)>,
    /// The file (or folder) made, for "Show in folder".
    pub file: Option<String>,
}

impl JobReport {
    pub fn new(title: impl Into<String>, ok: bool) -> Self {
        Self { title: title.into(), ok, rows: Vec::new(), file: None }
    }
    /// The file the job wrote (adds a "Show in folder" button when it exists).
    pub fn file(mut self, path: &str) -> Self {
        self.file = Some(path.to_string());
        self
    }
    pub fn row(mut self, k: &str, v: impl Into<String>) -> Self {
        self.rows.push((k.to_string(), v.into()));
        self
    }
    /// Draw the report; true when the user closes it.
    pub fn ui(&self, ui: &mut egui::Ui) -> bool {
        let col = if self.ok { Color32::from_rgb(70, 170, 90) } else { Color32::from_rgb(230, 140, 40) };
        let mut close = false;
        egui::Frame::group(ui.style()).stroke(egui::Stroke::new(1.5, col)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(col, egui::RichText::new(format!("{} {}", if self.ok { "✔" } else { "✖" }, self.title)).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = ui.small_button("×").on_hover_text(tr("Close the report")).clicked();
                });
            });
            egui::Grid::new(("report", &self.title)).num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
                for (k, v) in &self.rows {
                    ui.label(egui::RichText::new(k).weak());
                    ui.add(egui::Label::new(v).wrap());
                    ui.end_row();
                }
            });
            if let Some(f) = self.file.as_ref().filter(|f| std::path::Path::new(f).exists()) {
                if ui.small_button(tr("Show in folder")).on_hover_text(f.as_str()).clicked() {
                    dialogs::show_in_folder(f);
                }
            }
        });
        close
    }
}

/// A duration for people: "42 s", "3 min 05 s", "1 h 02 min".
pub fn duration(secs: f64) -> String {
    let s = secs.max(0.0);
    if s < 10.0 {
        format!("{s:.1} s")
    } else if s < 60.0 {
        format!("{s:.0} s")
    } else if s < 3600.0 {
        crate::trf!("{} min {} s", (s / 60.0) as u64, format!("{:02}", (s % 60.0) as u64))
    } else {
        crate::trf!("{} h {} min", (s / 3600.0) as u64, format!("{:02}", ((s % 3600.0) / 60.0) as u64))
    }
}

/// A progress bar with the time elapsed and an estimate of the time left.
pub fn progress_bar(ui: &mut egui::Ui, msg: &str, f: f32, start: Option<std::time::Instant>) {
    // Only the percentage in the bar; the message and the times below it, wrapped,
    // so nothing runs off a narrow panel.
    ui.add(egui::ProgressBar::new(f).desired_width(ui.available_width()).text(format!("{:.0} %", 100.0 * f.clamp(0.0, 1.0))));
    if !msg.is_empty() {
        ui.add(egui::Label::new(msg).wrap());
    }
    if let Some(t) = start {
        let el = t.elapsed().as_secs_f64();
        let left = if f > 0.03 && f < 1.0 { crate::trf!(", about {} left", duration((el * (1.0 - f as f64) / f as f64).max(1.0))) } else { String::new() };
        ui.add(egui::Label::new(egui::RichText::new(format!("{} elapsed{left}", duration(el))).weak()).wrap());
    }
}

/// Size of a file on disk, for reports ("38 MB"), or "?" if it cannot be read.
pub fn file_size(path: &str) -> String {
    std::fs::metadata(path).map(|m| crate::sys::gb(m.len() as f64)).unwrap_or_else(|_| "?".into())
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Stitch,
    Stack,
    Finish,
    Timelapse,
}

/// Appearance: light or dark colours, classic or modern look (rounder, roomier),
/// full or simple layout (advanced controls left out, for beginners).
#[derive(Clone, Copy, PartialEq)]
pub struct Prefs {
    pub dark: bool,
    pub modern: bool,
    pub simple: bool,
    pub lang: i18n::Lang,
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

pub use crate::sys::relpath;

pub struct App {
    tab: Tab,
    prefs: Prefs,
    pub stitch: stitch::StitchTab,
    pub stack: stack::StackTab,
    pub finish: finish::FinishTab,
    pub timelapse: timelapse::TimelapseTab,
    /// Why closing the window was held back (shown in a dialog), if it was.
    confirm_close: Option<Vec<String>>,
    allow_close: bool,
    /// The manual, in a window of its own.
    pub help: help::Help,
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
        let mut app = Self { tab: Tab::Stitch, prefs, stitch: stitch::StitchTab::default(), stack: stack::StackTab::default(), finish: finish::FinishTab::default(), timelapse: timelapse::TimelapseTab::default(), confirm_close: None, allow_close: false, help: help::Help::default() };
        i18n::set_lang(app.prefs.lang);
        app.restore_from(&config_dir().join(REMEMBERED));
        // Project files open in their tab; images go to the stitcher.
        let (stackp, rest): (Vec<String>, Vec<String>) = paths.into_iter().partition(|p| p.to_lowercase().ends_with(stack::PROJECT_EXT));
        let (tlp, rest): (Vec<String>, Vec<String>) = rest.into_iter().partition(|p| p.to_lowercase().ends_with(crate::timelapse::PROJECT_EXT));
        let (panop, imgs): (Vec<String>, Vec<String>) = rest.into_iter().partition(|p| p.to_lowercase().ends_with(stitch::PROJECT_EXT));
        if let Some(p) = panop.first() {
            app.stitch.open_project(p);
            app.tab = Tab::Stitch;
        } else if !imgs.is_empty() {
            app.stitch.add_paths(imgs);
            app.tab = Tab::Stitch;
        }
        if let Some(p) = stackp.first() {
            app.stack.open_project(p);
            app.tab = Tab::Stack;
        }
        if let Some(p) = tlp.first() {
            app.timelapse.open_project(p);
            app.tab = Tab::Timelapse;
        }
        // Development aid (screenshots): PANOSTITCH_FINISH=<file> opens it in Post-processing,
        // PANOSTITCH_TAB=timelapse shows that tab.
        if let Ok(p) = std::env::var("PANOSTITCH_FINISH") {
            app.finish.open(&p);
            app.tab = Tab::Finish;
        }
        if std::env::var("PANOSTITCH_TAB").as_deref() == Ok("timelapse") {
            app.tab = Tab::Timelapse;
        }
        let marker = config_dir().join("crashed");
        if marker.exists() {
            let _ = std::fs::remove_file(&marker);
            app.stitch.notice(crate::trf!("PanoStitch closed unexpectedly last time. The details are in {} – please attach that file when you report it.", crash_log().display()));
        }
        app
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.remember_in(&config_dir().join(REMEMBERED));
    }
}

/// Folder (in the settings folder) with what is remembered between sessions.
const REMEMBERED: &str = "remembered";
/// How many recent projects are listed.
const RECENT_MAX: usize = 10;

fn recent_file() -> std::path::PathBuf {
    config_dir().join("recent_projects.txt")
}

/// Project files opened or saved lately, newest first (only those that still exist).
pub fn recent_projects() -> Vec<String> {
    std::fs::read_to_string(recent_file()).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && std::path::Path::new(l).is_file()).collect()
}

/// Put a project at the top of the recent list.
pub fn remember_project(path: &str) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = std::path::absolute(path).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| path.to_string());
    let mut v = recent_projects();
    v.retain(|q| *q != p);
    v.insert(0, p);
    v.truncate(RECENT_MAX);
    let _ = std::fs::create_dir_all(config_dir());
    let _ = std::fs::write(recent_file(), v.join("\n") + "\n");
}

/// Which tab a project file belongs to (by its extension).
fn project_tab(path: &str) -> Option<Tab> {
    let l = path.to_lowercase();
    if l.ends_with(stack::PROJECT_EXT) {
        Some(Tab::Stack)
    } else if l.ends_with(stitch::PROJECT_EXT) {
        Some(Tab::Stitch)
    } else if l.ends_with(crate::timelapse::PROJECT_EXT) {
        Some(Tab::Timelapse)
    } else {
        None
    }
}

impl Tab {
    fn name(self) -> &'static str {
        match self {
            Tab::Stitch => "stitch",
            Tab::Stack => "stack",
            Tab::Finish => "finish",
            Tab::Timelapse => "timelapse",
        }
    }
    fn from_name(s: &str) -> Option<Tab> {
        [Tab::Stitch, Tab::Stack, Tab::Finish, Tab::Timelapse].into_iter().find(|t| t.name() == s)
    }
    fn title(self) -> &'static str {
        match self {
            Tab::Stitch => tr("Panorama stitcher"),
            Tab::Stack => tr("Star stacking"),
            Tab::Finish => tr("Post-processing"),
            Tab::Timelapse => tr("Timelapse"),
        }
    }
}

impl App {
    /// Open a project file in its tab.
    pub fn open_project(&mut self, path: &str) {
        match project_tab(path) {
            Some(Tab::Stack) => self.stack.open_project(path),
            Some(Tab::Stitch) => self.stitch.open_project(path),
            Some(Tab::Timelapse) => self.timelapse.open_project(path),
            _ => return,
        }
        self.tab = project_tab(path).unwrap_or(self.tab);
    }

    /// Files dropped on the window: projects open in their tab, images (and
    /// folders of images) go to the tab on show.
    pub fn drop_files(&mut self, paths: Vec<String>) {
        let (projects, rest): (Vec<String>, Vec<String>) = paths.into_iter().partition(|p| project_tab(p).is_some());
        if let Some(p) = projects.first() {
            self.open_project(p);
            return;
        }
        let mut imgs: Vec<String> = rest.iter().flat_map(|p| dialogs::expand_paths(p)).filter(|p| crate::io::is_image(p)).collect();
        imgs.dedup();
        if imgs.is_empty() {
            return;
        }
        match self.tab {
            Tab::Stitch => self.stitch.add_dropped(imgs),
            Tab::Stack => self.stack.add_dropped(imgs),
            Tab::Finish => self.finish.add_dropped(imgs),
            Tab::Timelapse => self.timelapse.add_dropped(imgs),
        }
    }

    /// Keep the settings for the next session (in `dir`): the tab on show, each
    /// tab's settings (not its files), RAW reading in the stitcher.
    pub fn remember_in(&mut self, dir: &std::path::Path) {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let mut t = toml::Table::new();
        t.insert("tab".into(), self.tab.name().into());
        t.insert("stitcher".into(), toml::Value::Table(self.stitch.remembered()));
        let _ = std::fs::write(dir.join("window.toml"), toml::to_string(&t).unwrap_or_default());
        let _ = std::fs::write(dir.join("stacking.toml"), toml::to_string(&self.stack.settings_table()).unwrap_or_default());
        let _ = std::fs::write(dir.join("timelapse.tlproj"), self.timelapse.remembered());
    }

    /// The settings kept by [`App::remember_in`]; what is missing or unreadable stays as it is.
    pub fn restore_from(&mut self, dir: &std::path::Path) {
        let read = |f: &str| -> toml::Table { std::fs::read_to_string(dir.join(f)).ok().and_then(|s| s.parse().ok()).unwrap_or_default() };
        let w = read("window.toml");
        if let Some(t) = w.get("tab").and_then(|v| v.as_str()).and_then(Tab::from_name) {
            self.tab = t;
        }
        if let Some(t) = w.get("stitcher").and_then(|v| v.as_table()) {
            self.stitch.restore_remembered(t);
        }
        let st = read("stacking.toml");
        if !st.is_empty() {
            self.stack.apply_settings(&st);
        }
        let tl = dir.join("timelapse.tlproj");
        if tl.is_file() {
            self.timelapse.restore_remembered(&tl);
        }
    }

    pub fn busy(&self) -> bool {
        self.stitch.busy() || self.stack.busy() || self.finish.busy() || self.timelapse.busy()
    }
    /// Show the Post-processing tab.
    pub fn show_finish(&mut self) {
        self.tab = Tab::Finish;
    }
    /// Show the Timelapse tab.
    pub fn show_timelapse(&mut self) {
        self.tab = Tab::Timelapse;
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
                ui.selectable_value(&mut self.tab, Tab::Stitch, egui::RichText::new(tr("Panorama stitcher")).strong());
                ui.selectable_value(&mut self.tab, Tab::Stack, egui::RichText::new(tr("Star stacking (landscape / trails)")).strong());
                ui.selectable_value(&mut self.tab, Tab::Finish, egui::RichText::new(tr("Post-processing")).strong());
                ui.selectable_value(&mut self.tab, Tab::Timelapse, egui::RichText::new(tr("Timelapse")).strong());
                ui.add_space(12.0);
                let mut open = None;
                ui.menu_button(tr("Recent projects"), |ui| {
                    let v = recent_projects();
                    if v.is_empty() {
                        ui.weak(tr("None yet: projects you open or save appear here."));
                    }
                    for p in v {
                        let tab = project_tab(&p).map(|t| t.title()).unwrap_or("");
                        if ui.button(format!("{}  ({tab})", crate::io::basename(&p))).on_hover_text(&p).clicked() {
                            open = Some(p);
                            ui.close();
                        }
                    }
                });
                if let Some(p) = open {
                    self.open_project(&p);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let before = self.prefs;
                    if ui.button(format!("? {}", tr("Help"))).on_hover_text(tr("The manual (F1: the chapter of this tab)")).clicked() {
                        self.help.show_chapter(self.prefs.lang, None);
                    }
                    egui::ComboBox::from_id_salt("ui_lang").selected_text(tr(self.prefs.lang.name())).width(90.0).show_ui(ui, |ui| {
                        for l in i18n::Lang::ALL {
                            ui.selectable_value(&mut self.prefs.lang, l, tr(l.name()));
                        }
                    });
                    ui.checkbox(&mut self.prefs.simple, tr("✓ Simple")).on_hover_text(tr("Leave out the advanced controls"));
                    ui.checkbox(&mut self.prefs.modern, tr("◆ Modern"));
                    ui.checkbox(&mut self.prefs.dark, tr("☾ Dark"));
                    if self.prefs != before {
                        apply_style(&ctx, self.prefs);
                        save_prefs(self.prefs);
                        i18n::set_lang(self.prefs.lang);
                    }
                });
            });
        });
        // Files dragged onto the window.
        let dropped: Vec<String> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_string_lossy().to_string()).filter(|p| !p.is_empty()).collect());
        if !dropped.is_empty() {
            self.drop_files(dropped);
        }
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let r = ctx.content_rect();
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("drop_overlay")));
            p.rect_filled(r, 0.0, Color32::from_black_alpha(150));
            let txt = match self.tab {
                Tab::Stitch => tr("Drop to add the images to the panorama"),
                Tab::Stack => tr("Drop to add the files to the stacking"),
                Tab::Finish => tr("Drop to open the image"),
                Tab::Timelapse => tr("Drop to add the frames"),
            };
            p.text(r.center(), egui::Align2::CENTER_CENTER, format!("{txt}\n{}", tr("(project files open in their tab)")), egui::FontId::proportional(24.0), Color32::WHITE);
        }
        // Stacked panels handed to the stitcher.
        let handed = std::mem::take(&mut self.stack.to_stitcher);
        if !handed.is_empty() {
            self.stitch.add_paths(handed);
            self.tab = Tab::Stitch;
        }
        // Results handed to Post-processing (the last one wins).
        let handed: Vec<String> = std::mem::take(&mut self.stack.to_finish).into_iter().chain(std::mem::take(&mut self.stitch.to_finish)).collect();
        if let Some(p) = handed.last() {
            self.finish.open(p);
            self.tab = Tab::Finish;
        }
        self.stitch.simple = self.prefs.simple;
        self.stack.simple = self.prefs.simple;
        self.finish.simple = self.prefs.simple;
        self.timelapse.simple = self.prefs.simple;
        self.timelapse.finish_look = self.finish.has_image().then(|| self.finish.p.clone());
        match self.tab {
            Tab::Stitch => self.stitch.ui(ui, &ctx),
            Tab::Stack => self.stack.ui(ui, &ctx),
            Tab::Finish => self.finish.ui(ui, &ctx),
            Tab::Timelapse => self.timelapse.ui(ui, &ctx),
        }
        // F1: the manual at the chapter of the tab on show.
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            let ch = match self.tab {
                Tab::Stitch => "5",
                Tab::Stack => "13",
                Tab::Finish => "16",
                Tab::Timelapse => "17",
            };
            self.help.show_chapter(self.prefs.lang, Some(ch));
        }
        self.help.ui(&ctx);
        if self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.guard_close(&ctx);
    }

    /// What would be lost by closing the window now.
    pub fn close_reasons(&mut self) -> Vec<String> {
        let mut r = Vec::new();
        if self.stack.running() {
            r.push(tr("Stacking is running; it will be stopped.").to_string());
        } else if self.stitch.busy() {
            r.push(tr("The stitcher is still working (rendering or saving).").to_string());
        } else if self.finish.saving() {
            r.push(tr("Post-processing is still saving the image.").to_string());
        } else if self.timelapse.busy() {
            r.push(tr("A video is being made; it will be stopped and not kept.").to_string());
        }
        if self.stitch.unsaved() {
            r.push(tr("Panorama stitcher: changes not saved in a project.").to_string());
        }
        if self.stack.unsaved() {
            r.push(tr("Star stacking: setup not saved in a project.").to_string());
        }
        if self.finish.unsaved() {
            r.push(tr("Post-processing: adjusted image not saved.").to_string());
        }
        if self.timelapse.unsaved() {
            r.push(tr("Timelapse: frames and settings not saved in a project.").to_string());
        }
        r
    }

    /// Ask before closing the window when work would be lost.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.confirm_close.is_none() {
            let r = self.close_reasons();
            if !r.is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm_close = Some(r);
            }
        }
        let Some(reasons) = self.confirm_close.clone() else { return };
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("confirm_close")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading(tr("Quit PanoStitch?"));
            for r in &reasons {
                ui.label(format!("• {r}"));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new(tr("Quit anyway")).strong()).clicked() {
                    choice = Some(0);
                }
                if self.stitch.unsaved() && ui.button(tr("Save panorama project…")).clicked() {
                    choice = Some(1);
                }
                if self.stack.unsaved() && ui.button(tr("Save stacking project…")).clicked() {
                    choice = Some(2);
                }
                if self.timelapse.unsaved() && ui.button(tr("Save timelapse project…")).clicked() {
                    choice = Some(4);
                }
                if ui.button(tr("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    choice = Some(3);
                }
            });
        });
        match choice {
            Some(0) => {
                self.confirm_close = None;
                self.allow_close = true;
                self.stack.cancel_run();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(1) => {
                self.confirm_close = None;
                self.tab = Tab::Stitch;
                self.stitch.ask_save_project(true);
            }
            Some(2) => {
                self.confirm_close = None;
                self.tab = Tab::Stack;
                self.stack.ask_save_project(true);
            }
            Some(4) => {
                self.confirm_close = None;
                self.tab = Tab::Timelapse;
                self.timelapse.ask_save_project(true);
            }
            Some(_) => self.confirm_close = None,
            None => {}
        }
    }
}

/// Folder for the program's settings.
pub fn config_dir() -> std::path::PathBuf {
    // PANOSTITCH_CONFIG points elsewhere (tests, a portable copy).
    if let Some(d) = std::env::var_os("PANOSTITCH_CONFIG") {
        return d.into();
    }
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("panostitch")
}
fn load_prefs() -> Prefs {
    let t: toml::Table = std::fs::read_to_string(config_dir().join("ui.toml")).ok().and_then(|s| s.parse().ok()).unwrap_or_default();
    let b = |k: &str| t.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    // PANOSTITCH_LANG=en|cs|de overrides (tests); else the saved choice, else the system's language.
    let lang = std::env::var("PANOSTITCH_LANG").ok().and_then(|v| i18n::Lang::from_code(&v))
        .or_else(|| t.get("lang").and_then(|v| v.as_str()).and_then(i18n::Lang::from_code))
        .unwrap_or_else(i18n::Lang::from_system);
    Prefs { dark: b("dark"), modern: b("modern"), simple: b("simple"), lang }
}
fn save_prefs(p: Prefs) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("ui.toml"), format!("dark = {}\nmodern = {}\nsimple = {}\nlang = \"{}\"\n", p.dark, p.modern, p.simple, p.lang.code()));
}

/// Where a crash is recorded.
pub fn crash_log() -> std::path::PathBuf {
    config_dir().join("crash.log")
}

/// Record panics (with the place in the code) in the crash log, so a crash
/// leaves something to report even when there is no console (Windows).
fn install_crash_log() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Panics inside background jobs are caught and shown in the status line; log them all anyway.
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let text = format!(
            "{} PanoStitch {} ({} {}), thread {thread}:\n{info}\n{}\n\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::backtrace::Backtrace::force_capture()
        );
        let _ = std::fs::create_dir_all(config_dir());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(crash_log()) {
            use std::io::Write;
            let _ = f.write_all(text.as_bytes());
        }
        // A panic on the window's thread ends the program: say so at the next start.
        if std::thread::current().name() == Some("main") {
            let _ = std::fs::write(config_dir().join("crashed"), "");
        }
        prev(info);
    }));
}

/// Show a message when there is no window to show it in (e.g. the graphics driver is too old).
pub fn fatal_message(text: &str) {
    eprintln!("{text}");
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        unsafe { MessageBoxW(std::ptr::null_mut(), w(text).as_ptr(), w("PanoStitch").as_ptr(), MB_OK | MB_ICONERROR) };
    }
}

/// Open the window with `paths` loaded into the stitcher.
pub fn run(paths: Vec<String>) -> eframe::Result {
    install_crash_log();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1700.0, 1000.0]).with_title(concat!("PanoStitch ", env!("CARGO_PKG_VERSION"))),
        ..Default::default()
    };
    eframe::run_native("PanoStitch", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, paths)))))
}

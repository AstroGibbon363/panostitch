//! The manual inside the program, as in the help windows of older desktop
//! programs: contents on the left (with search), the chapter on the right,
//! links between chapters, Back. F1 opens the chapter of the tab on show.
//! The manuals (docs/manual_*.md) are built into the program.

use super::i18n::{tr, Lang};
use egui::text::LayoutJob;
use egui::{Color32, FontId, RichText, TextFormat};

fn manual(l: Lang) -> &'static str {
    match l {
        Lang::En => include_str!("../../docs/manual_en.md"),
        Lang::Cs => include_str!("../../docs/manual_cs.md"),
        Lang::De => include_str!("../../docs/manual_de.md"),
        Lang::Es => include_str!("../../docs/manual_es.md"),
        Lang::Fr => include_str!("../../docs/manual_fr.md"),
        Lang::Pl => include_str!("../../docs/manual_pl.md"),
    }
}

/// A heading of the manual and the lines up to the next one.
#[derive(Clone, Debug)]
pub struct Section {
    /// 1 = the manual's title (introduction), 2 = chapter, 3 = part of a chapter.
    pub level: u8,
    pub title: String,
    /// Link target, as GitHub makes it ("13-star-stacking").
    pub slug: String,
    /// The chapter (level 1 or 2 section) this one belongs to.
    pub chapter: usize,
    /// Lines of the manual: the heading's line, and the end (exclusive) of its text.
    start: usize,
    end: usize,
}

/// Link target of a heading, as GitHub makes it: lower case, spaces to dashes,
/// letters (also accented), digits, dashes and underscores kept.
pub fn slug(title: &str) -> String {
    title.trim().to_lowercase().chars().filter_map(|c| if c == ' ' { Some('-') } else if c.is_alphanumeric() || c == '-' || c == '_' { Some(c) } else { None }).collect()
}

/// The manual's headings (outside code blocks).
pub fn sections(text: &str) -> Vec<Section> {
    let lines: Vec<&str> = text.lines().collect();
    let mut v: Vec<Section> = Vec::new();
    let mut code = false;
    for (i, l) in lines.iter().enumerate() {
        if l.trim_start().starts_with("```") {
            code = !code;
            continue;
        }
        if code {
            continue;
        }
        let level = if l.starts_with("# ") { 1 } else if l.starts_with("## ") { 2 } else if l.starts_with("### ") { 3 } else { 0 };
        if level == 0 {
            continue;
        }
        let title = l[level as usize + 1..].trim().to_string();
        if let Some(last) = v.last_mut() {
            last.end = i;
        }
        let chapter = if level <= 2 { v.len() } else { v.iter().rposition(|s| s.level <= 2).unwrap_or(0) };
        v.push(Section { level, slug: slug(&title), title, chapter, start: i, end: lines.len() });
    }
    v
}

/// The text of a section and (for a chapter) of its parts.
fn chapter_lines(text: &str, secs: &[Section], k: usize) -> Vec<(usize, String)> {
    let s = &secs[k];
    let end = secs.iter().skip(k + 1).find(|t| t.level <= s.level.max(2)).map(|t| t.start).unwrap_or(usize::MAX);
    text.lines().enumerate().skip(s.start).take_while(|(i, _)| *i < end).map(|(i, l)| (i, l.to_string())).collect()
}

/// A piece of a line of text.
#[derive(Debug, PartialEq)]
enum Span {
    Text { s: String, bold: bool, italic: bool, code: bool },
    Link { text: String, target: String },
}

/// **bold**, *italic*, `code` and [links](target) in a line.
fn spans(line: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let (mut bold, mut italic) = (false, false);
    let mut cur = String::new();
    let c: Vec<char> = line.chars().collect();
    let mut i = 0;
    let flush = |cur: &mut String, out: &mut Vec<Span>, bold: bool, italic: bool| {
        if !cur.is_empty() {
            out.push(Span::Text { s: std::mem::take(cur), bold, italic, code: false });
        }
    };
    while i < c.len() {
        if c[i] == '`' {
            if let Some(j) = c[i + 1..].iter().position(|&x| x == '`') {
                flush(&mut cur, &mut out, bold, italic);
                out.push(Span::Text { s: c[i + 1..i + 1 + j].iter().collect(), bold, italic, code: true });
                i += j + 2;
                continue;
            }
        }
        if c[i] == '*' && c.get(i + 1) == Some(&'*') {
            flush(&mut cur, &mut out, bold, italic);
            bold = !bold;
            i += 2;
            continue;
        }
        // A single * starts italics only before a non-space (not "a * b").
        if c[i] == '*' && (italic || c.get(i + 1).is_some_and(|n| !n.is_whitespace())) {
            flush(&mut cur, &mut out, bold, italic);
            italic = !italic;
            i += 1;
            continue;
        }
        if c[i] == '[' {
            if let Some(j) = c[i + 1..].iter().position(|&x| x == ']') {
                let close = i + 1 + j;
                if c.get(close + 1) == Some(&'(') {
                    if let Some(k) = c[close + 2..].iter().position(|&x| x == ')') {
                        flush(&mut cur, &mut out, bold, italic);
                        let text: String = c[i + 1..close].iter().collect();
                        let target: String = c[close + 2..close + 2 + k].iter().collect();
                        out.push(Span::Link { text: text.replace("**", ""), target });
                        i = close + 3 + k;
                        continue;
                    }
                }
            }
        }
        if c[i] == '\\' && i + 1 < c.len() {
            cur.push(c[i + 1]);
            i += 2;
            continue;
        }
        cur.push(c[i]);
        i += 1;
    }
    flush(&mut cur, &mut out, bold, italic);
    out
}

/// Every link target in the manual (for the tests).
pub fn links(text: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut code = false;
    for l in text.lines() {
        if l.trim_start().starts_with("```") {
            code = !code;
        }
        if !code {
            v.extend(spans(l).into_iter().filter_map(|s| if let Span::Link { target, .. } = s { Some(target) } else { None }));
        }
    }
    v
}

/// The manual's text for a language (for the tests).
pub fn manual_text(l: Lang) -> &'static str {
    manual(l)
}

pub struct Help {
    pub open: bool,
    /// The manual's language (follows the window's when it opens).
    lang: Lang,
    secs: Vec<Section>,
    /// The chapter shown, and the section to scroll to.
    page: usize,
    scroll_to: Option<usize>,
    history: Vec<usize>,
    search: String,
}

impl Default for Help {
    fn default() -> Self {
        Self { open: false, lang: Lang::En, secs: sections(manual(Lang::En)), page: 0, scroll_to: None, history: Vec::new(), search: String::new() }
    }
}

impl Help {
    fn set_lang(&mut self, l: Lang) {
        if l != self.lang || self.secs.is_empty() {
            // Keep the chapter: the manuals have the same chapters in the same order.
            let chapter_no = self.secs.get(self.page).map(|s| s.title.split('.').next().unwrap_or("").to_string());
            self.lang = l;
            self.secs = sections(manual(l));
            self.history.clear();
            self.page = chapter_no.and_then(|n| self.find_chapter(&n)).unwrap_or(0);
            self.scroll_to = Some(self.page);
        }
    }

    /// The chapter whose number is `n` ("13").
    fn find_chapter(&self, n: &str) -> Option<usize> {
        self.secs.iter().position(|s| s.level == 2 && s.title.split('.').next() == Some(n))
    }

    /// Open the help (in the window's language) at chapter `n`, or where it was.
    pub fn show_chapter(&mut self, lang: Lang, n: Option<&str>) {
        self.set_lang(lang);
        if let Some(k) = n.and_then(|n| self.find_chapter(n)) {
            self.go(k);
        }
        self.open = true;
    }

    fn go(&mut self, k: usize) {
        let chapter = self.secs[k].chapter;
        if chapter != self.page {
            self.history.push(self.page);
        }
        self.page = chapter;
        self.scroll_to = Some(k);
    }

    fn follow(&mut self, target: &str) {
        if let Some(a) = target.strip_prefix('#') {
            if let Some(k) = self.secs.iter().position(|s| s.slug == a) {
                self.go(k);
            }
        } else if let Some(l) = target.strip_prefix("manual_").and_then(|t| t.strip_suffix(".md")).and_then(Lang::from_code) {
            self.set_lang(l);
        } else if target.starts_with("http") {
            open_url(target);
        }
    }

    /// The help window (when open).
    pub fn ui(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        egui::Window::new(RichText::new(tr("Help")).strong())
            .open(&mut open)
            .default_size([980.0, 700.0])
            .min_size([520.0, 320.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| self.contents(ui));
        self.open = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.history.is_empty(), egui::Button::new(format!("◀ {}", tr("Back")))).clicked() {
                if let Some(p) = self.history.pop() {
                    self.page = p;
                    self.scroll_to = Some(p);
                }
            }
            ui.add_space(8.0);
            let mut l = self.lang;
            egui::ComboBox::from_id_salt("help_lang").selected_text(l.name()).show_ui(ui, |ui| {
                for x in Lang::ALL {
                    ui.selectable_value(&mut l, x, x.name());
                }
            });
            self.set_lang(l);
            ui.add_space(8.0);
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(tr("Search the manual")).desired_width(220.0));
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
            }
        });
        ui.separator();
        let text = manual(self.lang);
        let avail = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(250.0);
                egui::ScrollArea::vertical().id_salt("help_toc").max_height(avail).auto_shrink([false, false]).show(ui, |ui| self.toc(ui, text));
            });
            ui.separator();
            ui.vertical(|ui| {
                egui::ScrollArea::vertical().id_salt(("help_page", self.page, self.lang.code())).max_height(avail).auto_shrink([false, false]).show(ui, |ui| self.page_ui(ui, text));
            });
        });
    }

    fn toc(&mut self, ui: &mut egui::Ui, text: &str) {
        let q = self.search.trim().to_lowercase();
        let mut go = None;
        if q.is_empty() {
            for (k, s) in self.secs.iter().enumerate() {
                if ["Contents", "Obsah", "Inhalt", "Índice", "Sommaire", "Spis treści"].contains(&s.title.as_str()) {
                    continue;
                }
                let current = s.chapter == self.page;
                let label = match s.level {
                    3 => RichText::new(format!("    {}", s.title)).size(12.5),
                    _ => RichText::new(&s.title).strong(),
                };
                // Parts of a chapter are listed while it is open.
                if s.level == 3 && !current {
                    continue;
                }
                if ui.selectable_label(current && s.level <= 2, label).clicked() {
                    go = Some(k);
                }
            }
        } else {
            let lines: Vec<&str> = text.lines().collect();
            let mut n = 0;
            for (k, s) in self.secs.iter().enumerate() {
                let hits = lines[s.start..s.end].iter().filter(|l| l.to_lowercase().contains(&q)).count();
                if hits > 0 {
                    n += 1;
                    if ui.selectable_label(false, format!("{}  ({hits})", s.title)).clicked() {
                        go = Some(k);
                    }
                }
            }
            if n == 0 {
                ui.weak(tr("Nothing found."));
            }
        }
        if let Some(k) = go {
            self.go(k);
        }
    }

    fn page_ui(&mut self, ui: &mut egui::Ui, text: &str) {
        let Some(sec) = self.secs.get(self.page) else { return };
        let lines = chapter_lines(text, &self.secs, self.page);
        let first_line = sec.start;
        let mut follow = None;
        let mut i = 0;
        let mut para: Vec<String> = Vec::new();
        let flush = |ui: &mut egui::Ui, para: &mut Vec<String>, follow: &mut Option<String>| {
            if !para.is_empty() {
                inline(ui, &para.join(" "), 14.0, follow);
                para.clear();
                ui.add_space(6.0);
            }
        };
        while i < lines.len() {
            let (ln, l) = (&lines[i].0, lines[i].1.as_str());
            let t = l.trim();
            if t.starts_with("```") {
                flush(ui, &mut para, &mut follow);
                let mut code = Vec::new();
                i += 1;
                while i < lines.len() && !lines[i].1.trim().starts_with("```") {
                    code.push(lines[i].1.clone());
                    i += 1;
                }
                i += 1;
                egui::Frame::group(ui.style()).fill(ui.visuals().code_bg_color).show(ui, |ui| {
                    egui::ScrollArea::horizontal().id_salt(("code", ln)).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(code.join("\n")).monospace()).extend());
                    });
                });
                ui.add_space(6.0);
                continue;
            }
            let level = if l.starts_with("### ") { 3 } else if l.starts_with("## ") { 2 } else if l.starts_with("# ") { 1 } else { 0 };
            if level > 0 {
                flush(ui, &mut para, &mut follow);
                let size = match level {
                    1 => 24.0,
                    2 => 21.0,
                    _ => 17.0,
                };
                let r = ui.label(RichText::new(t.trim_start_matches('#').trim()).size(size).strong());
                if self.scroll_to.is_some_and(|k| self.secs.get(k).is_some_and(|s| s.start == *ln)) || (self.scroll_to == Some(self.page) && *ln == first_line) {
                    r.scroll_to_me(Some(egui::Align::TOP));
                    self.scroll_to = None;
                }
                ui.add_space(4.0);
                i += 1;
                continue;
            }
            if t.starts_with('|') {
                flush(ui, &mut para, &mut follow);
                let mut rows: Vec<Vec<String>> = Vec::new();
                while i < lines.len() && lines[i].1.trim().starts_with('|') {
                    let r = lines[i].1.trim();
                    let cells: Vec<String> = split_row(r);
                    if !cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) {
                        rows.push(cells);
                    }
                    i += 1;
                }
                table(ui, &rows, &mut follow);
                ui.add_space(6.0);
                continue;
            }
            if t == "---" {
                flush(ui, &mut para, &mut follow);
                ui.separator();
                i += 1;
                continue;
            }
            let bullet = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "));
            let number = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()));
            if bullet.is_some() || number.is_some() {
                flush(ui, &mut para, &mut follow);
                let (mark, mut item) = match (bullet, number) {
                    (Some(b), _) => ("•".to_string(), b.to_string()),
                    (None, Some((n, rest))) => (format!("{n}."), rest.to_string()),
                    _ => unreachable!(),
                };
                let indent = l.len() - l.trim_start().len();
                i += 1;
                // Continuation lines are indented.
                while i < lines.len() {
                    let n = &lines[i].1;
                    let nt = n.trim();
                    if nt.is_empty() || n.len() - n.trim_start().len() <= indent || nt.starts_with("- ") || nt.starts_with("* ") || nt.starts_with('|') || nt.starts_with("```") {
                        break;
                    }
                    item.push(' ');
                    item.push_str(nt);
                    i += 1;
                }
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0 + indent as f32 * 6.0);
                    ui.label(RichText::new(mark).strong());
                    ui.vertical(|ui| inline(ui, &item, 14.0, &mut follow));
                });
                continue;
            }
            if t.is_empty() {
                flush(ui, &mut para, &mut follow);
                i += 1;
                continue;
            }
            para.push(t.trim_start_matches("> ").to_string());
            i += 1;
        }
        flush(ui, &mut para, &mut follow);
        if let Some(t) = follow {
            self.follow(&t);
        }
    }
}

/// The cells of a table row ("| a | b |"), with "\|" kept inside cells.
fn split_row(r: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut code = false;
    let mut prev = ' ';
    for ch in r.trim().trim_start_matches('|').chars() {
        if ch == '`' {
            code = !code;
        }
        if ch == '|' && !code && prev != '\\' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(ch);
        }
        prev = ch;
    }
    if !cur.trim().is_empty() {
        cells.push(cur.trim().to_string());
    }
    cells
}

/// A table: columns as wide as their texts need (within limits); cells wrap.
fn table(ui: &mut egui::Ui, rows: &[Vec<String>], follow: &mut Option<String>) {
    let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncol == 0 {
        return;
    }
    let weight: Vec<f32> = (0..ncol).map(|c| rows.iter().map(|r| r.get(c).map(|s| s.chars().count()).unwrap_or(0)).max().unwrap_or(0).clamp(6, 70) as f32).collect();
    let total: f32 = weight.iter().sum();
    let w = ui.available_width() - 12.0 * ncol as f32;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        for (k, r) in rows.iter().enumerate() {
            ui.horizontal_top(|ui| {
                for (c, wt) in weight.iter().enumerate() {
                    let cw = (w * wt / total).max(60.0);
                    ui.allocate_ui_with_layout(egui::vec2(cw, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(cw);
                        let txt = r.get(c).map(|s| s.as_str()).unwrap_or("");
                        if k == 0 {
                            ui.label(RichText::new(txt.replace("**", "")).strong());
                        } else {
                            inline(ui, txt, 13.5, follow);
                        }
                    });
                }
            });
            if k + 1 < rows.len() {
                ui.separator();
            }
        }
    });
}

/// A paragraph with bold, italic, code and links; wraps to the width.
fn inline(ui: &mut egui::Ui, text: &str, size: f32, follow: &mut Option<String>) {
    let sp = spans(text);
    let strong = ui.visuals().strong_text_color();
    let normal = ui.visuals().text_color();
    let code_bg = ui.visuals().code_bg_color;
    let fmt = |bold: bool, italic: bool, code: bool| TextFormat {
        font_id: if code { FontId::monospace(size - 1.0) } else { FontId::proportional(size) },
        color: if bold { strong } else { normal },
        italics: italic,
        background: if code { code_bg } else { Color32::TRANSPARENT },
        ..Default::default()
    };
    if !sp.iter().any(|s| matches!(s, Span::Link { .. })) {
        let mut job = LayoutJob::default();
        for s in &sp {
            if let Span::Text { s, bold, italic, code } = s {
                job.append(s, 0.0, fmt(*bold, *italic, *code));
            }
        }
        job.wrap.max_width = ui.available_width();
        ui.add(egui::Label::new(job).wrap());
        return;
    }
    // With links: word by word, so the paragraph still wraps.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for s in sp {
            match s {
                Span::Text { s, bold, italic, code } => {
                    for w in s.split_inclusive(' ') {
                        let mut job = LayoutJob::default();
                        job.append(w, 0.0, fmt(bold, italic, code));
                        ui.label(job);
                    }
                }
                Span::Link { text, target } => {
                    if ui.link(RichText::new(text).size(size)).on_hover_text(&target).clicked() {
                        *follow = Some(target);
                    }
                }
            }
        }
    });
}

fn open_url(url: &str) {
    if !super::dialogs::external_allowed() {
        return;
    }
    let mut c = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(url);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    std::thread::spawn(move || {
        let _ = c.status();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_spans() {
        let s = spans("a **b** *c* `d` [e](#f) 2 * 3");
        assert_eq!(s[1], Span::Text { s: "b".into(), bold: true, italic: false, code: false });
        assert_eq!(s[3], Span::Text { s: "c".into(), bold: false, italic: true, code: false });
        assert_eq!(s[5], Span::Text { s: "d".into(), bold: false, italic: false, code: true });
        assert_eq!(s[7], Span::Link { text: "e".into(), target: "#f".into() });
        assert_eq!(s[8], Span::Text { s: " 2 * 3".into(), bold: false, italic: false, code: false });
        assert_eq!(slug("13. Star stacking"), "13-star-stacking");
        assert_eq!(slug("2. Spuštění programu"), "2-spuštění-programu");
        assert_eq!(slug("9. FITS files (astronomy)"), "9-fits-files-astronomy");
    }
}

//! The window's languages and the built-in manual: every text shown through
//! `tr` / `trf!` has its Czech and German translation, the manuals' links
//! lead somewhere, and the help window shows every chapter in every language.
//! (A test binary of its own: switching the language is global.)

use panostitch::gui::help::{self, sections};
use panostitch::gui::i18n::{self, tr_in, Lang};
use panostitch::gui::App;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// The text of the string literal starting at `s` (after the opening quote), unescaped.
fn literal(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '\r' | '\n' => {
                    // A line continuation: skip the next line's indentation.
                    let rest: String = it.clone().collect();
                    let skip = rest.len() - rest.trim_start().len();
                    for _ in 0..rest[..skip].chars().count() {
                        it.next();
                    }
                }
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

/// Length in bytes of a string literal's source text up to (not including) its closing quote.
fn raw_literal_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return i,
            _ => i += 1,
        }
    }
    b.len()
}

/// The names the engine gives its choices (shown translated in menus).
fn engine_names() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for f in ["src/stacker/mod.rs", "src/stitcher/mod.rs", "src/stitcher/render.rs", "src/timelapse.rs", "src/finish.rs"] {
        let src = std::fs::read_to_string(f).unwrap();
        let mut inside = false;
        for l in src.lines() {
            let t = l.trim();
            if t.starts_with("pub fn name(self) -> &'static str") {
                inside = true;
            } else if inside && t == "}" && l.starts_with("    }") {
                inside = false;
            }
            if (inside && t.contains("=> \"")) || t.starts_with("named_enum!(") {
                for part in t.split("=> \"").skip(1) {
                    if let Some(name) = literal(part) {
                        v.push((name, f.to_string()));
                    }
                }
            }
        }
    }
    v
}

/// Every text passed to `tr("…")` or `trf!("…", …)` in the window's code.
fn texts_in_code() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for e in std::fs::read_dir("src/gui").unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".rs") || name.starts_with("i18n") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        // Lines marked "// tr": every text on them is shown translated (tables of names).
        for l in src.lines().filter(|l| l.trim_end().ends_with("// tr")) {
            let mut rest = l;
            while let Some(k) = rest.find('"') {
                let Some(t) = literal(&rest[k + 1..]) else { break };
                let len = rest[k + 1..].find(|_c: char| true).map(|_| 0).unwrap_or(0);
                let _ = len;
                if t.chars().filter(|c| c.is_alphabetic()).count() >= 2 {
                    v.push((t.clone(), name.clone()));
                }
                // Skip past this literal (its raw length in the source).
                let raw_end = raw_literal_len(&rest[k + 1..]);
                rest = &rest[k + 1 + raw_end + 1..];
            }
        }
        for pat in ["tr(\"", "trf!(\"", "tr_ctx(\""] {
            let mut from = 0;
            while let Some(k) = src[from..].find(pat) {
                let at = from + k;
                // Not part of a longer name (e.g. "str(").
                let before = src[..at].chars().last().unwrap_or(' ');
                if !(before.is_alphanumeric() || before == '_') {
                    if let Some(t) = literal(&src[at + pat.len()..]) {
                        v.push((t, name.clone()));
                    }
                }
                from = at + pat.len();
            }
        }
    }
    v
}

#[test]
fn every_text_is_translated() {
    let texts: Vec<(String, String)> = texts_in_code().into_iter().chain(engine_names()).collect();
    for l in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
        let pairs = i18n::table_of(l);
        let mut dup = HashSet::new();
        for (e, _) in &pairs {
            assert!(dup.insert(*e), "{l:?}: twice in the table: {e:?}");
        }
        let table: HashMap<&str, &str> = pairs.into_iter().collect();
        let mut missing = Vec::new();
        for (t, file) in &texts {
            match table.get(t.as_str()) {
                None => missing.push(format!("{file}: {t:?}")),
                Some(x) => {
                    assert!(!x.is_empty(), "{l:?} {file}: no translation for {t:?}");
                    assert_eq!(x.matches("{}").count(), t.matches("{}").count(), "{l:?}: {{}} places differ: {t:?} → {x:?}");
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "{l:?}: {} texts without translation:\n{}", missing.len(), missing.join("\n"));
    }
    assert_eq!(tr_in(Lang::Cs, "Help"), "Nápověda");
    assert_eq!(tr_in(Lang::De, "Help"), "Hilfe");
    assert_eq!(tr_in(Lang::Pl, "Help"), "Pomoc");
    assert_eq!(tr_in(Lang::Fr, "Help"), "Aide");
    assert_eq!(tr_in(Lang::Es, "Help"), "Ayuda");
    assert_eq!(i18n::fill("{} of {}", &[&3, &"x"]), "3 of x");
}

#[test]
fn manual_links_lead_somewhere() {
    let numbers = |l: Lang| -> Vec<String> { sections(help::manual_text(l)).iter().filter(|s| s.level == 2).filter_map(|s| s.title.split_once(". ").map(|x| x.0.to_string())).collect() };
    let en = numbers(Lang::En);
    assert!(en.len() >= 17, "{en:?}");
    for l in Lang::ALL {
        // The same chapters in every language (F1 finds a chapter by its number).
        assert_eq!(numbers(l), en, "{l:?}");
        let text = help::manual_text(l);
        let slugs: HashSet<String> = sections(text).into_iter().map(|s| s.slug).collect();
        for t in help::links(text) {
            if let Some(a) = t.strip_prefix('#') {
                assert!(slugs.contains(a), "{l:?}: link to #{a} leads nowhere");
            } else {
                assert!(t.starts_with("manual_") || t.starts_with("http"), "{l:?}: link {t}");
            }
        }
    }
}

#[test]
fn help_window_and_languages() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PANOSTITCH_CONFIG", dir.path());
    std::env::set_var("PANOSTITCH_LANG", "en");
    let ctx = egui::Context::default();
    let mut app = App::with_context(&ctx, vec![]);
    let t0 = Instant::now();
    let frame = |app: &mut App| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1700.0, 1000.0))), time: Some(t0.elapsed().as_secs_f64()), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
    };
    for l in Lang::ALL {
        i18n::set_lang(l);
        // Every tab draws in every language.
        for tab in 0..4 {
            match tab {
                0 => app.show_stack(false),
                1 => app.show_stack(true),
                2 => app.show_finish(),
                _ => app.show_timelapse(),
            }
            frame(&mut app);
            frame(&mut app);
        }
        // Every chapter of the manual in the help window.
        let n = sections(help::manual_text(l)).iter().filter(|s| s.level == 2).count();
        for ch in 1..=n {
            app.help.show_chapter(l, Some(&ch.to_string()));
            frame(&mut app);
            frame(&mut app);
            assert!(app.help.open);
        }
    }
    // The stacking engine's messages, a nested one too, and one without a template.
    i18n::set_lang(Lang::Cs);
    let tm = panostitch::gui::engine_text::tr_msg;
    assert_eq!(tm("alignment failed: only 3 distinct stars match"), "zarovnání selhalo: shoduje se jen 3 různých hvězd");
    assert_eq!(tm("Loading, calibrating, aligning: 4/12"), "Načítání, kalibrace, zarovnání: 4/12");
    assert_eq!(tm("something new"), "something new");
    // A value that is a name in the table (the kind of distortion correction).
    assert_eq!(tm("Correcting distortion (Lens)…"), format!("Oprava zkreslení ({})…", i18n::tr("Lens")));
    i18n::set_lang(Lang::En);
    assert_eq!(tm("alignment failed: too few stars"), "alignment failed: too few stars");
}

/// `cargo test --test i18n -- --ignored dump`: the texts still missing from the table, one per line.
#[test]
#[ignore]
fn dump_missing() {
    // I18N_LANG=pl (default cs) and I18N_DUMP=file: the texts that language still lacks, one per line.
    let l = Lang::from_code(&std::env::var("I18N_LANG").unwrap_or_else(|_| "cs".into())).unwrap();
    let have: HashSet<&str> = i18n::table_of(l).into_iter().map(|e| e.0).collect();
    let mut v: Vec<String> = texts_in_code().into_iter().chain(engine_names()).map(|t| t.0).filter(|t| !have.contains(t.as_str())).collect();
    v.sort();
    v.dedup();
    let out = std::env::var("I18N_DUMP").unwrap_or_else(|_| "i18n_missing.txt".into());
    std::fs::write(&out, v.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join("\n")).unwrap();
    println!("{} texts → {out}", v.len());
}

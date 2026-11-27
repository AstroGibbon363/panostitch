//! The window in English, Czech, German, Spanish, French or Polish. Texts are
//! written in English in the code and looked up in tables when shown
//! (src/gui/i18n_table.rs: Czech and German; i18n_es.rs, i18n_fr.rs, i18n_pl.rs):
//! `tr("Add…")` for fixed texts, `trf!("{} frames", n)` for texts with values.
//! A test checks that every text passed to `tr` / `trf!` has its translations.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    En,
    Cs,
    De,
    Es,
    Fr,
    Pl,
}

impl Lang {
    /// In the order of their own names (Čeština, Deutsch, English, Español, Français, Polski).
    pub const ALL: [Lang; 6] = [Lang::Cs, Lang::De, Lang::En, Lang::Es, Lang::Fr, Lang::Pl];
    /// Code used in the settings file and the manual's file name.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Cs => "cs",
            Lang::De => "de",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::Pl => "pl",
        }
    }
    pub fn from_code(s: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == s)
    }
    /// The language's name in itself.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Cs => "Čeština",
            Lang::De => "Deutsch",
            Lang::Es => "Español",
            Lang::Fr => "Français",
            Lang::Pl => "Polski",
        }
    }
    /// The language the system is set to, if it is one of these.
    pub fn from_system() -> Lang {
        // Windows: the language of the user interface (LANG is rarely set there).
        #[cfg(windows)]
        let l = match unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff {
            0x05 => Lang::Cs,
            0x07 => Lang::De,
            0x0a => Lang::Es,
            0x0c => Lang::Fr,
            0x15 => Lang::Pl,
            _ => Lang::En,
        };
        #[cfg(not(windows))]
        let l = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
            .next()
            .map(|v| v.to_lowercase())
            .map(|v| Lang::ALL.into_iter().find(|l| *l != Lang::En && v.starts_with(l.code())).unwrap_or(Lang::En))
            .unwrap_or(Lang::En);
        l
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Cs,
        2 => Lang::De,
        3 => Lang::Es,
        4 => Lang::Fr,
        5 => Lang::Pl,
        _ => Lang::En,
    }
}

pub fn set_lang(l: Lang) {
    LANG.store(l as u8, Ordering::Relaxed);
}

/// English → translation, for one language.
pub fn table_of(l: Lang) -> Vec<(&'static str, &'static str)> {
    let t = &super::i18n_table::T;
    match l {
        Lang::En => Vec::new(),
        Lang::Cs => t.iter().map(|e| (e.0, e.1)).collect(),
        Lang::De => t.iter().map(|e| (e.0, e.2)).collect(),
        Lang::Es => super::i18n_es::T.to_vec(),
        Lang::Fr => super::i18n_fr::T.to_vec(),
        Lang::Pl => super::i18n_pl::T.to_vec(),
    }
}

fn table(l: Lang) -> &'static HashMap<&'static str, &'static str> {
    static T: [OnceLock<HashMap<&'static str, &'static str>>; 6] = [const { OnceLock::new() }; 6];
    T[l as usize].get_or_init(|| table_of(l).into_iter().collect())
}

/// The text in the chosen language (the English text if it has no translation).
pub fn tr(en: &'static str) -> &'static str {
    tr_in(lang(), en)
}

/// The translation of a text known only at run time (a value inside an engine
/// message, e.g. a mode's name), if the table has it.
pub fn tr_dyn(en: &str) -> Option<&'static str> {
    let l = lang();
    if l == Lang::En {
        return None;
    }
    table(l).get(en).copied().filter(|t| !t.is_empty())
}

pub fn tr_in(l: Lang, en: &'static str) -> &'static str {
    if l == Lang::En {
        return en;
    }
    table(l).get(en).copied().filter(|t| !t.is_empty()).unwrap_or(en)
}

/// A text whose English wording is shared by things that other languages
/// name differently ("Light" for the deflicker strength, not the light
/// section): looked up under its own `key`, shown as `en` in English.
pub fn tr_ctx(key: &'static str, en: &'static str) -> &'static str {
    let l = lang();
    if l == Lang::En {
        return en;
    }
    table(l).get(key).copied().filter(|t| !t.is_empty()).unwrap_or_else(|| tr(en))
}

/// Put the values into the `{}` places of a translated text, in order.
pub fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut it = args.iter();
    let mut rest = template;
    while let Some(k) = rest.find("{}") {
        out.push_str(&rest[..k]);
        match it.next() {
            Some(a) => out.push_str(&a.to_string()),
            None => out.push_str("{}"),
        }
        rest = &rest[k + 2..];
    }
    out.push_str(rest);
    out
}

/// `trf!("Added {} frame(s).", n)`: the translated text with the values put in.
#[macro_export]
macro_rules! trf {
    ($s:literal $(, $a:expr)* $(,)?) => {
        $crate::gui::i18n::fill($crate::gui::i18n::tr($s), &[$(&$a as &dyn std::fmt::Display),*])
    };
}

/// Every text in the table, for the tests.
pub fn entries() -> &'static [(&'static str, &'static str, &'static str)] {
    super::i18n_table::T
}

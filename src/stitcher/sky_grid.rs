//! The sky's coordinates on a panorama of the night sky (as Siril's celestial
//! grid): lines of right ascension and declination, and the names of the
//! brightest stars and of the Messier and other bright deep-sky objects.
//!
//! The mapping comes from the photos found on the star catalogue (plate
//! solving): every identified star is a pair of a catalogue direction and the
//! direction where the panorama shows it; one rotation fits them all. It is
//! approximate where the sky turned between the shots (the panorama shows the
//! stars of every photo as at one moment only when it measured the turning).

use super::solve::{direction, kabsch};
use super::M3;
use nalgebra::Vector3;
use std::sync::OnceLock;

type V3 = Vector3<f64>;

/// A named object to circle.
#[derive(Clone, Debug)]
pub struct Label {
    pub star: bool,
    pub name: String,
    pub dir: V3,
    pub mag: f32,
    /// Size (degrees across; 0 for stars).
    pub size: f64,
}

static LABELS: &str = include_str!("../../assets/stars/labels.tsv");

/// The names (bright stars and deep-sky objects, brightest first within each kind).
pub fn labels() -> &'static [Label] {
    static L: OnceLock<Vec<Label>> = OnceLock::new();
    L.get_or_init(|| {
        LABELS
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() < 6 {
                    return None;
                }
                let num = |s: &str| s.trim().parse::<f64>().ok();
                Some(Label { star: f[0] == "s", name: f[1].to_string(), dir: direction(num(f[2])?, num(f[3])?), mag: num(f[4])? as f32, size: num(f[5])? / 60.0 })
            })
            .collect()
    })
}

/// The sky's rotation into the panorama: R with `out ≈ R · sky` from pairs
/// (sky direction, direction in the panorama's view), and the pairs' spread
/// around it (degrees); None with fewer than 3 pairs.
pub fn fit(pairs: &[(V3, V3)]) -> Option<(M3, f64)> {
    if pairs.len() < 3 {
        return None;
    }
    let (sky, out): (Vec<V3>, Vec<V3>) = pairs.iter().map(|(s, o)| (s.normalize(), o.normalize())).unzip();
    let r = kabsch(&out, &sky);
    let rms = (pairs.len() as f64).recip() * sky.iter().zip(&out).map(|(s, o)| (r * s).dot(o).clamp(-1.0, 1.0).acos().powi(2)).sum::<f64>();
    Some((r, rms.sqrt().to_degrees()))
}

/// A step of the grid (degrees) that puts lines at least `min_deg` apart.
pub fn grid_step(min_deg: f64) -> f64 {
    [0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0].into_iter().find(|&s| s >= min_deg).unwrap_or(30.0)
}

/// A right ascension step (degrees, a whole number of time minutes) at least `min_deg`.
pub fn ra_step(min_deg: f64) -> f64 {
    // 1, 2, 5, 10, 20, 30 minutes; 1, 2, 3, 6 hours (15° per hour)
    [0.25, 0.5, 1.25, 2.5, 5.0, 7.5, 15.0, 30.0, 45.0, 90.0].into_iter().find(|&s| s >= min_deg).unwrap_or(90.0)
}

/// Right ascension as hours and minutes ("3h", "3h20m").
pub fn ra_text(ra: f64) -> String {
    let m = (ra.rem_euclid(360.0) / 15.0 * 60.0).round() as i64 % (24 * 60);
    if m % 60 == 0 { format!("{}h", m / 60) } else { format!("{}h{:02}m", m / 60, m % 60) }
}

/// Declination ("+30°", "−12.5°").
pub fn dec_text(dec: f64) -> String {
    let v = (dec * 4.0).round() / 4.0;
    let s = if v.fract() == 0.0 { format!("{:.0}", v.abs()) } else { format!("{}", v.abs()) };
    format!("{}{s}°", if v > 0.0 { "+" } else if v < 0.0 { "−" } else { "" })
}

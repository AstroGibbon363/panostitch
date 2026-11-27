//! The stacking engine's messages (progress, errors, why a frame was left
//! out) come in English, as the command line shows them; the window shows
//! them translated. Each message is matched against the templates below ({}
//! stands for a value); a value that is itself a message is translated too.

use super::i18n::{fill, tr, tr_dyn};

/// The engine's messages, as templates.
const TEMPLATES: &[&str] = &[
    "alignment failed: {}", // tr
    "too few stars", // tr
    "star patterns do not agree", // tr
    // (the longer one first: "only {} stars match" would read "7 distinct" as its value)
    "only {} distinct stars match", // tr
    "only {} stars match", // tr
    "no matching star patterns", // tr
    "implausible alignment (local scale {})", // tr
    "few stars ({}, typical {}) – clouds?", // tr
    "elongated stars ({}, typical {}) – shake / trailing?", // tr
    "dim stars ({} % of the base image) – clouds, haze or dew?", // tr
    "blurred stars (FWHM {} px, typical {})", // tr
    "no gap filling before this frame: {}", // tr
    "No light frames to stack.", // tr
    "No output file.", // tr
    "Select best pixels and Freeze ground need at least 4 light frames.", // tr
    "Freeze ground needs a sky region (boundary line, gradient or irregular mask).", // tr
    "Freeze ground keeps the foreground still, but the sky region is All sky (no foreground): use Select best pixels.", // tr
    "A foreground image needs a sky region with a foreground (boundary line, gradient, irregular mask or automatic).", // tr
    "The foreground image is {}×{}, not the shape of the light frames ({}×{}).", // tr
    "The flat frames do not have the size of the light frames.", // tr
    "The dark / bias frames do not have the size of the light frames.", // tr
    "{} is {}×{}, the base image {}×{}.", // tr
    "{} is {}×{}, not {}×{}.", // tr
    "Cannot size the frame file: {}", // tr
    "Cannot reserve {} of disk space in {}: {}", // tr
    "Cannot map the frame file: {}", // tr
    "Cannot create the frame file {}: {}", // tr
    "Loading the base image {}…", // tr
    "Loading, calibrating, aligning: {}/{}", // tr
    "Loaded {} frames.", // tr
    "Measuring the star motion between frames…", // tr
    "Correcting distortion ({})…", // tr
    "Loading the foreground image…", // tr
    "Writing aligned frame {}/{}", // tr
    "Time-lapse frame {}/{}…", // tr
    "Post-processing…", // tr
    "Saving {}…", // tr
    "Combining: {} %", // tr
    "Master bias: {}/{} {}", // tr
    "Master dark: {}/{} {}", // tr
    "Master flat: {}/{} {}", // tr
    "Master flat dark: {}/{} {}", // tr
];

/// Values of `msg` read with `template` (pieces between the {} in order; the
/// last piece ends the message), or None when it does not fit.
fn read(template: &str, msg: &str) -> Option<Vec<String>> {
    let pieces: Vec<&str> = template.split("{}").collect();
    let mut rest = msg.strip_prefix(pieces[0])?;
    let mut vals = Vec::new();
    for (k, p) in pieces.iter().enumerate().skip(1) {
        let last = k == pieces.len() - 1;
        let at = if last {
            if p.is_empty() { rest.len() } else { rest.strip_suffix(p).map(|r| r.len())? }
        } else {
            rest.find(p).filter(|_| !p.is_empty())?
        };
        vals.push(rest[..at].to_string());
        rest = &rest[at + p.len()..];
    }
    rest.is_empty().then_some(vals)
}

/// An engine message in the window's language (as it came when no template fits).
pub fn tr_msg(msg: &str) -> String {
    for &t in TEMPLATES {
        if let Some(vals) = read(t, msg) {
            let vals: Vec<String> = vals.iter().map(|v| tr_msg(v)).collect();
            let args: Vec<&dyn std::fmt::Display> = vals.iter().map(|v| v as &dyn std::fmt::Display).collect();
            return fill(tr(t), &args);
        }
    }
    // A value of a message (a mode's name such as "Lens") may be in the table on its own.
    tr_dyn(msg).map(str::to_string).unwrap_or_else(|| msg.to_string())
}

#[cfg(test)]
mod tests {
    use super::read;

    #[test]
    fn templates_read_their_values() {
        assert_eq!(read("alignment failed: {}", "alignment failed: only 3 distinct stars match"), Some(vec!["only 3 distinct stars match".to_string()]));
        assert_eq!(read("only {} stars match", "only 7 stars match"), Some(vec!["7".to_string()]));
        assert_eq!(read("{} is {}×{}, not {}×{}.", "a.tif is 10×20, not 30×40."), Some(vec!["a.tif", "10", "20", "30", "40"].into_iter().map(String::from).collect()));
        assert_eq!(read("Combining: {} %", "Combining: 40 %"), Some(vec!["40".to_string()]));
    }
}

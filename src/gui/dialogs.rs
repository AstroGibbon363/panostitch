//! File dialogs with the tools each system already has, so nothing extra needs
//! installing: Linux zenity / kdialog (KDE first) / yad, macOS AppleScript,
//! Windows PowerShell. They run as separate programs on a worker thread, so
//! the window keeps redrawing. Without a tool the path boxes still work.

use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

pub const NO_DIALOG: &str = "No file dialog available (install zenity or kdialog on Linux): type a path into the box instead.";

fn tool() -> Option<&'static str> {
    use crate::io::raw::which;
    if cfg!(windows) {
        return ["powershell", "pwsh"].into_iter().find(|t| which(t).is_some());
    }
    if cfg!(target_os = "macos") {
        return which("osascript").map(|_| "osascript");
    }
    let kde = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase().contains("KDE");
    let order: [&'static str; 3] = if kde { ["kdialog", "zenity", "yad"] } else { ["zenity", "kdialog", "yad"] };
    order.into_iter().find(|t| which(t).is_some())
}

fn run(mut c: Command) -> Option<String> {
    let out = c.output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

fn applescript(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
fn ps(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
const PS_HEAD: &str = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; ";

/// Choose files to open. None = no dialog tool; Some(empty) = cancelled.
pub fn open_files(title: &str, multiple: bool, filter: Option<(&str, Vec<String>)>) -> Option<Vec<String>> {
    let t = tool()?;
    let (name, exts) = filter.unwrap_or(("All files", vec![]));
    let globs: Vec<String> = exts.iter().flat_map(|e| [format!("*{e}"), format!("*{}", e.to_uppercase())]).collect();
    let mut c = Command::new(t);
    match t {
        "zenity" | "yad" => {
            if t == "zenity" {
                c.args(["--file-selection", &format!("--title={title}")]);
            } else {
                c.args(["--file", &format!("--title={title}"), "--width=1000", "--height=650"]);
            }
            if multiple {
                c.args(["--multiple", "--separator=\n"]);
            }
            if !globs.is_empty() {
                c.arg(format!("--file-filter={name} | {}", globs.join(" "))).arg("--file-filter=All files | *");
            }
        }
        "kdialog" => {
            c.args(["--title", title, "--getopenfilename"]).arg(dirs::home_dir().unwrap_or_default());
            if !globs.is_empty() {
                c.arg(format!("{name} ({})", globs.join(" ")));
            }
            if multiple {
                c.args(["--multiple", "--separate-output"]);
            }
        }
        "osascript" => {
            let pick = format!("choose file with prompt {}{}", applescript(title), if multiple { " with multiple selections allowed" } else { "" });
            let lines: Vec<String> = if multiple {
                vec![format!("set fs to ({pick})"), "set out to \"\"".into(), "repeat with f in fs".into(), "set out to out & POSIX path of f & linefeed".into(), "end repeat".into(), "return out".into()]
            } else {
                vec![format!("return POSIX path of ({pick})")]
            };
            for l in lines {
                c.args(["-e", &l]);
            }
        }
        _ => {
            let pats = if exts.is_empty() { "All files|*.*".to_string() } else { format!("{name}|{}|All files|*.*", exts.iter().map(|e| format!("*{e}")).collect::<Vec<_>>().join(";")) };
            let script = format!(
                "{PS_HEAD}$d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = {}; $d.Filter = {}; $d.Multiselect = ${}; if ($d.ShowDialog() -eq 'OK') {{ [Console]::Out.Write(($d.FileNames -join \"`n\")) }}",
                ps(title), ps(&pats), multiple
            );
            c.args(["-NoProfile", "-STA", "-Command", &script]);
        }
    }
    let mut v: Vec<String> = run(c).map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default();
    v.sort();
    Some(v)
}

/// Choose where to save. Outer None = no dialog tool; inner None = cancelled.
pub fn save_file(title: &str, default: &str) -> Option<Option<String>> {
    let t = tool()?;
    let p = std::path::Path::new(default);
    let dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut c = Command::new(t);
    match t {
        "zenity" => {
            c.args(["--file-selection", "--save", &format!("--title={title}"), &format!("--filename={default}")]);
        }
        "yad" => {
            c.args(["--file", "--save", "--confirm-overwrite", &format!("--title={title}"), "--width=1000", "--height=650", &format!("--filename={default}")]);
        }
        "kdialog" => {
            c.args(["--title", title, "--getsavefilename", default]);
        }
        "osascript" => {
            let loc = if std::path::Path::new(&dir).is_dir() { format!(" default location (POSIX file {})", applescript(&dir)) } else { String::new() };
            c.args(["-e", &format!("return POSIX path of (choose file name with prompt {} default name {}{loc})", applescript(title), applescript(&name))]);
        }
        _ => {
            let script = format!(
                "{PS_HEAD}$d = New-Object System.Windows.Forms.SaveFileDialog; $d.Title = {}; $d.FileName = {}; {}if ($d.ShowDialog() -eq 'OK') {{ [Console]::Out.Write($d.FileName) }}",
                ps(title), ps(&name), if std::path::Path::new(&dir).is_dir() { format!("$d.InitialDirectory = {}; ", ps(&dir)) } else { String::new() }
            );
            c.args(["-NoProfile", "-STA", "-Command", &script]);
        }
    }
    Some(run(c))
}

/// What a dialog thread hands back.
pub enum Picked {
    NoTool,
    Files(Vec<String>),
    Save(Option<String>),
}

/// Image files, on a worker thread.
pub fn pick_images() -> Receiver<Picked> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let r = open_files("Add images", true, Some(("Images", crate::io::image_exts())));
        let _ = tx.send(r.map(Picked::Files).unwrap_or(Picked::NoTool));
    });
    rx
}

/// A save location, on a worker thread.
pub fn pick_save(title: &str, default: &str) -> Receiver<Picked> {
    let (tx, rx) = channel();
    let (t, d) = (title.to_string(), default.to_string());
    std::thread::spawn(move || {
        let r = save_file(&t, &d);
        let _ = tx.send(r.map(Picked::Save).unwrap_or(Picked::NoTool));
    });
    rx
}

/// Files under a folder (images only, sorted), or the path itself.
pub fn expand_paths(s: &str) -> Vec<String> {
    let s = s.trim();
    let s = if let Some(rest) = s.strip_prefix("~/") { dirs::home_dir().map(|h| h.join(rest).to_string_lossy().to_string()).unwrap_or_else(|| s.to_string()) } else { s.to_string() };
    let p = std::path::Path::new(&s);
    if !p.is_dir() {
        return vec![s];
    }
    let mut v: Vec<String> = std::fs::read_dir(p)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path().to_string_lossy().to_string())
                .filter(|f| crate::io::is_image(f) && !crate::io::basename(f).to_lowercase().starts_with("panorama"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

//! Star stacking from the command line (the same engine as the "Star stacking" tab).
//!
//!   panostack [options] STAR_IMAGES_OR_FOLDER...
//!
//! Run `panostack --help` for the options. The frame report is printed at the end.

use panostitch::io;
use panostitch::stacker as sk;
use std::process::exit;

const HELP: &str = "Star stacking from the command line (the same engine as the window's \"Star stacking\" tab).

  panostack [options] STAR_IMAGES_OR_FOLDER...

  -o FILE             output: .tif (16-bit), .jpg, .fits / .xisf (32-bit float, linear);
                      default stacked.tif next to the first star image
  --base FILE         base image (default: the middle one)
  --darks X           noise images: a folder or comma-separated files
  --bias X, --flats X, --flatdarks X   the other calibration frames, the same way
  --mode M            accumulation | best (default) | freeze | alignonly | trails | trails-motion
  --fade F            star trails: in | out | both (fade the trails)
  --fill-gaps         star trails: fill the gaps between frames along the star motion
  --horizon V         sky region: horizontal line at V (0 = top, 1 = bottom) …
  --gradient G        … with a transition zone of G (fraction of the height)
  --auto-sky          sky region: detected automatically in the base image
  --foreground FILE   the ground from this image (same framing) instead of the stack; needs a sky region
  --fg-ev EV          … brighter / darker by EV (after matching it to the stacked ground)
  --fg-no-match       … without matching its brightness
  --sigma LO,HI       sigma clipping (default 3,3)
  --half              decode RAW files at half size (faster, less memory)
  --raw-decoder D     built-in (default) | libraw | rawtherapee | darktable | dcraw | auto
  --wb W              white balance of RAW files: camera (default) | auto | daylight
  --no-reject         keep every frame (no automatic rejection)
  --distortion D      lens | complex | auto (distortion correction)
  --lp F[:S]          light pollution: deep[:strength] or uneven[:strength] (strength 0–100, default 50)
  --enhance S         enhance star light with strength S (0–100)
  --timelapse N       stack every N frames into a numbered sequence (folder <output>_timelapse)
  --bgx [DEG]         background extraction (polynomial degree, default 2)
  --colorcal          basic colour calibration (neutral sky, white stars)
  --color-noise S     reduce colour noise with strength S (5–100)
  --stretch           automatic histogram stretch instead of the sRGB curve
  --hdr, --no-auto-brightness, --linear (no tone curve), --adobe, --merge4, --dynamic-noise
  --preview           quick preview: up to 10 frames at low resolution, nothing saved
  --quiet             no progress messages
";

fn files_of(x: &str) -> Vec<String> {
    let p = std::path::Path::new(x);
    if p.is_dir() {
        let mut v: Vec<String> = std::fs::read_dir(p)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().to_string()).filter(|f| io::is_image(f)).collect())
            .unwrap_or_default();
        v.sort();
        return v;
    }
    x.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
}

fn abs(p: &str) -> String {
    std::path::absolute(p).map(|q| q.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string())
}

fn fail(msg: String) -> ! {
    eprintln!("panostack: {msg}");
    exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        exit(if args.is_empty() { 1 } else { 0 });
    }
    let mut cfg = sk::StackConfig::default();
    let mut stars: Vec<String> = Vec::new();
    let mut sky = sk::Sky { mode: sk::SkyMode::None, line: None, gradient: 0.0, mask: panostitch::image::Bits::new(0, 0), feather: 0.01 };
    let mut output: Option<String> = None;
    let mut preview = false;
    let mut quiet = false;
    let mut i = 0;
    let next = |i: &mut usize| -> String {
        *i += 1;
        if *i >= args.len() {
            fail(format!("{} needs a value", args[*i - 1]));
        }
        args[*i].clone()
    };
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-o" => output = Some(next(&mut i)),
            "--base" => cfg.base = Some(abs(&next(&mut i))),
            "--darks" => cfg.darks = files_of(&next(&mut i)).iter().map(|p| abs(p)).collect(),
            "--bias" => cfg.bias = files_of(&next(&mut i)).iter().map(|p| abs(p)).collect(),
            "--flats" => cfg.flats = files_of(&next(&mut i)).iter().map(|p| abs(p)).collect(),
            "--flatdarks" => cfg.flatdarks = files_of(&next(&mut i)).iter().map(|p| abs(p)).collect(),
            "--mode" => {
                let m = next(&mut i);
                match m.as_str() {
                    "trails" | "trails-motion" => {
                        cfg.composition = sk::Composition::StarTrails;
                        cfg.motion_effect = m == "trails-motion";
                    }
                    "accumulation" => cfg.align_mode = sk::AlignMode::Accumulation,
                    "best" => cfg.align_mode = sk::AlignMode::Best,
                    "freeze" => cfg.align_mode = sk::AlignMode::Freeze,
                    "alignonly" => cfg.align_mode = sk::AlignMode::AlignOnly,
                    _ => fail(format!("unknown mode {m}")),
                }
            }
            "--fade" => {
                let f = next(&mut i);
                cfg.trails.0 = match f.as_str() {
                    "in" => sk::Fade::In,
                    "out" => sk::Fade::Out,
                    "both" => sk::Fade::Both,
                    _ => fail(format!("unknown fade {f}")),
                };
            }
            "--fill-gaps" => cfg.trails.1 = true,
            "--auto-sky" => sky.mode = sk::SkyMode::Automatic,
            "--foreground" => cfg.foreground.path = abs(&next(&mut i)),
            "--fg-ev" => cfg.foreground.ev = next(&mut i).parse().unwrap_or_else(|_| fail("--fg-ev needs a number".into())),
            "--fg-no-match" => cfg.foreground.match_brightness = false,
            "--horizon" => {
                let v: f64 = next(&mut i).parse().unwrap_or_else(|_| fail("--horizon needs a number".into()));
                sky.mode = if sky.gradient > 0.0 { sk::SkyMode::Gradient } else { sk::SkyMode::BoundaryLine };
                sky.line = Some((0.0, v, 1.0, v));
            }
            "--gradient" => {
                let g: f64 = next(&mut i).parse().unwrap_or_else(|_| fail("--gradient needs a number".into()));
                sky.gradient = g;
                sky.mode = if sky.line.is_none() { sk::SkyMode::None } else { sk::SkyMode::Gradient };
            }
            "--sigma" => {
                let s = next(&mut i);
                let v: Vec<f64> = s.split(',').map(|x| x.parse().unwrap_or_else(|_| fail("--sigma needs LO,HI".into()))).collect();
                if v.len() != 2 {
                    fail("--sigma needs LO,HI".into());
                }
                cfg.sigma = sk::Sigma { low: v[0], high: v[1], iters: 3 };
            }
            "--half" => cfg.raw.half = true,
            "--raw-decoder" => {
                let d = next(&mut i);
                cfg.raw.decoder = io::RawDecoder::from_name(&d).unwrap_or_else(|| fail(format!("unknown RAW decoder {d}")));
            }
            "--wb" => {
                let w = next(&mut i);
                cfg.raw.wb = io::WhiteBalance::from_name(&w).unwrap_or_else(|| fail(format!("unknown white balance {w}")));
            }
            "--no-reject" => cfg.reject.auto = false,
            "--distortion" => {
                let d = next(&mut i);
                let d = format!("{}{}", d[..1].to_uppercase(), &d[1..]);
                cfg.distortion = sk::Distortion::from_name(&d).unwrap_or_else(|| fail(format!("unknown distortion correction {d}")));
            }
            "--lp" => {
                let s = next(&mut i);
                let f: Vec<&str> = s.split(':').collect();
                cfg.light_pollution.filter = match f[0] {
                    "deep" => sk::Pollution::DeepSky,
                    "uneven" => sk::Pollution::Uneven,
                    _ => fail("--lp needs deep or uneven".into()),
                };
                cfg.light_pollution.strength = f.get(1).map(|v| v.parse().unwrap_or(50.0)).unwrap_or(50.0);
            }
            "--enhance" => cfg.enhance_stars = (true, next(&mut i).parse().unwrap_or_else(|_| fail("--enhance needs a number".into()))),
            "--timelapse" => cfg.timelapse = (true, next(&mut i).parse().unwrap_or_else(|_| fail("--timelapse needs a number".into()))),
            "--adobe" => cfg.color_space = sk::ColorSpace::AdobeRgb,
            "--bgx" => {
                let deg = if i + 1 < args.len() && args[i + 1].chars().all(|c| c.is_ascii_digit()) { next(&mut i).parse().unwrap_or(2) } else { 2 };
                cfg.bg_extraction = (true, deg);
            }
            "--colorcal" => cfg.color_calibration = true,
            "--color-noise" => cfg.color_noise = (true, next(&mut i).parse().unwrap_or_else(|_| fail("--color-noise needs a number".into()))),
            "--stretch" => cfg.tone = sk::Tone::Stretch,
            "--hdr" => cfg.hdr = true,
            "--no-auto-brightness" => cfg.auto_brightness = false,
            "--linear" => cfg.tone = sk::Tone::Linear,
            "--merge4" => cfg.merge4 = true,
            "--dynamic-noise" => cfg.dynamic_noise = true,
            "--preview" => preview = true,
            "--quiet" => quiet = true,
            _ if a.starts_with('-') => fail(format!("Unknown option {a} (see panostack --help)")),
            _ => stars.extend(files_of(a).iter().map(|p| abs(p))),
        }
        i += 1;
    }
    if stars.is_empty() {
        fail("No star images given.".into());
    }
    cfg.output = output.unwrap_or_else(|| {
        std::path::Path::new(&stars[0]).parent().map(|d| d.join("stacked.tif").to_string_lossy().to_string()).unwrap_or_else(|| "stacked.tif".into())
    });
    cfg.stars = stars;
    cfg.sky = sky;
    let last = std::sync::Mutex::new(String::new());
    let progress = |m: &str| -> Result<(), String> {
        if !quiet {
            let mut l = last.lock().unwrap();
            if *l != m {
                *l = m.to_string();
                println!("{m}");
            }
        }
        Ok(())
    };
    match sk::stack(&cfg, preview, &progress, None) {
        Ok(r) => {
            print!("{}", sk::report_text(&r.report));
            println!("\nProcess log:");
            for l in &r.log {
                println!("  {l}");
            }
            if r.saved.is_empty() {
                println!("Nothing saved (quick preview).");
            } else {
                println!("Saved: {}", r.saved.join(", "));
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            exit(1);
        }
    }
}

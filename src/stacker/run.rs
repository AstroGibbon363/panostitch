//! The whole run (`stack`) and the frame report.

use super::*;
use crate::image::{median_f64, Bits, Gray, Img};
use crate::io::{basename, is_astro, save_panorama};
use std::path::Path;
use std::time::Instant;

/// Everything a run returns.
#[derive(Clone)]
pub struct StackResult {
    pub image: Img,
    pub linear: Img,
    pub report: Vec<FrameInfo>,
    pub gain: f64,
    pub saved: Vec<String>,
    pub base: String,
    pub background: Option<Img>,
    pub stretch: Option<(f64, f64, f64)>,
    pub raw: Img,
    pub weight: Gray,
    pub fwhm: f64,
    pub log: Vec<String>,
    pub mask: Option<Bits>,
    pub preview: bool,
}

pub fn mode_of(cfg: &StackConfig) -> Mode {
    if cfg.composition == Composition::StarTrails {
        return Mode::Trails;
    }
    match cfg.align_mode {
        AlignMode::Accumulation => Mode::Accumulation,
        AlignMode::Best => Mode::Best,
        AlignMode::Freeze => Mode::Freeze,
        AlignMode::AlignOnly => Mode::AlignOnly,
    }
}

pub fn medfin(v: impl IntoIterator<Item = f64>) -> f64 {
    let v: Vec<f64> = v.into_iter().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        f64::NAN
    } else {
        median_f64(v)
    }
}

/// Mark frames that are much worse than the typical one (needs `reject.auto`).
pub fn reject(infos: &mut [FrameInfo], cfg: &StackConfig, aligned: bool) {
    for fi in infos.iter_mut() {
        if fi.status != Status::Pending {
            continue;
        }
        fi.status = if aligned && !fi.err.is_finite() { Status::Failed } else { Status::Used };
    }
    if !cfg.reject.auto {
        return;
    }
    let ok: Vec<usize> = (0..infos.len()).filter(|&i| infos[i].used()).collect();
    if ok.len() < 3 {
        return;
    }
    let ms = median_f64(ok.iter().map(|&i| infos[i].stars as f64));
    let mf = medfin(ok.iter().map(|&i| infos[i].fwhm));
    let me = medfin(ok.iter().map(|&i| infos[i].elong));
    let r = &cfg.reject;
    for &i in &ok {
        let fi = &mut infos[i];
        if fi.status == Status::Base {
            continue;
        }
        let why = if fi.light < r.min_light {
            format!("dim stars ({} % of the base image) – clouds, haze or dew?", (100.0 * fi.light).round() as i64)
        } else if (fi.stars as f64) < r.min_stars * ms {
            format!("few stars ({}, typical {}) – clouds?", fi.stars, ms.round() as i64)
        } else if fi.fwhm > r.max_fwhm * mf {
            format!("blurred stars (FWHM {:.1} px, typical {:.1})", fi.fwhm, mf)
        } else if fi.elong > r.max_elong * me {
            format!("elongated stars ({:.2}, typical {:.2}) – shake / trailing?", fi.elong, me)
        } else {
            String::new()
        };
        if !why.is_empty() {
            fi.status = Status::Rejected;
            fi.reason = why;
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { one.to_string() } else { many.to_string() }
}

/// Stack the star images described by `cfg`. `preview` makes a quick, small
/// version from up to PREVIEW_FRAMES images and saves nothing. `progress` is
/// called along the way and may return an error to cancel; `loader` replaces
/// reading files (tests).
pub fn stack(cfg: &StackConfig, preview: bool, progress: Progress, loader: Option<Loader>) -> Result<StackResult, String> {
    let mut cfg = cfg.clone();
    let mut log: Vec<String> = Vec::new();
    let t_start = Instant::now();
    let mut t_step = Instant::now();
    let mut note = |log: &mut Vec<String>, msg: String| {
        log.push(format!("{:>6.1} s  {msg}", t_step.elapsed().as_secs_f64()));
        t_step = Instant::now();
    };
    let loadcfg = cfg.clone();
    let default_loader = |p: &str| load_linear(p, &loadcfg, preview);
    let load: Loader = match loader {
        Some(l) => l,
        None => &default_loader,
    };
    let mut stars: Vec<String> = cfg.stars.clone();
    stars.sort();
    stars.dedup();
    let mut infos: Vec<FrameInfo> =
        stars.iter().map(|p| FrameInfo::new(p, if cfg.excluded.contains(p) { Status::Excluded } else { Status::Pending })).collect();
    let mut cand: Vec<usize> = (0..infos.len()).filter(|&i| infos[i].status == Status::Pending).collect();
    if cand.is_empty() {
        return Err("No star images to stack.".into());
    }
    let mut bi = cfg.base.as_ref().and_then(|b| cand.iter().position(|&i| &infos[i].path == b)).unwrap_or(cand.len().div_ceil(2) - 1);
    if preview && cand.len() > PREVIEW_FRAMES {
        let mut keep: Vec<usize> = (0..PREVIEW_FRAMES).map(|t| ((t as f64) * (cand.len() - 1) as f64 / (PREVIEW_FRAMES - 1) as f64).round() as usize).collect();
        keep.push(bi);
        keep.sort();
        keep.dedup();
        for (k, &i) in cand.iter().enumerate() {
            if !keep.contains(&k) {
                infos[i].status = Status::Skipped;
                infos[i].reason = "not in the quick preview".into();
            }
        }
        let newcand: Vec<usize> = keep.iter().map(|&k| cand[k]).collect();
        bi = keep.iter().position(|&k| k == bi).unwrap();
        cand = newcand;
    }
    let base_i = cand[bi];
    let mode = mode_of(&cfg);
    let aligned = mode != Mode::Trails;
    if matches!(mode, Mode::Best | Mode::Freeze) && cand.len() < 4 {
        return Err("Select best pixels and Freeze ground need at least 4 star images.".into());
    }
    if mode == Mode::Freeze && cfg.sky.mode == SkyMode::None {
        return Err("Freeze ground needs a sky region (boundary line, gradient or irregular mask).".into());
    }
    let usefg = !cfg.foreground.path.is_empty();
    if usefg && cfg.sky.mode == SkyMode::None {
        return Err("A foreground image needs a sky region (boundary line, gradient, irregular mask or automatic).".into());
    }
    let gapfill = mode == Mode::Trails && cfg.trails.1;
    let dir: std::path::PathBuf = if cfg.output.is_empty() {
        std::env::temp_dir()
    } else {
        std::path::absolute(&cfg.output).map(|p| p.parent().map(|d| d.to_path_buf()).unwrap_or_default()).unwrap_or_else(|_| std::env::temp_dir())
    };
    let pick = |ps: &[String]| -> Vec<String> { if preview { ps.iter().take(PREVIEW_CALIB).cloned().collect() } else { ps.to_vec() } };
    let calcfg = StackConfig { darks: pick(&cfg.darks), bias: pick(&cfg.bias), flats: pick(&cfg.flats), flatdarks: pick(&cfg.flatdarks), ..cfg.clone() };
    let cal = calibration(&calcfg, load, &dir, progress)?;
    let ncal = [(calcfg.darks.len(), "noise"), (calcfg.bias.len(), "bias"), (calcfg.flats.len(), "vignetting"), (calcfg.flatdarks.len(), "flat-dark")];
    note(
        &mut log,
        format!(
            "Calibration: {}",
            if ncal.iter().all(|c| c.0 == 0) {
                "none".to_string()
            } else {
                ncal.iter().filter(|c| c.0 > 0).map(|c| format!("master {} of {} frames", c.1, c.0)).collect::<Vec<_>>().join(", ")
                    + &if cfg.darks.is_empty() { String::new() } else { format!("; {} hot pixels found in the noise images", cal.hot.len()) }
            }
        ),
    );

    // Load, calibrate and measure every frame (the base first: it fixes the size).
    let prepare = |fi: &FrameInfo| -> Result<Img, String> {
        let mut img = calibrate(load(&fi.path)?, &cal)?;
        if cfg.dynamic_noise {
            let hot = dynamic_hot_pixels(&img);
            fix_pixels(&mut img, &hot);
        }
        if cfg.merge4 {
            img = merge4(&img);
        }
        Ok(img)
    };
    progress(&format!("Loading the base image {}…", basename(&infos[base_i].path)))?;
    let img0 = prepare(&infos[base_i])?;
    let (h, w) = (img0.h, img0.w);
    if cfg.sky.mode == SkyMode::Automatic {
        let (mask, _) = detect_sky(&img0, 400, None, 0.01);
        let pct = (100.0 * mask.count() as f64 / mask.len().max(1) as f64).round() as i64;
        cfg.sky.mode = SkyMode::IrregularMask;
        cfg.sky.mask = mask;
        note(&mut log, format!("Sky detected automatically in the base image: {pct} % of the frame"));
    }
    let wt = sky_weight(&cfg.sky, h, w);
    if cfg.sky.mode != SkyMode::None {
        let pct = (100.0 * wt.px.iter().filter(|v| **v >= 0.5).count() as f64 / wt.len() as f64).round() as i64;
        note(&mut log, format!("Sky region ({}): {pct} % of the frame", cfg.sky.mode.name()));
    }
    let store = FrameStore::new(h, w, cand.len(), &dir)?;
    let measure = |fi: &mut FrameInfo, k: usize, img: Img| -> Result<(), String> {
        if (img.h, img.w) != (h, w) {
            return Err(format!("{} is {}×{}, the base image {}×{}.", basename(&fi.path), img.w, img.h, w, h));
        }
        fi.detected = detect_stars(&img, &wt);
        let good: Vec<&Star> = fi.detected.iter().filter(|s| s.peak < 0.9).take(30).collect(); // bright, unsaturated
        fi.stars = fi.detected.len();
        fi.fwhm = if good.is_empty() { f64::NAN } else { median_f64(good.iter().map(|s| s.fwhm)) };
        fi.elong = if good.is_empty() { f64::NAN } else { median_f64(good.iter().map(|s| s.elong)) };
        let step = img.step_for(100_000);
        let mut idx: Vec<usize> = (0..img.len()).step_by(step).filter(|&i| wt.px[i] >= 0.5).collect();
        if idx.is_empty() {
            idx = (0..img.len()).step_by(step).collect();
        }
        fi.level = [
            crate::image::median_of(idx.iter().map(|&i| img.px[i][0])),
            crate::image::median_of(idx.iter().map(|&i| img.px[i][1])),
            crate::image::median_of(idx.iter().map(|&i| img.px[i][2])),
        ];
        fi.background = crate::image::luminance(fi.level) as f64;
        fi.light = fi.detected.iter().take(20).map(|s| s.flux).sum(); // made relative below
        store.store_shared(k, &img);
        Ok(())
    };
    measure(&mut infos[base_i], bi, img0)?;
    infos[base_i].status = Status::Base;
    let base_stars = infos[base_i].detected.clone();
    let done = std::sync::atomic::AtomicUsize::new(1);
    let ncand = cand.len();
    // Load several frames at once (RAW decoding is the slow part), a few at a time to bound the memory.
    let nthreads = rayon::current_num_threads().div_ceil(2).clamp(1, 4);
    let results: Vec<(usize, FrameInfo)> = {
        let jobs: Vec<(usize, usize)> = cand.iter().enumerate().filter(|(k, _)| *k != bi).map(|(k, &i)| (k, i)).collect();
        let pool = rayon::ThreadPoolBuilder::new().num_threads(nthreads).build().map_err(|e| e.to_string())?;
        pool.install(|| {
            use rayon::prelude::*;
            jobs.par_iter()
                .map(|&(k, i)| {
                    let mut fi = infos[i].clone();
                    let res = prepare(&fi).and_then(|img| measure(&mut fi, k, img));
                    match res {
                        Ok(()) => {
                            if aligned {
                                match align_stars(&base_stars, &fi.detected, w, cfg.model) {
                                    Ok((t, m, e)) => {
                                        fi.t = t;
                                        fi.matched = m;
                                        fi.err = e;
                                        fi.warp = Warp::new(t);
                                    }
                                    Err(e) => fi.reason = format!("alignment failed: {e}"),
                                }
                            }
                        }
                        Err(e) => {
                            fi.status = Status::Failed;
                            fi.reason = e;
                        }
                    }
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let _ = progress(&format!("Loading, calibrating, aligning: {n}/{ncand}"));
                    (i, fi)
                })
                .collect()
        })
    };
    for (i, fi) in results {
        infos[i] = fi;
    }
    progress(&format!("Loaded {ncand} frames."))?; // may cancel
    {
        let b = &mut infos[base_i];
        b.matched = b.stars;
        b.err = 0.0;
    }
    let bl = infos[base_i].light;
    for &i in &cand {
        infos[i].light = if bl > 0.0 { infos[i].light / bl } else { f64::NAN };
    }
    let (bstars, bfwhm) = (infos[base_i].stars, infos[base_i].fwhm);
    note(
        &mut log,
        format!(
            "Loaded, calibrated and measured {ncand} frames{}{}{}; {h}×{w} px; base: {bstars} stars, FWHM {bfwhm:.1} px",
            if aligned { format!(", aligned to {}", basename(&infos[base_i].path)) } else { String::new() },
            if cfg.dynamic_noise { ", dynamic hot pixels replaced" } else { "" },
            if cfg.merge4 { ", 2×2 pixels merged" } else { "" }
        ),
    );
    {
        let mut sub: Vec<FrameInfo> = cand.iter().map(|&i| infos[i].clone()).collect();
        reject(&mut sub, &cfg, aligned);
        for (k, &i) in cand.iter().enumerate() {
            infos[i] = sub[k].clone();
        }
    }
    // The frames in use: (index in cand = store slot, index in infos).
    let frames_idx: Vec<(usize, usize)> = cand.iter().enumerate().filter(|(_, &i)| infos[i].used()).map(|(k, &i)| (k, i)).collect();
    let nrej = cand.iter().filter(|&&i| matches!(infos[i].status, Status::Rejected | Status::Failed)).count();
    let nexcl = infos.iter().filter(|fi| fi.status == Status::Excluded).count();
    note(
        &mut log,
        format!(
            "Frames used: {} of {}{}{}{}",
            frames_idx.len(),
            infos.len(),
            if nrej > 0 { format!("; {nrej} left out (reasons in the table)") } else { String::new() },
            if nexcl > 0 { format!("; {nexcl} excluded by you") } else { String::new() },
            if cfg.reject.auto { "" } else { "; automatic rejection off" }
        ),
    );
    let mut gaps: Option<Vec<Option<M3>>> = None;
    let mut steps = 0usize;
    if gapfill && frames_idx.len() > 1 {
        // Star motion from each frame to the next: the gaps are filled along it.
        progress("Measuring the star motion between frames…")?;
        let nf = frames_idx.len();
        let det: Vec<Vec<Star>> = frames_idx.iter().map(|&(_, i)| infos[i].detected.clone()).collect();
        let res: Vec<Result<(M3, usize, f64), String>> = {
            use rayon::prelude::*;
            (0..nf - 1).into_par_iter().map(|k| align_stars(&det[k], &det[k + 1], w, cfg.model)).collect()
        };
        let mut g = vec![None; nf];
        for (k, r) in res.into_iter().enumerate() {
            let bi_ = frames_idx[k + 1].1;
            match r {
                Ok((m, n, e)) => {
                    g[k] = Some(m);
                    infos[bi_].matched = n;
                    infos[bi_].err = e;
                }
                Err(e) => infos[bi_].reason = format!("no gap filling before this frame: {e}"),
            }
        }
        let mut dmax: f64 = 0.0;
        for m in g.iter().flatten() {
            for (x, y) in [(1.0, 1.0), (w as f64, 1.0), (1.0, h as f64), (w as f64, h as f64), (w as f64 / 2.0, h as f64 / 2.0)] {
                let (px, py) = apply_h(m, x, y);
                dmax = dmax.max((px - x).hypot(py - y));
            }
        }
        steps = ((dmax / GAP_SPACING).ceil() as i64 - 1).clamp(0, GAP_MAX_STEPS as i64) as usize;
        let measured = g.iter().filter(|m| m.is_some()).count();
        note(
            &mut log,
            format!("Gap filling: star motion up to {dmax:.1} px per frame, {steps} shifted {} per gap; {measured} of {} gaps measured", plural(steps, "copy", "copies"), nf - 1),
        );
        gaps = Some(g);
    }
    if cfg.unify_exposure {
        let bl = infos[base_i].level;
        for &(_, i) in &frames_idx {
            let l = infos[i].level;
            infos[i].gain = [
                if l[0] > 0.0 { bl[0] / l[0] } else { 1.0 },
                if l[1] > 0.0 { bl[1] / l[1] } else { 1.0 },
                if l[2] > 0.0 { bl[2] / l[2] } else { 1.0 },
            ];
        }
    }
    let mut kind = cfg.distortion;
    if aligned && kind != Distortion::Off && frames_idx.len() > 1 {
        if kind == Distortion::Auto {
            // a correction grid needs plenty of stars everywhere
            let med = median_f64(frames_idx.iter().filter(|&&(_, i)| i != base_i).map(|&(_, i)| infos[i].matched as f64));
            kind = if med >= 80.0 { Distortion::Complex } else { Distortion::Lens };
        }
        progress(&format!("Correcting distortion ({})…", kind.name().to_lowercase()))?;
        let mut sub: Vec<FrameInfo> = frames_idx.iter().map(|&(_, i)| infos[i].clone()).collect();
        let bpos = frames_idx.iter().position(|&(_, i)| i == base_i).unwrap_or(0);
        let k1 = correct_distortion(&mut sub, bpos, kind == Distortion::Complex, h, w);
        for (t, &(_, i)) in frames_idx.iter().enumerate() {
            infos[i] = sub[t].clone();
        }
        let mederr = medfin(frames_idx.iter().filter(|&&(_, i)| i != base_i).map(|&(_, i)| infos[i].err));
        note(
            &mut log,
            format!(
                "Distortion correction ({}): radial k1 = {k1:.4}{}; median alignment error {mederr:.2} px",
                kind.name().to_lowercase(),
                if kind == Distortion::Complex { ", plus a correction grid per frame" } else { "" }
            ),
        );
    }
    let mut fg: Option<Img> = None;
    if usefg {
        progress("Loading the foreground image…")?;
        fg = Some(load_foreground(&cfg, load, h, w)?);
    }
    let frames: Vec<&FrameInfo> = frames_idx.iter().map(|&(_, i)| &infos[i]).collect();
    let ks: Vec<usize> = frames_idx.iter().map(|&(k, _)| k).collect();
    let mut saved: Vec<String> = Vec::new();
    let stacked: Img;
    let base_fwhm = infos[base_i].fwhm;
    if mode == Mode::AlignOnly {
        if !preview {
            let outdir = dir.join("aligned");
            std::fs::create_dir_all(&outdir).map_err(|e| e.to_string())?;
            let ext = if is_astro(&cfg.output) { format!(".{}", crate::io::ext_of(&cfg.output)) } else { ".tif".to_string() };
            for (n, (fi, &k)) in frames.iter().zip(ks.iter()).enumerate() {
                progress(&format!("Writing aligned frame {}/{}", n + 1, frames.len()))?;
                let stem = Path::new(&fi.path).file_stem().and_then(|s| s.to_str()).unwrap_or("frame").to_string();
                let p = outdir.join(format!("{stem}{ext}")).to_string_lossy().to_string();
                let g = fi.gain;
                let mut img = resample(&store, k, &fi.warp, cfg.interpolation == Interpolation::Lanczos);
                img.map_inplace(move |c| [c[0] * g[0], c[1] * g[1], c[2] * g[2]]);
                save_panorama(&p, &img)?;
                saved.push(p);
            }
        }
        stacked = resample(&store, bi, &infos[base_i].warp, true);
    } else if cfg.timelapse.0 {
        // Every N consecutive frames → one output frame, aligned to its middle frame
        // (so the sky moves through the sequence); the brightness of the first for all.
        let n = cfg.timelapse.1.clamp(1, frames.len());
        let nout = if preview { 1 } else { frames.len() - n + 1 };
        let outp = Path::new(&cfg.output);
        let stem = outp.file_stem().and_then(|s| s.to_str()).unwrap_or("stacked").to_string();
        let ext = outp.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_else(|| ".tif".into());
        let outdir = dir.join(format!("{stem}_timelapse"));
        if !preview {
            std::fs::create_dir_all(&outdir).map_err(|e| e.to_string())?;
        }
        let mut gain0: Option<f64> = None;
        let mut first: Option<Img> = None;
        for t in 0..nout {
            let win: Vec<usize> = (t..t + n).collect();
            progress(&format!("Time-lapse frame {}/{nout}…", t + 1))?;
            let ref_ = if mode == Mode::Trails { None } else { Some(&frames[win[n.div_ceil(2) - 1]].warp) };
            let g: Option<Vec<Option<M3>>> = gaps.as_ref().map(|gp| win.iter().map(|&k| if k < *win.last().unwrap() { gp[k].clone() } else { None }).collect());
            let wks: Vec<usize> = win.iter().map(|&k| ks[k]).collect();
            let wfr: Vec<&FrameInfo> = win.iter().map(|&k| frames[k]).collect();
            let mut raw_ = combine(&store, &wks, &wfr, &wt, &cfg, mode, progress, ref_, g.as_deref(), steps);
            if let Some(f) = &fg {
                raw_ = blend_foreground(&raw_, f, &wt, &cfg.foreground).0;
            }
            let pp = postprocess(&raw_, &wt, &cfg, base_fwhm, gain0);
            if gain0.is_none() {
                gain0 = Some(pp.gain);
                first = Some(raw_);
            }
            if !preview {
                let p = outdir.join(format!("{:04}{ext}", t + 1)).to_string_lossy().to_string();
                save_panorama(&p, if is_astro(&p) { &pp.linear } else { &pp.image })?;
                saved.push(p);
            }
        }
        stacked = first.unwrap();
    } else {
        stacked = combine(&store, &ks, &frames, &wt, &cfg, mode, progress, None, gaps.as_deref(), steps);
    }
    note(
        &mut log,
        format!(
            "Combined: {}{}{}",
            match mode {
                Mode::Accumulation => "average".to_string(),
                Mode::Best => format!("sigma-clipped average ({} / {} σ, {}×)", cfg.sigma.low, cfg.sigma.high, cfg.sigma.iters),
                Mode::Freeze => "freeze ground (sky aligned, foreground in place, sigma-clipped)".to_string(),
                Mode::Trails => format!(
                    "star trails (brightest value in the sky{})",
                    if fade_kind(&cfg) == Fade::Off { String::new() } else { format!(", {}", fade_kind(&cfg).name().to_lowercase()) }
                ),
                Mode::AlignOnly => "aligned frames written".to_string(),
            },
            if cfg.timelapse.0 && mode != Mode::AlignOnly { format!("; time-lapse of every {} frames", cfg.timelapse.1) } else { String::new() },
            if cfg.unify_exposure { "; exposure unified" } else { "" }
        ),
    );
    let mut stacked = stacked;
    if let (Some(f), false, false) = (&fg, cfg.timelapse.0, mode == Mode::AlignOnly) {
        let (s, kfg) = blend_foreground(&stacked, f, &wt, &cfg.foreground);
        stacked = s;
        note(
            &mut log,
            format!(
                "Foreground from {}, brightness ×{:.3}{}",
                basename(&cfg.foreground.path),
                kfg,
                if cfg.foreground.match_brightness {
                    format!(" (matched to the stacked ground{})", if cfg.foreground.ev == 0.0 { String::new() } else { format!(", {} EV", cfg.foreground.ev) })
                } else {
                    String::new()
                }
            ),
        );
    }
    progress("Post-processing…")?;
    let pp = postprocess(&stacked, &wt, &cfg, base_fwhm, None);
    note(&mut log, format!("Post-processing: {}; brightness ×{:.3}", postprocess_summary(&cfg), pp.gain));
    if !preview && mode != Mode::AlignOnly && !cfg.timelapse.0 {
        if cfg.output.is_empty() {
            return Err("No output file.".into());
        }
        progress(&format!("Saving {}…", basename(&cfg.output)))?;
        save_panorama(&cfg.output, if is_astro(&cfg.output) { &pp.linear } else { &pp.image })?;
        saved.push(cfg.output.clone());
    }
    if !saved.is_empty() {
        let msg = if saved.len() == 1 {
            saved[0].clone()
        } else {
            format!("{} files in {}", saved.len(), Path::new(&saved[0]).parent().map(|p| p.display().to_string()).unwrap_or_default())
        };
        note(&mut log, format!("Saved {msg}"));
    }
    log.push(format!("{:>6.1} s  in total", t_start.elapsed().as_secs_f64()));
    let mask = if cfg.sky.mode == SkyMode::IrregularMask { Some(cfg.sky.mask.clone()) } else { None };
    Ok(StackResult {
        image: pp.image,
        linear: pp.linear,
        base: infos[base_i].path.clone(),
        report: infos,
        gain: pp.gain,
        saved,
        background: pp.background,
        stretch: pp.stretch,
        raw: stacked,
        weight: wt,
        fwhm: base_fwhm,
        log,
        mask,
        preview,
    })
}

/// The frame report as a text table.
pub fn report_text(infos: &[FrameInfo]) -> String {
    let f = |v: f64, d: usize| if v.is_finite() { format!("{:.*}", d, v) } else { "–".to_string() };
    let mut s = format!("{:<26}{:>6}{:>7}{:>7}{:>8}{:>7}{:>8}  status\n", "frame", "stars", "FWHM", "elong", "sky", "light", "align");
    for fi in infos {
        let name: String = basename(&fi.path).chars().take(25).collect();
        s += &format!(
            "{:<26}{:>6}{:>7}{:>7}{:>8}{:>7}{:>8}  {}{}\n",
            name,
            fi.stars,
            f(fi.fwhm, 1),
            f(fi.elong, 2),
            f(fi.background, 4),
            f(fi.light, 2),
            f(fi.err, 2),
            fi.status.name(),
            if fi.reason.is_empty() { String::new() } else { format!(": {}", fi.reason) }
        );
    }
    s
}

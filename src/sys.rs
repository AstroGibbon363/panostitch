//! System resources: threads for long jobs (kept below the window so it
//! stays responsive), free memory and disk space.

use std::sync::OnceLock;

pub use crate::stacker::{available_disk, available_memory};

/// Total memory (bytes).
pub fn total_memory() -> f64 {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};
    System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::everything())).total_memory() as f64
}

/// Free memory to leave to the system and the window (bytes): 1 GB, or 10 % on large machines.
pub fn memory_reserve() -> f64 {
    1e9_f64.max(0.1 * total_memory())
}

pub fn gb(bytes: f64) -> String {
    if bytes < 1e9 { format!("{:.0} MB", bytes / 1e6) } else { format!("{:.1} GB", bytes / 1e9) }
}

/// Lower the priority of the calling thread (below the window's).
pub fn lower_this_thread() {
    #[cfg(target_os = "linux")]
    unsafe {
        // On Linux the nice value is per thread.
        let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
        libc::setpriority(libc::PRIO_PROCESS, tid, 10);
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
    #[cfg(target_os = "macos")]
    unsafe {
        // Quality of service "utility": long-running work the user waits for, below the window.
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
}

/// A thread pool of `n` low-priority threads (0 = one per CPU core).
pub fn low_priority_pool(n: usize) -> Result<rayon::ThreadPool, String> {
    rayon::ThreadPoolBuilder::new().num_threads(n).thread_name(|i| format!("work-{i}")).start_handler(|_| lower_this_thread()).build().map_err(|e| e.to_string())
}

/// The pool long jobs of the window run in. It is separate from rayon's global
/// pool, which the window thread itself uses for small things (showing an
/// image): if they shared one pool, the window would wait behind a long job
/// and the system would report it as not responding.
pub fn work_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| low_priority_pool(0).expect("cannot start worker threads"))
}

/// Run `f` in the background: on its own (low-priority) thread, with parallel
/// work inside it going to the work pool.
pub fn background(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        lower_this_thread();
        work_pool().install(f)
    });
}

/// Free memory and free disk space in `dir`, refreshed at most every 3 s
/// (cheap enough to call while drawing the window).
pub fn free_cached(dir: &std::path::Path) -> (f64, Option<u64>) {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    type Cache = (Option<(Instant, f64)>, HashMap<std::path::PathBuf, (Instant, Option<u64>)>);
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let ttl = Duration::from_secs(3);
    let mut c = CACHE.get_or_init(|| Mutex::new((None, HashMap::new()))).lock().unwrap_or_else(|e| e.into_inner());
    let mem = match c.0 {
        Some((t, m)) if t.elapsed() < ttl => m,
        _ => {
            let m = available_memory();
            c.0 = Some((Instant::now(), m));
            m
        }
    };
    let disk = match c.1.get(dir) {
        Some((t, d)) if t.elapsed() < ttl => *d,
        _ => {
            let d = available_disk(dir);
            c.1.insert(dir.to_path_buf(), (Instant::now(), d));
            d
        }
    };
    (mem, disk)
}

/// Whether `file` can be written: its folder exists and accepts new files.
/// Checked before long jobs, so they do not fail only at the very end.
pub fn check_writable(file: &str) -> Result<(), String> {
    let p = std::path::Path::new(file);
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    if !dir.is_dir() {
        return Err(format!("The folder {} does not exist: choose another place to save.", dir.display()));
    }
    tempfile::NamedTempFile::new_in(dir).map(|_| ()).map_err(|e| format!("Cannot save into {}: {e}. Choose another folder.", dir.display()))?;
    if p.is_dir() {
        return Err(format!("{file} is a folder: add a file name."));
    }
    Ok(())
}

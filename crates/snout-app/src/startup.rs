//! The way to the first window, timed: with `SNOUT_STARTUP_TRACE=1` each step prints how long
//! after the process started it was reached (on stderr, so in the log), from the process's own
//! start time as the system recorded it, so the time spent loading the program counts too.

use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

/// Whether the trace is on, and when the process started.
fn trace() -> Option<SystemTime> {
    static TRACE: OnceLock<Option<SystemTime>> = OnceLock::new();
    *TRACE.get_or_init(|| {
        std::env::var_os("SNOUT_STARTUP_TRACE")
            .filter(|v| v != "0")
            .map(|_| process_start().unwrap_or_else(SystemTime::now))
    })
}

/// When this process started, by the kernel's record of it.
#[cfg(target_os = "macos")]
fn process_start() -> Option<SystemTime> {
    // SAFETY: `proc_bsdinfo` is plain data, valid all zeros.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: `info` is a `proc_bsdinfo` of the size given.
    let got = unsafe {
        libc::proc_pidinfo(std::process::id() as libc::c_int, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size)
    };
    (got == size).then(|| {
        SystemTime::UNIX_EPOCH
            + Duration::from_secs(info.pbi_start_tvsec)
            + Duration::from_micros(info.pbi_start_tvusec)
    })
}

#[cfg(not(target_os = "macos"))]
fn process_start() -> Option<SystemTime> {
    None
}

/// Note that startup reached `step` (when the trace is on).
pub fn mark(step: &str) {
    if let Some(start) = trace() {
        let ms = SystemTime::now().duration_since(start).unwrap_or_default().as_secs_f64() * 1000.;
        eprintln!("startup {ms:8.1} ms  {step}");
    }
}

/// With the trace on, note each time in the first seconds the UI thread was busy long enough
/// to miss frames: a sign of work that should run elsewhere.
pub fn watch_ui_thread(cx: &mut gpui::App) {
    if trace().is_none() {
        return;
    }
    cx.spawn(async move |cx| {
        let tick = Duration::from_millis(5);
        let mut last = std::time::Instant::now();
        for _ in 0..2000 {
            cx.background_executor().timer(tick).await;
            let gap = last.elapsed();
            if gap > Duration::from_millis(40) {
                mark(&format!("the UI thread was busy for {} ms", gap.as_millis()));
            }
            last = std::time::Instant::now();
        }
    })
    .detach();
}

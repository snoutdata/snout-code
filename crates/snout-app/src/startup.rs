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
fn process_start() -> Option<SystemTime> {
    crate::platform::process_start()
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

/// Print the messages of GPUI, wgpu and the other libraries on stderr (so in the log), at the
/// level `SNOUT_LOG` names (`error`, `warn`, `info`, `debug` or `trace`); errors only without
/// it. `SNOUT_LOG=info` says which GPU the window draws with, and why one was passed over.
pub fn init_log() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::max_level()
        }
        fn log(&self, r: &log::Record) {
            if self.enabled(r.metadata()) {
                eprintln!("[{}] {}: {}", r.level().as_str().to_lowercase(), r.target(), r.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    let level = std::env::var("SNOUT_LOG").ok().and_then(|v| v.parse().ok()).unwrap_or(log::LevelFilter::Error);
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(level);
    }
}

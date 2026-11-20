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

/// The file a panic is written to: `panic.log` among the app's logs
/// (`~/Library/Logs/SnoutCode` on macOS). A window started from the Finder or the Start menu
/// has no stderr anyone reads, and a panic in a window's event handler aborts the app, so
/// this is where the reason stays.
pub fn panic_log_path() -> std::path::PathBuf {
    snout_host::dirs::AppDirs::local().state.join("panic.log")
}

/// Write each panic to [`panic_log_path`] (its message, where, the thread, a backtrace),
/// then let the default hook print it as before.
pub fn install_panic_log() {
    let path = panic_log_path();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        write_panic(&path, &panic_entry(info, &backtrace.to_string()));
        previous(info);
    }));
}

/// One panic, as the log keeps it.
fn panic_entry(info: &std::panic::PanicHookInfo<'_>, backtrace: &str) -> String {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".to_owned());
    let at = info.location().map_or_else(String::new, |l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
    let seconds = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    entry_text(seconds, std::thread::current().name().unwrap_or("unnamed"), &at, &message, backtrace)
}

fn entry_text(seconds: u64, thread: &str, at: &str, message: &str, backtrace: &str) -> String {
    format!(
        "=== panic at {seconds} (unix seconds), SnoutCode {} on {}-{}\nthread '{thread}' panicked at {at}:\n{message}\n\n{backtrace}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Add `entry` to the log at `path`; a log over 1 MB starts over. Never fails.
fn write_panic(path: &std::path::Path, entry: &str) {
    use std::io::Write as _;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > 1 << 20) {
        let _ = std::fs::remove_file(path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(entry.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_is_written_with_its_message_and_place() {
        let dir = crate::tests::scratch_dir("panic-log");
        let path = dir.join("logs/panic.log");
        let entry = entry_text(7, "main", "src/view.rs:12:5", "index out of bounds", "the backtrace");
        write_panic(&path, &entry);
        write_panic(&path, &entry);
        let log = std::fs::read_to_string(&path).unwrap();
        assert_eq!(log.matches("=== panic at 7 ").count(), 2, "{log}");
        assert!(
            log.contains("thread 'main' panicked at src/view.rs:12:5:\nindex out of bounds\n\nthe backtrace"),
            "{log}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

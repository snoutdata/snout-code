//! Processes start with no signal blocked and every signal at its default action.
//!
//! A process inherits the signal mask of the thread that starts it, and the signals its
//! parent ignores. The app starts processes on background threads that block signals, so a
//! child started from one of them kept SIGTERM, SIGINT and SIGHUP blocked: a `kill` of ssh,
//! Gradle, a run or the engine stayed pending and did nothing. Every `std::process::Command`
//! SnoutCode starts goes through [`default_signals`]. Terminals start through `portable-pty`,
//! which clears the mask and resets the signals that end a process itself.

use std::process::Command;

/// The highest signal number of the system.
#[cfg(target_os = "linux")]
const LAST_SIGNAL: libc::c_int = 64;
#[cfg(not(target_os = "linux"))]
const LAST_SIGNAL: libc::c_int = 31;

/// Make `c` start its process with no signal blocked and every signal at its default action.
pub fn default_signals(c: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt as _;
    // SAFETY: the closure runs between fork and exec and makes only async-signal-safe calls
    // (sigaction, sigemptyset, pthread_sigmask).
    unsafe {
        c.pre_exec(|| {
            reset();
            Ok(())
        })
    }
}

/// Every signal to its default action, then none blocked. Async-signal-safe.
unsafe fn reset() {
    // SAFETY: plain system calls on this process; zeroed is a valid sigaction and sigset_t.
    unsafe {
        let mut dfl: libc::sigaction = std::mem::zeroed();
        dfl.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&mut dfl.sa_mask);
        for sig in 1..=LAST_SIGNAL {
            if sig != libc::SIGKILL && sig != libc::SIGSTOP {
                // Fails for the numbers the C library keeps for itself, which is fine.
                libc::sigaction(sig, &dfl, std::ptr::null_mut());
            }
        }
        let mut none: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut none);
        libc::pthread_sigmask(libc::SIG_SETMASK, &none, std::ptr::null_mut());
    }
}

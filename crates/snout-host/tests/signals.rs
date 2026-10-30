//! A process started from a thread that blocks signals, in a process that ignores one, still
//! starts with no signal blocked and every signal at its default action. Its own test binary,
//! since it changes what the whole process does with SIGUSR1.

#![cfg(unix)]

use std::time::{Duration, Instant};

use snout_host::{Cmd, Host as _, LocalHost};

/// Send `sig` to `pid` and wait up to five seconds for `exited` to say it ended.
fn ends_on(pid: u32, sig: libc::c_int, mut exited: impl FnMut() -> bool) -> bool {
    // SAFETY: a plain system call on a child of ours that has not been waited for.
    assert_eq!(unsafe { libc::kill(libc::pid_t::try_from(pid).unwrap(), sig) }, 0);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if exited() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn children_start_with_default_signals() {
    // SAFETY: SIGUSR1 is not used by anything else in this test binary.
    unsafe { libc::signal(libc::SIGUSR1, libc::SIG_IGN) };
    // As the app's background threads do.
    std::thread::spawn(|| {
        // SAFETY: changes the mask of this thread only.
        unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                libc::sigaddset(&mut set, sig);
            }
            libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        }
        let host = LocalHost::new();
        let sleep = Cmd::new("/bin/sleep").arg("30");

        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGUSR1] {
            let mut p = host.spawn(&sleep).unwrap();
            let pid = p.pid().unwrap();
            let ended = ends_on(pid, sig, || p.try_wait().unwrap().is_some());
            if !ended {
                p.kill().unwrap();
            }
            assert!(ended, "a process of the host ends on signal {sig}");
            assert_eq!(p.wait().unwrap().signal, Some(sig));
        }

        // Terminals: SIGTERM and SIGHUP, which end a shell.
        for sig in [libc::SIGTERM, libc::SIGHUP] {
            let mut p = host.spawn_pty(&sleep, 24, 80).unwrap();
            let pid = p.pid().unwrap();
            let ended = ends_on(pid, sig, || p.try_wait().unwrap().is_some());
            if !ended {
                p.kill().unwrap();
            }
            assert!(ended, "a terminal's process ends on signal {sig}");
        }

        // A plain command made with the helper.
        let mut c = std::process::Command::new("/bin/sleep");
        let mut child = snout_host::default_signals(&mut c).arg("30").spawn().unwrap();
        let ended = ends_on(child.id(), libc::SIGTERM, || child.try_wait().unwrap().is_some());
        if !ended {
            child.kill().unwrap();
        }
        assert!(ended, "a command with default_signals ends on SIGTERM");
    })
    .join()
    .unwrap();
}

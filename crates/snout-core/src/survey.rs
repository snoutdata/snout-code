//! Ahead and behind counts for every repository in the list, so the list can show which
//! ones have commits to pull. A pass runs on a thread over the whole list; every so often
//! it fetches first, so the counts say what is on the remote now and not only what was
//! there at the last fetch.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use crate::{Repo, Tracking};

/// How often a pass fetches the repositories in the list.
pub const FETCH_EVERY: Duration = Duration::from_secs(10 * 60);

/// One pass over the repositories. Dropping it stops the thread after the repository it is on.
pub struct Survey {
    rx: mpsc::Receiver<(PathBuf, Option<Tracking>)>,
    /// Set while one of the user's jobs runs: the pass fetches nothing then, so the two do
    /// not race for the same refs.
    hold: Arc<AtomicBool>,
    pub fetches: bool,
}

impl Survey {
    /// Start a pass; `wake` runs on the pass's thread after each repository, to have the
    /// front end come and [`Survey::poll`].
    pub fn start(repos: Vec<PathBuf>, fetch: bool, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let hold = Arc::new(AtomicBool::new(false));
        let held = hold.clone();
        std::thread::spawn(move || {
            for path in repos {
                let tracking = Repo::open(&path).ok().and_then(|repo| {
                    if fetch && !held.load(Ordering::Relaxed) {
                        // Offline, signed out, no remote: the counts from the last fetch stand.
                        let _ = repo.fetch_unattended();
                    }
                    repo.tracking().ok().flatten()
                });
                if tx.send((path, tracking)).is_err() {
                    return; // the app let go of the pass
                }
                wake();
            }
        });
        Self { rx, hold, fetches: fetch }
    }

    pub fn hold(&self, on: bool) {
        self.hold.store(on, Ordering::Relaxed);
    }

    /// The repositories done since the last call, and whether the pass is over.
    pub fn poll(&self) -> (Vec<(PathBuf, Option<Tracking>)>, bool) {
        let mut done = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(r) => done.push(r),
                Err(mpsc::TryRecvError::Empty) => return (done, false),
                Err(mpsc::TryRecvError::Disconnected) => return (done, true),
            }
        }
    }
}

/// The chip text for a repository row ("↓3", "↑1 ↓3") and the matching hover line, when
/// there is anything to pull or push.
pub fn describe(t: Tracking) -> Option<(String, String)> {
    let commits = |n: u32| if n == 1 { "1 commit".to_owned() } else { format!("{n} commits") };
    match (t.ahead, t.behind) {
        (0, 0) => None,
        (0, b) => Some((format!("↓{b}"), format!("{} to pull", commits(b)))),
        (a, 0) => Some((format!("↑{a}"), format!("{} to push", commits(a)))),
        (a, b) => Some((format!("↑{a} ↓{b}"), format!("{} to pull, {a} to push", commits(b)))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_say_what_to_pull_and_push() {
        let t = |ahead, behind| describe(Tracking { ahead, behind });
        assert_eq!(t(0, 0), None);
        assert_eq!(t(0, 1), Some(("↓1".into(), "1 commit to pull".into())));
        assert_eq!(t(2, 3), Some(("↑2 ↓3".into(), "3 commits to pull, 2 to push".into())));
    }
}

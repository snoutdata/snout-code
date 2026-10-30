//! Ahead and behind counts for the repositories in the list, so the list can show which
//! ones have commits to pull. The app's pass over the list counts one repository at a time;
//! every so often it fetches first, so the counts say what is on the remote now and not only
//! what was there at the last fetch.

use std::time::Duration;

use snout_host::HostRef;

use crate::{Repo, Tracking};

/// How often a pass fetches the repositories in the list.
pub const FETCH_EVERY: Duration = Duration::from_secs(10 * 60);

/// How one repository on `host` stands against its upstream, fetching first when `fetch`.
/// Blocks; `None` when it cannot be opened or has no upstream.
pub fn count(host: &HostRef, path: &std::path::Path, fetch: bool) -> Option<Tracking> {
    let repo = Repo::open(host, path).ok()?;
    if fetch {
        // Offline, signed out, no remote: the counts from the last fetch stand.
        let _ = repo.fetch_unattended();
    }
    repo.tracking().ok().flatten()
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

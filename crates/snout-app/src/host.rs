//! The host the workspace lives on (see `snout-host`): this Mac, or a slowed-down copy of it
//! with `SNOUT_HOST_LATENCY_MS`. Everything of the workspace goes through it, off the UI thread.

use gpui::{App, Global};
pub use snout_host::{Host, HostRef, local};

/// The app's host.
pub struct HostGlobal(pub HostRef);

impl Global for HostGlobal {}

/// The workspace's host: the one the app was given, else this Mac.
pub fn host(cx: &App) -> HostRef {
    cx.try_global::<HostGlobal>().map_or_else(snout_host::local, |h| h.0.clone())
}

/// Facts about paths on the host, asked off the UI thread and kept: a view shows what is
/// known, and redraws when an answer comes in.
pub struct Facts<T> {
    known: std::collections::HashMap<std::path::PathBuf, T>,
    asked: std::collections::HashSet<std::path::PathBuf>,
}

impl<T> Default for Facts<T> {
    fn default() -> Self {
        Self { known: Default::default(), asked: Default::default() }
    }
}

impl<T: Clone + Send + 'static> Facts<T> {
    /// What is known of `path`; asked with `ask` (on a background thread) when it is not.
    /// `of` finds these facts in the view `V`.
    pub fn get<V: 'static>(
        &mut self,
        path: &std::path::Path,
        cx: &mut gpui::Context<V>,
        of: fn(&mut V) -> &mut Self,
        ask: fn(&dyn Host, &std::path::Path) -> T,
    ) -> Option<T> {
        if let Some(t) = self.known.get(path) {
            return Some(t.clone());
        }
        if self.asked.insert(path.to_owned()) {
            let (h, p) = (host(cx), path.to_owned());
            let answer = cx.background_executor().spawn(async move { (ask(&*h, &p), p) });
            cx.spawn(async move |this, cx| {
                let (t, p) = answer.await;
                let _ = this.update(cx, |view, cx| {
                    let facts = of(view);
                    if facts.asked.remove(&p) {
                        facts.known.insert(p, t);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        None
    }

    /// Forget everything, to ask again.
    pub fn clear(&mut self) {
        self.known.clear();
        self.asked.clear();
    }
}

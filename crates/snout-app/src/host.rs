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

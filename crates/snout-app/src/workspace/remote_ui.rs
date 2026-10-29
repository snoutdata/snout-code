//! Remote SSH in the window: Connect to SSH Host, the folder picker of a remote machine,
//! opening a remote project (the window opens again over the remote host) and closing the
//! connection (it opens again on this Mac). See [`crate::remote`].

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{AppContext as _, Context, Window};
use snout_remote::RemoteHost;

use super::Workspace;
use crate::model::Action;
use crate::remote::dialog::{ConnectDialog, ConnectEvent};
use crate::remote::picker::{FolderPicker, PickerEvent};
use crate::remote::{self, RemoteProject};

/// What a folder picked on the remote machine is for.
#[derive(Clone)]
pub enum Purpose {
    /// The first project of a new connection.
    Connect(String, Arc<RemoteHost>),
    /// An action of the window already on the remote machine (Open Project…).
    Then(fn(PathBuf) -> Action),
}

impl Workspace {
    /// File ▸ Connect to SSH Host….
    pub fn open_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.connect_dialog.is_some() {
            return;
        }
        let recent = remote::recents(cx).hosts;
        let configured: Vec<String> = remote::known_hosts(cx).into_iter().filter(|h| !recent.contains(h)).collect();
        let dialog = cx.new(|cx| ConnectDialog::new(recent, configured, window, cx));
        cx.subscribe_in(&dialog, window, |this, dialog, e: &ConnectEvent, window, cx| match e {
            ConnectEvent::Cancel => {
                this.connect_dialog = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
            ConnectEvent::Connect(name) => {
                let (ws, d, name) = (cx.weak_entity(), dialog.downgrade(), name.clone());
                let handle = window.window_handle();
                remote::connect(
                    name.clone(),
                    move |result, cx| match result {
                        Ok(host) => {
                            let _ = handle.update(cx, |_, window, cx| {
                                let _ = ws.update(cx, |this, cx| {
                                    this.connect_dialog = None;
                                    this.pick_remote(Purpose::Connect(name, host), window, cx);
                                });
                            });
                        }
                        Err(why) => {
                            let _ = d.update(cx, |d, cx| d.failed(why, cx));
                        }
                    },
                    cx,
                );
            }
        })
        .detach();
        let input = dialog.read(cx).input.clone();
        cx.defer_in(window, move |_, window, cx| input.update(cx, |i, cx| i.focus(window, cx)));
        self.connect_dialog = Some(dialog);
        cx.notify();
    }

    /// Put up the folder picker of the remote machine, for `purpose`.
    pub fn pick_remote(&mut self, purpose: Purpose, window: &mut Window, cx: &mut Context<Self>) {
        let (host, name): (snout_host::HostRef, String) = match &purpose {
            Purpose::Connect(name, host) => {
                cx.global_mut::<remote::Remote>().pending = Some((name.clone(), host.clone()));
                (host.clone(), name.clone())
            }
            Purpose::Then(_) => {
                let Some((name, _)) = remote::connection(cx) else { return };
                (crate::host::host(cx), name)
            }
        };
        // From the project open last on that machine, else its home.
        let start = match &purpose {
            Purpose::Connect(..) => remote::recents(cx)
                .projects
                .iter()
                .find(|p| p.host == name)
                .and_then(|p| p.path.parent().map(PathBuf::from)),
            Purpose::Then(_) => self.model.read(cx).project_root().and_then(|p| p.parent().map(PathBuf::from)),
        }
        .unwrap_or_else(|| host.info().home.clone());
        let picker = cx.new(|cx| FolderPicker::new(host, name, start, window, cx));
        cx.subscribe_in(&picker, window, move |this, _, e: &PickerEvent, window, cx| {
            this.remote_picker = None;
            window.focus(&this.focus, cx);
            match (e, &purpose) {
                (PickerEvent::Open(dir), Purpose::Connect(name, host)) => {
                    this.open_remote(name.clone(), host.clone(), dir.clone(), window, cx)
                }
                (PickerEvent::Open(dir), Purpose::Then(then)) => this.send(then(dir.clone()), cx),
                (PickerEvent::Cancel, Purpose::Connect(..)) => remote::drop_pending(cx),
                (PickerEvent::Cancel, Purpose::Then(_)) => {}
            }
            cx.notify();
        })
        .detach();
        let input = picker.read(cx).input.clone();
        cx.defer_in(window, move |_, window, cx| input.update(cx, |i, cx| i.focus(window, cx)));
        self.remote_picker = Some(picker);
        cx.notify();
    }

    /// Leave this window's project (its unsaved tabs settled, its engines stopped) for
    /// `dir` on the remote machine `host`.
    fn open_remote(
        &mut self,
        name: String,
        host: Arc<RemoteHost>,
        dir: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settle_unsaved(window, cx, move |this, window, cx| {
            this.stash_session(cx);
            this.save_layout(cx);
            this.hub.update(cx, |h, _| h.shutdown_all());
            let model = this.model.clone();
            let old = window.window_handle();
            cx.defer(move |cx| remote::open_project(name, host, dir, &model, old, cx));
        });
    }

    /// File ▸ Open Recent on a remote project: on the connection there is, else after
    /// connecting.
    pub fn open_remote_recent(&mut self, p: RemoteProject, window: &mut Window, cx: &mut Context<Self>) {
        if remote::connection(cx).is_some_and(|(name, _)| name == p.host) {
            self.send(Action::OpenPath(p.path), cx);
            return;
        }
        let (ws, handle) = (cx.weak_entity(), window.window_handle());
        let RemoteProject { host: name, path } = p;
        remote::connect(
            name.clone(),
            move |result, cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = ws.update(cx, |this, cx| match result {
                        Ok(host) => this.open_remote(name, host, path, window, cx),
                        Err(why) => this.model.update(cx, |m, cx| {
                            m.message = format!("Could not connect to {name}: {why}");
                            cx.notify();
                        }),
                    });
                });
            },
            cx,
        );
    }

    /// File ▸ Close Remote Connection: the window opens again on this Mac.
    pub fn close_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !remote::is_remote(cx) {
            return;
        }
        self.settle_unsaved(window, cx, |this, window, cx| {
            this.stash_session(cx);
            this.save_layout(cx);
            this.hub.update(cx, |h, _| h.shutdown_all());
            let old = window.window_handle();
            cx.defer(move |cx| remote::disconnect(old, cx));
        });
    }
}

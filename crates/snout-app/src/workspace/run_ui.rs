//! The workspace's side of running and debugging: the tool windows the runs bring forward,
//! the places their consoles link to, the title bar's Run, Debug and Stop, the debugger's
//! keys, and the Edit Configurations dialog.

use std::path::PathBuf;

use gpui::{AppContext as _, Context, Entity, Window};

use super::Workspace;
use crate::run::ansi::Link;
use crate::run::config_dialog::{ConfigDialog, ConfigEvent};
use crate::run::{Launch, Place, Runs, RunsEvent, Target};

impl Workspace {
    pub(super) fn on_runs_event(
        &mut self,
        _: &Entity<Runs>,
        e: &RunsEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match e {
            RunsEvent::Show(place) => {
                let id = match place {
                    Place::Run => self.tabs.run,
                    Place::Debug => self.tabs.debug,
                    Place::Build => self.tabs.build,
                };
                if self.side.read(cx).activity == crate::panels::Activity::Git {
                    self.show_activity(crate::panels::Activity::Project, window, cx);
                }
                self.show_bottom(id, window, cx);
            }
            RunsEvent::OpenLink(link) => self.open_link(link.clone(), window, cx),
            RunsEvent::OpenAt(path, line) => self.open_file(path.clone(), Some((*line, 0)), false, window, cx),
            RunsEvent::EditConfigurations => self.open_config_dialog(window, cx),
            RunsEvent::ShowProblems => self.show_problems(window, cx),
        }
    }

    /// Open the place a console line names: a path, or a stack frame's class found under the
    /// project's source folders.
    pub fn open_link(&mut self, link: Link, window: &mut Window, cx: &mut Context<Self>) {
        let line = link.line().saturating_sub(1);
        let path = match &link {
            Link::Path { path, column, .. } => {
                if path.is_file() {
                    return self.open_file(path.clone(), Some((line, column.saturating_sub(1))), false, window, cx);
                }
                None
            }
            Link::Frame { .. } => link.relative_file().and_then(|rel| self.find_source(&rel, cx)),
        };
        match path {
            Some(path) => self.open_file(path, Some((line, 0)), false, window, cx),
            None => self.model.update(cx, |m, cx| {
                m.message = match &link {
                    Link::Frame { class, .. } => format!("{class} is not among the project's sources"),
                    Link::Path { path, .. } => format!("{} does not exist", path.display()),
                };
                cx.notify();
            }),
        }
    }

    /// The file at `rel` (`demo/App.java`) under one of the project's source folders.
    fn find_source(&self, rel: &std::path::Path, cx: &gpui::App) -> Option<PathBuf> {
        let model = crate::project_model::store(cx)?.read(cx).current.clone()?;
        let roots =
            model.modules.iter().flat_map(|m| m.roots.iter()).chain(model.shared.iter().flat_map(|s| s.roots.iter()));
        for root in roots {
            let candidate = root.dir.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        // Scala and Kotlin keep classes in files of other names: look for the file alone.
        None
    }

    /// The title bar's Run (or Debug): the selected configuration, or what the file in front
    /// holds (its test class, else its main class).
    pub fn run_selected(&mut self, debug: bool, window: &mut Window, cx: &mut Context<Self>) {
        let runs = self.runs.clone();
        if let Some(config) = runs.read(cx).selected_config() {
            runs.update(cx, |r, cx| {
                r.start(Launch::Config { config, debug }, cx);
            });
            return;
        }
        let editor = self.active_editor(window, cx);
        let found = editor.and_then(|e| {
            let r = e.read(cx).runnables.clone()?;
            let pick = r
                .items
                .iter()
                .find(|i| matches!(i.target, Target::TestClass { .. }))
                .or_else(|| r.items.iter().find(|i| matches!(i.target, Target::Main { .. })))
                .cloned()?;
            Some((pick.target, r.module.clone()))
        });
        match found {
            Some((target, module)) => runs.update(cx, |r, cx| r.run_target(&target, module.as_deref(), debug, cx)),
            None => self.model.update(cx, |m, cx| {
                m.message = "The file in front has no tests or main class to run".into();
                cx.notify();
            }),
        }
    }

    /// Stop what runs in the Run and Debug windows.
    pub fn stop_runs(&mut self, cx: &mut Context<Self>) {
        self.runs.update(cx, |r, cx| {
            let ids: Vec<u64> =
                r.sessions.iter().filter(|s| s.running() && s.place() != Place::Build).map(|s| s.id).collect();
            for id in ids {
                r.stop(id, cx);
            }
        });
    }

    /// A debugger key: step, resume.
    pub fn debug_step(&mut self, command: &'static str, cx: &mut Context<Self>) {
        let Some(id) = self.runs.read(cx).shown(Place::Debug).map(|s| s.id) else { return };
        self.runs.update(cx, |r, cx| r.step(id, command, cx));
    }

    /// Put up the Edit Configurations dialog.
    pub fn open_config_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.config_dialog.is_some() {
            return;
        }
        let runs = self.runs.clone();
        let dialog = cx.new(|cx| ConfigDialog::new(runs, window, cx));
        cx.subscribe_in(&dialog, window, |this, _, e: &ConfigEvent, window, cx| {
            let ConfigEvent::Close = e;
            this.config_dialog = None;
            window.focus(&this.focus, cx);
            cx.notify();
        })
        .detach();
        let focus = gpui::Focusable::focus_handle(dialog.read(cx), cx);
        window.focus(&focus, cx);
        self.config_dialog = Some(dialog);
        cx.notify();
    }
}

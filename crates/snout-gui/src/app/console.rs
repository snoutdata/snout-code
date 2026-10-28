//! The Claude Code console: `claude` running in the open repository, docked on the right.
//! One session per repository, kept alive while switching between them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};

use snout_core::claude::{ENV_REMOVE, find_claude};

use crate::art::{self, Glyph};
use crate::terminal::{Launch, Terminal};
use crate::theme::Palette;

/// Refresh the Git panes once the agent has been quiet this long...
const QUIET: Duration = Duration::from_millis(800);
/// ...and at most this often while it keeps printing.
const BUSY_REFRESH: Duration = Duration::from_secs(3);

struct Session {
    term: Terminal,
    seen_gen: u64,
    refreshed: Instant,
}

#[derive(Default)]
pub struct Console {
    sessions: HashMap<PathBuf, Session>,
    /// Program override (tests); otherwise `claude` is looked up.
    pub program: Option<PathBuf>,
    error: Option<String>,
    focus_next: bool,
}

pub enum ConsoleEvent {
    /// The agent printed and then went quiet: files may have changed.
    Activity,
    Close,
}

impl Console {
    /// Give the terminal keyboard focus the next time it is shown.
    pub fn focus(&mut self) {
        self.focus_next = true;
    }

    /// The `claude` to run: the override, or the one found on this machine.
    pub fn claude(&self) -> Option<PathBuf> {
        self.program.clone().or_else(find_claude)
    }

    #[cfg(test)]
    pub fn session_text(&self, root: &Path) -> Option<String> {
        self.sessions.get(root).map(|s| s.term.contents())
    }

    pub fn start(&mut self, root: &Path, args: &[String], ctx: &egui::Context) {
        self.sessions.remove(root);
        self.error = None;
        let Some(program) = self.claude() else {
            self.error = Some(
                "Claude Code was not found on PATH or in ~/.local/bin, ~/.claude/local, \
                 /opt/homebrew/bin, /usr/local/bin.\nInstall it from https://docs.claude.com/en/docs/claude-code \
                 and press Start."
                    .into(),
            );
            return;
        };
        let launch = Launch { program: &program, args, cwd: root, env: &[], env_remove: ENV_REMOVE };
        match Terminal::spawn(&launch, ctx) {
            Ok(term) => {
                self.sessions.insert(root.to_owned(), Session { term, seen_gen: 0, refreshed: Instant::now() });
                self.focus_next = true;
            }
            Err(e) => self.error = Some(format!("Could not start {}: {e}", program.display())),
        }
    }

    pub fn stop(&mut self, root: &Path) {
        if let Some(mut s) = self.sessions.remove(root) {
            s.term.kill();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, root: Option<&Path>) -> Option<ConsoleEvent> {
        let mut event = None;
        let Some(root) = root else {
            header(ui, "", None, |ui| {
                if art::icon_button(ui, Glyph::Close, true).on_hover_text("Hide (⌘J)").clicked() {
                    event = Some(ConsoleEvent::Close);
                }
            });
            super::centered_note(ui, "Open a repository to run Claude Code in it");
            return event;
        };
        if !self.sessions.contains_key(root) && self.error.is_none() {
            self.start(root, &[], ui.ctx());
        }

        let mut restart: Option<Vec<String>> = None;
        let mut stop = false;
        let exit = self.sessions.get_mut(root).and_then(|s| s.term.exit_status().map(str::to_owned));
        let title = self.sessions.get(root).map(|s| s.term.title()).unwrap_or_default();
        let running = self.sessions.contains_key(root) && exit.is_none();
        header(ui, &title, Some(running), |ui| {
            if art::icon_button(ui, Glyph::Close, true).on_hover_text("Hide (⌘J); the session keeps running").clicked()
            {
                event = Some(ConsoleEvent::Close);
            }
            ui.add_space(4.0);
            if running && art::chip_button(ui, Glyph::Stop, "End", true).on_hover_text("End this session").clicked() {
                stop = true;
            }
            if art::chip_button(ui, Glyph::Refresh, "Continue", true)
                .on_hover_text("Restart with the most recent conversation in this folder (claude --continue)")
                .clicked()
            {
                restart = Some(vec!["--continue".into()]);
            }
            if art::chip_button(ui, Glyph::Plus, "New", true).on_hover_text("Restart with a new conversation").clicked()
            {
                restart = Some(Vec::new());
            }
        });

        match self.sessions.get_mut(root) {
            Some(session) => {
                if let Some(how) = &exit {
                    ui.horizontal(|ui| {
                        ui.weak(format!("Claude Code {how}."));
                        if ui.link("Start a new session").clicked() {
                            restart = Some(Vec::new());
                        }
                    });
                }
                let response = session.term.show(ui);
                if std::mem::take(&mut self.focus_next) {
                    response.request_focus();
                }
                let generation = session.term.output_gen();
                let quiet = session.term.last_output().is_some_and(|t| t.elapsed() >= QUIET);
                if generation != session.seen_gen {
                    if quiet || session.refreshed.elapsed() >= BUSY_REFRESH {
                        session.seen_gen = generation;
                        session.refreshed = Instant::now();
                        event = event.or(Some(ConsoleEvent::Activity));
                    } else {
                        // Come back when the quiet period is over.
                        ui.ctx().request_repaint_after(QUIET);
                    }
                }
            }
            None => {
                ui.add_space(12.0);
                ui.label(self.error.as_deref().unwrap_or("Not running"));
                if ui.button("Start").clicked() {
                    restart = Some(Vec::new());
                }
            }
        }

        if stop {
            self.stop(root);
        }
        if let Some(args) = restart {
            self.start(root, &args, ui.ctx());
        }
        event
    }
}

/// The pane's title row: the spark, the name, the session's own title, and a dot that is
/// green while the session runs (`running` is `None` when there is no session to speak of).
fn header(ui: &mut egui::Ui, title: &str, running: Option<bool>, buttons: impl FnOnce(&mut egui::Ui)) {
    let p = Palette::of(ui.visuals());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
        Glyph::Spark.drawing().paint(ui.painter(), rect, Some(p.fg));
        ui.label(RichText::new("Claude Code").strong());
        if let Some(running) = running {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 3.5, if running { p.ok } else { p.fg_dim });
        }
        let title = title.trim().trim_start_matches(['✳', '✻', '·', '✶', '✢', '*', ' ']);
        if !title.is_empty() && title != "Claude Code" {
            ui.add(egui::Label::new(RichText::new(title).color(p.fg_dim)).truncate());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            buttons(ui);
        });
    });
    ui.add_space(4.0);
    ui.separator();
}

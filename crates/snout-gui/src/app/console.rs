//! The Claude Code console: `claude` running in the open repository, docked on the right.
//! One session per repository, kept alive while switching between them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};

use crate::terminal::{Launch, Terminal};

/// Inherited variables that would make Claude Code think it runs nested inside another
/// session (snout started from a Claude Code terminal), or misreport the terminal.
const ENV_REMOVE: &[&str] =
    &["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_SSE_PORT", "TERM_PROGRAM", "TERM_PROGRAM_VERSION"];

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

    #[cfg(test)]
    pub fn session_text(&self, root: &Path) -> Option<String> {
        self.sessions.get(root).map(|s| s.term.contents())
    }

    pub fn start(&mut self, root: &Path, args: &[String], ctx: &egui::Context) {
        self.sessions.remove(root);
        self.error = None;
        let Some(program) = self.program.clone().or_else(find_claude) else {
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
            header(ui, "", |ui| {
                if ui.small_button("✕").on_hover_text("Hide (⌘J)").clicked() {
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
        header(ui, &title, |ui| {
            if ui.small_button("✕").on_hover_text("Hide (⌘J); the session keeps running").clicked() {
                event = Some(ConsoleEvent::Close);
            }
            if self.sessions.contains_key(root)
                && exit.is_none()
                && ui.small_button("⏹ End").on_hover_text("End this session").clicked()
            {
                stop = true;
            }
            if ui
                .small_button("↻ Continue")
                .on_hover_text("Restart with the most recent conversation in this folder (claude --continue)")
                .clicked()
            {
                restart = Some(vec!["--continue".into()]);
            }
            if ui.small_button("✳ New").on_hover_text("Restart with a new conversation").clicked() {
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

fn header(ui: &mut egui::Ui, title: &str, buttons: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("✳ Claude Code").strong());
        let title = title.trim().trim_start_matches(['✳', '✻', '·', '✶', '✢', '*', ' ']);
        if !title.is_empty() && title != "Claude Code" {
            ui.weak(title);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), buttons);
    });
}

/// `claude` on PATH or in the places its installers put it (an app started from Finder
/// does not get the shell's PATH).
fn find_claude() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path_dirs = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    let extra = [".local/bin", ".claude/local", ".npm-global/bin", ".bun/bin"]
        .iter()
        .filter_map(|d| home.as_ref().map(|h| h.join(d)))
        .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    path_dirs.into_iter().chain(extra).map(|d| d.join("claude")).find(|p| p.is_file())
}

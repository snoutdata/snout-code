//! Modal dialogs.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, CornerRadius, FontId, Key, Margin, Rect, RichText, Sense, Stroke, Ui, vec2};
use snout_core::{
    Branch, BranchKind, CheckoutTarget, FileEntry, LocalChanges, PullMode, PullProblem, Tag, branch_name_problem,
};

use super::{Action, SnoutApp, dir_name, repo_dir_name, repos, state_color};
use crate::art::{self, Glyph, Paint as _};
use crate::theme::{self, Palette, mix};

pub(super) enum Dialog {
    About,
    Error(String),
    Info {
        title: String,
        text: String,
        mood: Mood,
    },
    ConfirmDiscard(Vec<FileEntry>),
    /// The working tree has changes; ask how to treat them before checking out.
    ConfirmCheckout(CheckoutTarget),
    Checkout(CheckoutPicker),
    NewBranch {
        name: String,
        start: Option<StartPoint>,
        checkout: bool,
    },
    DeleteBranch {
        name: String,
        force: bool,
        /// The branch's upstream, offered for deletion too.
        upstream: Option<RemoteRef>,
        with_upstream: bool,
    },
    DeleteRemoteBranch(RemoteRef),
    ConfirmDeleteFiles(Vec<FileEntry>),
    /// A pull could not combine the local and remote branches.
    PullProblem {
        problem: PullProblem,
        details: String,
    },
    /// A folder that is not a repository was opened.
    InitRepo(PathBuf),
    /// Name a new group (and put `repo` in it), or rename the group `rename`.
    GroupName {
        rename: Option<String>,
        name: String,
        repo: Option<PathBuf>,
    },
    Clone {
        url: String,
        parent: String,
    },
}

/// How an [`Dialog::Info`] message feels, which picks its badge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Mood {
    /// Something worked.
    Done,
    /// A search came back empty.
    Empty,
    /// Something needs a look.
    Warning,
}

/// A branch on a remote: `origin` and `feature/x` for `origin/feature/x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteRef {
    pub remote: String,
    pub branch: String,
}

impl RemoteRef {
    pub fn of(b: &Branch) -> Option<Self> {
        Some(Self { remote: b.remote.clone()?, branch: b.local_name().to_owned() })
    }

    pub fn label(&self) -> String {
        format!("{}/{}", self.remote, self.branch)
    }
}

/// Where a new branch starts. `None` in [`Dialog::NewBranch`] means HEAD.
pub(super) struct StartPoint {
    pub label: String,
    pub rev: String,
}

/// State of the "Check out…" picker.
#[derive(Default)]
pub(super) struct CheckoutPicker {
    pub query: String,
    cursor: usize,
    local: LocalChanges,
    focused: bool,
    /// `describe_commit` result for the query it was computed for.
    preview: Option<(String, Option<String>)>,
}

struct PickItem {
    target: CheckoutTarget,
    label: String,
    detail: String,
    section: &'static str,
    glyph: Glyph,
}

/// Candidates for the picker, filtered by `query`. Remote branches that already have a
/// local counterpart are left out: checking them out would just switch to the local one.
fn pick_items(branches: &[Branch], tags: &[Tag], query: &str, commit: Option<&str>) -> Vec<PickItem> {
    let q = query.to_lowercase();
    let matches = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
    let locals: Vec<&str> = branches.iter().filter(|b| b.kind == BranchKind::Local).map(|b| b.name.as_str()).collect();

    let mut items = Vec::new();
    for b in branches.iter().filter(|b| matches(&b.name)) {
        let (section, include) = match b.kind {
            BranchKind::Local => ("Local branches", !b.is_head),
            BranchKind::Remote => ("Remote branches", !locals.contains(&b.local_name())),
        };
        if include {
            items.push(PickItem {
                target: b.into(),
                label: b.name.clone(),
                detail: format!("{} {}", b.target, b.subject),
                section,
                glyph: if b.kind == BranchKind::Local { Glyph::Branch } else { Glyph::Web },
            });
        }
    }
    items.sort_by_key(|i| i.section != "Local branches");
    for t in tags.iter().filter(|t| matches(&t.name)) {
        items.push(PickItem {
            target: CheckoutTarget::Tag(t.name.clone()),
            label: t.name.clone(),
            detail: format!("{} {}", t.target, t.subject),
            section: "Tags",
            glyph: Glyph::Tag,
        });
    }
    // A query that names a commit but nothing above exactly: offer the commit itself.
    if let Some(described) = commit
        && !items.iter().any(|i| i.label == query)
    {
        items.push(PickItem {
            target: CheckoutTarget::Commit(query.to_owned()),
            label: query.to_owned(),
            detail: described.to_owned(),
            section: "Commit",
            glyph: Glyph::Commit,
        });
    }
    items
}

impl SnoutApp {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let has_changes = !self.status.files.is_empty();
        let has_tracked_changes = self.has_tracked_changes();
        let branch = self.status.branch.head.clone().unwrap_or_else(|| "HEAD".into());
        let upstream = self.status.branch.upstream.clone().unwrap_or_else(|| "its upstream".into());
        let Some(dialog) = &mut self.dialog else { return };
        let (branches, tags, repo, groups) = (&self.branches, &self.tags, self.repo.as_ref(), &self.settings.groups);
        let mut close = false;

        let p = Palette::of(&ctx.global_style().visuals);
        let dark = ctx.global_style().visuals.dark_mode;
        let width = match dialog {
            Dialog::Checkout(_) => 580.0,
            Dialog::About | Dialog::Error(_) | Dialog::PullProblem { .. } | Dialog::Clone { .. } => 520.0,
            _ => 460.0,
        };
        let modal = egui::Modal::new(egui::Id::unique("dialog"))
            .backdrop_color(if dark {
                Color32::from_black_alpha(150)
            } else {
                Color32::from_rgba_unmultiplied(38, 37, 30, 60)
            })
            .frame(
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(Stroke::new(1.0, if dark { mix(p.fg, 0.1, p.card) } else { p.border }))
                    .corner_radius(RADIUS)
                    .shadow(egui::Shadow {
                        offset: [0, 14],
                        blur: 48,
                        spread: 0,
                        color: Color32::from_black_alpha(if dark { 170 } else { 45 }),
                    }),
            );
        let modal = modal.show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 0.0;
            match dialog {
                Dialog::About => {
                    egui::Frame::new().inner_margin(Margin { left: SIDE, right: SIDE, top: 24, bottom: 20 }).show(
                        ui,
                        |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                let w = ui.available_width();
                                art::about().show(ui, vec2(w, w * 124.0 / 440.0));
                                ui.add_space(14.0);
                                ui.label(RichText::new("SnoutCode").size(26.0).strong().color(p.fg));
                                ui.add_space(2.0);
                                dim(ui, &format!("Version {}", env!("CARGO_PKG_VERSION")));
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new("A small, fast desktop Git client, with Claude Code beside it.")
                                        .color(p.fg),
                                );
                                ui.add_space(12.0);
                                ui.hyperlink_to("github.com/snoutdata/snout-code", super::menu::SOURCE_URL);
                                ui.add_space(4.0);
                                ui.hyperlink_to("snoutdata.com", super::menu::WEBSITE_URL);
                                ui.add_space(12.0);
                                dim(ui, "Made by SnoutData. MIT License.");
                            });
                        },
                    );
                    footer(ui, &[("esc", "close")], |ui| {
                        close =
                            theme::primary_button(ui, true, "OK").clicked() || ui.input(|i| i.key_pressed(Key::Enter));
                    });
                }
                Dialog::Error(msg) => {
                    header(ui, Glyph::Alert, Tone::Danger, "Something went wrong", |ui| {
                        dim(ui, "Git reported this:");
                    });
                    body(ui, |ui| code_box(ui, msg, 260.0));
                    footer(ui, &[("esc", "close")], |ui| {
                        close =
                            theme::primary_button(ui, true, "OK").clicked() || ui.input(|i| i.key_pressed(Key::Enter));
                        if theme::plain_button(ui, "Copy").clicked() {
                            ui.ctx().copy_text(msg.clone());
                        }
                    });
                }
                Dialog::PullProblem { problem: PullProblem::Divergent, details } => {
                    header(ui, Glyph::Pull, Tone::Warn, "Both sides have new commits", |ui| {
                        dim(
                            ui,
                            &format!(
                                "{branch} and {upstream} have each moved on, and this repository's settings do not \
                                 say how to combine them."
                            ),
                        );
                    });
                    body(ui, |ui| {
                        if choice(
                            ui,
                            1,
                            Glyph::Commit,
                            "Merge",
                            "Join both histories with a merge commit. Nothing is rewritten.",
                        )
                        .on_hover_text("git pull --no-rebase")
                        .clicked()
                            || ui.input(|i| i.key_pressed(Key::Num1))
                        {
                            actions.push(Action::Pull(PullMode::Merge));
                            close = true;
                        }
                        ui.add_space(8.0);
                        if choice(
                            ui,
                            2,
                            Glyph::Branch,
                            "Rebase",
                            "Replay your commits on top of the remote ones, for a straight history.",
                        )
                        .on_hover_text("git pull --rebase")
                        .clicked()
                            || ui.input(|i| i.key_pressed(Key::Num2))
                        {
                            actions.push(Action::Pull(PullMode::Rebase));
                            close = true;
                        }
                        ui.add_space(10.0);
                        git_details(ui, details);
                    });
                    footer(ui, &[("1", "merge"), ("2", "rebase"), ("esc", "cancel")], |ui| {
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::PullProblem { problem: PullProblem::Unrelated, details } => {
                    header(ui, Glyph::Alert, Tone::Warn, "No shared history", |ui| {
                        dim(
                            ui,
                            &format!(
                                "{branch} and {upstream} have no commit in common, so they cannot be merged. This \
                                 usually means the remote repository was recreated or its history was rewritten."
                            ),
                        );
                    });
                    body(ui, |ui| {
                        ui.label(format!(
                            "Resetting makes {branch} match {upstream} exactly. Your current commits stay on a backup \
                             branch, so nothing is lost."
                        ));
                        if has_tracked_changes {
                            ui.add_space(10.0);
                            callout(
                                ui,
                                Tone::Danger,
                                "Commit or stash your uncommitted changes first; resetting would discard them.",
                            );
                        }
                        ui.add_space(10.0);
                        git_details(ui, details);
                    });
                    footer(ui, &[("esc", "cancel")], |ui| {
                        if theme::primary_button(ui, !has_tracked_changes, &format!("Reset to {upstream}")).clicked() {
                            actions.push(Action::ResetToUpstream);
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::Info { title, text, mood } => {
                    let (glyph, tone) = match mood {
                        Mood::Done => (Glyph::Spark, Tone::Calm),
                        Mood::Empty => (Glyph::Search, Tone::Calm),
                        Mood::Warning => (Glyph::Alert, Tone::Warn),
                    };
                    header(ui, glyph, tone, title, |ui| {
                        egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(text.as_str()).color(Palette::of(ui.visuals()).fg_dim))
                                    .wrap(),
                            )
                        });
                    });
                    ui.add_space(6.0);
                    footer(ui, &[("↵", "close")], |ui| {
                        close =
                            theme::primary_button(ui, true, "OK").clicked() || ui.input(|i| i.key_pressed(Key::Enter));
                    });
                }
                Dialog::ConfirmDiscard(entries) => {
                    let what = match entries.len() {
                        1 => "this file".to_owned(),
                        n => format!("these {n} files"),
                    };
                    header(ui, Glyph::Discard, Tone::Danger, "Discard changes?", |ui| {
                        dim(ui, &format!("This puts {what} back as they are in HEAD. Untracked files are deleted."));
                    });
                    body(ui, |ui| {
                        well(ui, 200.0, |ui| {
                            for e in entries.iter() {
                                file_line(ui, e);
                            }
                        });
                    });
                    footer(ui, &[("esc", "keep them")], |ui| {
                        if theme::danger_button(ui, true, "Discard").clicked() {
                            actions.push(Action::Discard(entries.clone()));
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("This cannot be undone.").size(12.0).color(Palette::of(ui.visuals()).error),
                        );
                    });
                }
                Dialog::ConfirmCheckout(target) => {
                    header(ui, Glyph::Switch, Tone::Calm, &format!("Check out {}?", target.label()), |ui| {
                        dim(ui, "You have uncommitted changes. What should happen to them?");
                    });
                    body(ui, |ui| {
                        if choice(
                            ui,
                            1,
                            Glyph::Stage,
                            "Stash, check out, re-apply",
                            "Put the changes aside, switch, then bring them back. Works even if they touch files that \
                             differ on the target.",
                        )
                        .clicked()
                            || ui.input(|i| i.key_pressed(Key::Num1))
                        {
                            actions.push(Action::Checkout(target.clone(), LocalChanges::Stash));
                            close = true;
                        }
                        ui.add_space(8.0);
                        if choice(
                            ui,
                            2,
                            Glyph::Switch,
                            "Check out, keep changes",
                            "Carry the changes across as they are. Git refuses if they would be overwritten.",
                        )
                        .clicked()
                            || ui.input(|i| i.key_pressed(Key::Num2))
                        {
                            actions.push(Action::Checkout(target.clone(), LocalChanges::Keep));
                            close = true;
                        }
                        if target.detaches() {
                            ui.add_space(10.0);
                            detach_note(ui);
                        }
                    });
                    footer(ui, &[("1", "stash"), ("2", "keep"), ("esc", "cancel")], |ui| {
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::Checkout(picker) => {
                    if let Some(action) = checkout_picker(ui, picker, branches, tags, repo, has_changes, &mut close) {
                        actions.push(action);
                    }
                }
                Dialog::NewBranch { name, start, checkout } => {
                    let from = start.as_ref().map_or("HEAD", |s| s.label.as_str());
                    header(ui, Glyph::Branch, Tone::Calm, "New branch", |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            dim(ui, "Starting at");
                            chip(ui, from);
                        });
                    });
                    let mut enter = false;
                    body(ui, |ui| {
                        field_label(ui, "Name");
                        let edit = ui.add(field(egui::TextEdit::singleline(name).hint_text("feature/my-idea")));
                        edit.request_focus();
                        if edit.changed() && name.contains(' ') {
                            // Branch names cannot hold spaces; most people mean a dash.
                            *name = name.replace(' ', "-");
                        }
                        enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if let Some(problem) = branch_name_problem(name.trim()) {
                            ui.add_space(6.0);
                            ui.label(RichText::new(problem).size(12.0).color(Palette::of(ui.visuals()).warn));
                        }
                        ui.add_space(12.0);
                        ui.checkbox(checkout, "Check out the new branch");
                    });
                    footer(ui, &[("↵", "create"), ("esc", "cancel")], |ui| {
                        let ok = !name.trim().is_empty() && branch_name_problem(name.trim()).is_none();
                        if theme::primary_button(ui, ok, "Create").clicked() || (ok && enter) {
                            actions.push(Action::CreateBranch {
                                name: name.trim().to_owned(),
                                start: start.as_ref().map(|s| s.rev.clone()),
                                checkout: *checkout,
                            });
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::GroupName { rename, name, repo } => {
                    let title = if rename.is_some() { "Rename group" } else { "New group" };
                    header(ui, Glyph::Folder, Tone::Calm, title, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            match (rename.as_deref(), repo.as_deref()) {
                                (Some(old), _) => {
                                    dim(ui, "Renaming");
                                    chip(ui, old);
                                }
                                (None, Some(repo)) => {
                                    chip(ui, &dir_name(repo));
                                    dim(ui, "goes into the new group.");
                                }
                                (None, None) => {
                                    dim(ui, "Groups keep related repositories together. Drag repositories onto one.")
                                }
                            }
                        });
                    });
                    let mut enter = false;
                    let problem = repos::group_name_problem(groups, name.trim(), rename.as_deref());
                    body(ui, |ui| {
                        field_label(ui, "Name");
                        let edit = ui.add(field(egui::TextEdit::singleline(name).hint_text("Work")));
                        edit.request_focus();
                        enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if let Some(problem) = problem {
                            ui.add_space(6.0);
                            ui.label(RichText::new(problem).size(12.0).color(Palette::of(ui.visuals()).warn));
                        }
                    });
                    let verb = if rename.is_some() { "Rename" } else { "Create" };
                    footer(ui, &[("↵", "save"), ("esc", "cancel")], |ui| {
                        let ok = !name.trim().is_empty() && problem.is_none();
                        if theme::primary_button(ui, ok, verb).clicked() || (ok && enter) {
                            actions.push(Action::SaveGroup {
                                rename: rename.clone(),
                                name: name.trim().to_owned(),
                                repo: repo.clone(),
                            });
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::DeleteBranch { name, force, upstream, with_upstream } => {
                    header(ui, Glyph::Trash, Tone::Danger, "Delete branch?", |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            chip(ui, name);
                            dim(ui, "goes away. Its commits stay wherever else they are reachable.");
                        });
                    });
                    body(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        ui.checkbox(force, "Delete even if it is not fully merged");
                        if let Some(up) = upstream.as_ref() {
                            ui.checkbox(with_upstream, format!("Also delete {} on the remote", up.label()));
                            if *with_upstream {
                                callout(ui, Tone::Danger, "Removing it from the remote affects everyone who uses it.");
                            }
                        }
                    });
                    footer(ui, &[("esc", "keep it")], |ui| {
                        if theme::danger_button(ui, true, "Delete branch").clicked() {
                            actions.push(Action::DeleteBranch {
                                name: name.clone(),
                                force: *force,
                                upstream: upstream.clone().filter(|_| *with_upstream),
                            });
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::DeleteRemoteBranch(r) => {
                    header(ui, Glyph::Trash, Tone::Danger, &format!("Delete branch from {}?", r.remote), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            chip(ui, &r.label());
                            dim(
                                ui,
                                "is removed from the remote for everyone who uses it. Local branches stay as they are.",
                            );
                        });
                    });
                    ui.add_space(6.0);
                    footer(ui, &[("esc", "keep it")], |ui| {
                        if theme::danger_button(ui, true, &format!("Delete from {}", r.remote)).clicked() {
                            actions.push(Action::DeleteRemoteBranch(r.clone()));
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::ConfirmDeleteFiles(entries) => {
                    let trash = snout_core::can_trash();
                    let what = match entries.len() {
                        1 => "This file".to_owned(),
                        n => format!("These {n} files"),
                    };
                    let (title, text, button) = if trash {
                        (
                            "Move to the Trash?",
                            format!("{what} go to the Trash, where you can still get them back."),
                            "Move to Trash",
                        )
                    } else {
                        ("Delete files?", format!("{what} are deleted from disk for good."), "Delete")
                    };
                    header(ui, Glyph::Trash, Tone::Danger, title, |ui| {
                        dim(ui, &format!("{text} Tracked files then show as deleted, ready to commit."));
                    });
                    body(ui, |ui| {
                        well(ui, 200.0, |ui| {
                            for e in entries.iter() {
                                file_line(ui, e);
                            }
                        });
                    });
                    footer(ui, &[("esc", "keep them")], |ui| {
                        if theme::danger_button(ui, true, button).clicked() {
                            actions.push(Action::DeleteFiles(entries.clone()));
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::InitRepo(path) => {
                    header(ui, Glyph::Open, Tone::Calm, "Not a Git repository", |ui| {
                        ui.label(
                            RichText::new(path.display().to_string())
                                .monospace()
                                .size(12.0)
                                .color(Palette::of(ui.visuals()).fg_dim),
                        );
                    });
                    body(ui, |ui| {
                        if choice(
                            ui,
                            1,
                            Glyph::Search,
                            "Find repositories inside",
                            "Look through its subfolders and add every repository found.",
                        )
                        .clicked()
                            || ui.input(|i| i.key_pressed(Key::Num1))
                        {
                            actions.push(Action::Scan(path.clone()));
                            close = true;
                        }
                        ui.add_space(8.0);
                        if choice(
                            ui,
                            2,
                            Glyph::Spark,
                            "Initialize a repository here",
                            "Start a fresh history in this folder with git init.",
                        )
                        .clicked()
                            || ui.input(|i| i.key_pressed(Key::Num2))
                        {
                            actions.push(Action::InitRepo(path.clone()));
                            close = true;
                        }
                    });
                    footer(ui, &[("1", "find"), ("2", "initialize"), ("esc", "cancel")], |ui| {
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
                Dialog::Clone { url, parent } => {
                    header(ui, Glyph::Fetch, Tone::Calm, "Clone repository", |ui| {
                        dim(ui, "Copy a remote repository to this computer and open it.");
                    });
                    let dest = repo_dir_name(url).map(|name| Path::new(parent.as_str()).join(name));
                    let mut enter = false;
                    body(ui, |ui| {
                        field_label(ui, "URL");
                        let edit = ui
                            .add(field(egui::TextEdit::singleline(url).hint_text("https://github.com/owner/repo.git")));
                        if url.is_empty() && !edit.has_focus() && ui.memory(|m| m.focused().is_none()) {
                            edit.request_focus();
                        }
                        enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.add_space(12.0);
                        field_label(ui, "Into folder");
                        ui.horizontal(|ui| {
                            let choose = vec2(92.0, 0.0);
                            ui.add(
                                field(egui::TextEdit::singleline(parent))
                                    .desired_width(ui.available_width() - choose.x - 8.0),
                            );
                            if theme::plain_button(ui, "Choose…").clicked()
                                && let Some(dir) = rfd::FileDialog::new().pick_folder()
                            {
                                *parent = dir.display().to_string();
                            }
                        });
                        if let Some(dest) = &dest {
                            ui.add_space(10.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                dim(ui, "Lands in");
                                chip(ui, &dest.display().to_string());
                            });
                        }
                    });
                    footer(ui, &[("↵", "clone"), ("esc", "cancel")], |ui| {
                        let ok = dest.is_some() && !parent.trim().is_empty();
                        if theme::primary_button(ui, ok, "Clone").clicked() || (ok && enter) {
                            actions.push(Action::Clone { url: url.trim().to_owned(), dest: dest.clone().unwrap() });
                            close = true;
                        }
                        close |= theme::plain_button(ui, "Cancel").clicked();
                    });
                }
            }
        });
        if close || modal.should_close() {
            self.dialog = None;
        }
    }
}

// ----- The card every dialog is drawn on ----------------------------------------------------

const RADIUS: u8 = 14;
const SIDE: i8 = 24;

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Calm,
    Warn,
    Danger,
}

impl Tone {
    fn color(self, p: &Palette) -> Color32 {
        match self {
            Self::Calm => p.ok,
            Self::Warn => p.warn,
            Self::Danger => p.error,
        }
    }
}

/// The title row: a tinted badge with the dialog's glyph, the title and a line under it.
fn header(ui: &mut Ui, glyph: Glyph, tone: Tone, title: &str, subtitle: impl FnOnce(&mut Ui)) {
    let p = Palette::of(ui.visuals());
    egui::Frame::new().inner_margin(Margin { left: SIDE, right: SIDE, top: 22, bottom: 14 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            badge(ui, glyph, tone);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                ui.add_space(2.0);
                ui.label(RichText::new(title).size(18.0).strong().color(p.fg));
                subtitle(ui);
            });
        });
    });
}

fn badge(ui: &mut Ui, glyph: Glyph, tone: Tone) {
    let p = Palette::of(ui.visuals());
    let color = tone.color(p);
    let (rect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        12.0,
        mix(color, 0.14, p.card),
        Stroke::new(1.0, mix(color, 0.3, p.card)),
        egui::StrokeKind::Inside,
    );
    let ink = if tone == Tone::Calm { p.fg } else { color };
    glyph.drawing().paint(painter, rect.shrink(10.0), Some(ink));
    let spark = Rect::from_center_size(rect.right_top() + vec2(-2.0, 3.0), vec2(13.0, 13.0));
    painter.circle_filled(spark.center(), 5.0, p.card);
    art::sparkle().paint(painter, spark, Some(color));
}

fn body<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .inner_margin(Margin { left: SIDE, right: SIDE, top: 2, bottom: 20 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            add(ui)
        })
        .inner
}

/// The band at the bottom: key hints on the left, buttons on the right (add the main one
/// first; they lay out right to left).
fn footer<R>(ui: &mut Ui, keys: &[(&str, &str)], add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::of(ui.visuals());
    let r = RADIUS - 1;
    let frame = egui::Frame::new()
        .fill(mix(p.fg, if ui.visuals().dark_mode { 0.025 } else { 0.03 }, p.card))
        .corner_radius(CornerRadius { nw: 0, ne: 0, sw: r, se: r })
        .inner_margin(Margin::symmetric(SIDE, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                for (key, what) in keys {
                    keycap(ui, key);
                    ui.label(RichText::new(*what).size(12.0).color(p.fg_dim));
                    ui.add_space(8.0);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    add(ui)
                })
                .inner
            })
            .inner
        });
    let rect = frame.response.rect;
    ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(1.0, p.border));
    frame.inner
}

fn keycap(ui: &mut Ui, key: &str) {
    let p = Palette::of(ui.visuals());
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, mix(p.fg, 0.16, p.card)))
        .corner_radius(5)
        .inner_margin(Margin::symmetric(5, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(key).monospace().size(11.5).color(p.fg_dim));
        });
}

fn dim(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(Palette::of(ui.visuals()).fg_dim));
}

/// A name in a soft pill: a branch, a revision, a path.
fn chip(ui: &mut Ui, text: &str) {
    let p = Palette::of(ui.visuals());
    egui::Frame::new().fill(mix(p.fg, 0.07, p.card)).corner_radius(255).inner_margin(Margin::symmetric(8, 1)).show(
        ui,
        |ui| {
            ui.add(egui::Label::new(RichText::new(text).monospace().color(p.fg)).wrap());
        },
    );
}

fn field_label(ui: &mut Ui, text: &str) {
    let p = Palette::of(ui.visuals());
    ui.label(RichText::new(text).size(12.0).strong().color(p.fg_dim));
}

/// A roomier text field.
fn field(edit: egui::TextEdit<'_>) -> egui::TextEdit<'_> {
    edit.margin(vec2(10.0, 7.0)).desired_width(f32::INFINITY)
}

/// A recessed box for lists and logs.
fn well(ui: &mut Ui, max_height: f32, add: impl FnOnce(&mut Ui)) {
    let p = Palette::of(ui.visuals());
    egui::Frame::new()
        .fill(p.input)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                add(ui);
            });
        });
}

fn code_box(ui: &mut Ui, text: &str, max_height: f32) {
    well(ui, max_height, |ui| {
        ui.add(egui::Label::new(RichText::new(text).monospace().size(12.0)).wrap());
    });
}

fn git_details(ui: &mut Ui, details: &str) {
    egui::CollapsingHeader::new(RichText::new("Details from git").size(12.0).color(Palette::of(ui.visuals()).fg_dim))
        .show(ui, |ui| code_box(ui, details, 160.0));
}

/// A tinted note with a warning glyph.
fn callout(ui: &mut Ui, tone: Tone, text: &str) {
    let p = Palette::of(ui.visuals());
    let color = tone.color(p);
    egui::Frame::new()
        .fill(mix(color, 0.1, p.card))
        .stroke(Stroke::new(1.0, mix(color, 0.35, p.card)))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                Glyph::Alert.drawing().paint(ui.painter(), rect, Some(color));
                ui.add(egui::Label::new(RichText::new(text).color(p.fg)).wrap());
            });
        });
}

/// One of a few ways forward, as a card: glyph, title, a line about what it does, and the
/// number key that picks it.
fn choice(ui: &mut Ui, key: usize, glyph: Glyph, title: &str, detail: &str) -> egui::Response {
    let p = Palette::of(ui.visuals());
    let width = ui.available_width();
    let (pad, icon, gap, cap) = (vec2(14.0, 12.0), 20.0, 12.0, 22.0);
    let text_w = width - pad.x * 2.0 - icon - gap * 2.0 - cap;
    let title_g = ui.painter().layout(title.to_owned(), FontId::proportional(14.5), p.fg, text_w);
    let detail_g = ui.painter().layout(detail.to_owned(), FontId::proportional(12.5), p.fg_dim, text_w);
    let height = pad.y * 2.0 + title_g.size().y + 3.0 + detail_g.size().y;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if ui.is_rect_visible(rect) {
        let hot = resp.hovered() || resp.has_focus();
        let (fill, stroke) =
            if hot { (mix(p.ok, 0.08, p.card), mix(p.ok, 0.55, p.card)) } else { (mix(p.fg, 0.025, p.card), p.border) };
        let painter = ui.painter();
        painter.rect(rect, 10.0, fill, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
        let icon_rect = Rect::from_min_size(rect.min + pad + vec2(0.0, 0.0), vec2(icon, icon));
        glyph.drawing().paint(painter, icon_rect, Some(p.fg));
        let x = icon_rect.right() + gap;
        let title_h = title_g.size().y;
        painter.galley(egui::pos2(x, rect.top() + pad.y), title_g, p.fg);
        painter.galley(egui::pos2(x, rect.top() + pad.y + title_h + 3.0), detail_g, p.fg_dim);
        let cap_rect =
            Rect::from_center_size(egui::pos2(rect.right() - pad.x - cap / 2.0, rect.center().y), vec2(cap, cap));
        painter.rect(
            cap_rect,
            6.0,
            if hot { p.card } else { Color32::TRANSPARENT },
            Stroke::new(1.0, mix(p.fg, if hot { 0.3 } else { 0.16 }, p.card)),
            egui::StrokeKind::Inside,
        );
        painter.text(
            cap_rect.center(),
            egui::Align2::CENTER_CENTER,
            key.to_string(),
            FontId::monospace(11.5),
            if hot { p.fg } else { p.fg_dim },
        );
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One file in a list: a dot in its state colour, the name, the folder dimmed.
fn file_line(ui: &mut Ui, e: &FileEntry) {
    let p = Palette::of(ui.visuals());
    let color = state_color(e, ui.visuals());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 3.5, color);
        let (dir, name) = e.path.rsplit_once('/').map_or(("", e.path.as_str()), |(d, n)| (d, n));
        ui.label(RichText::new(name).color(p.fg));
        if !dir.is_empty() {
            ui.label(RichText::new(dir).size(12.0).color(p.fg_dim));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(e.state_label()).size(12.0).color(color));
        });
    });
}

fn detach_note(ui: &mut Ui) {
    callout(ui, Tone::Warn, "This leaves HEAD detached. Create a branch if you want to keep new commits.");
}

/// The searchable branch / tag / commit picker. Returns the checkout to perform.
fn checkout_picker(
    ui: &mut Ui,
    p: &mut CheckoutPicker,
    branches: &[Branch],
    tags: &[Tag],
    repo: Option<&snout_core::Repo>,
    has_changes: bool,
    close: &mut bool,
) -> Option<Action> {
    let pal = Palette::of(ui.visuals());
    header(ui, Glyph::Switch, Tone::Calm, "Check out", |ui| {
        dim(ui, "A branch, a tag, or any commit.");
    });

    let query = p.query.trim().to_owned();
    if p.preview.as_ref().map(|(q, _)| q) != Some(&query) {
        let described = repo.filter(|_| !query.is_empty()).and_then(|r| r.describe_commit(&query).ok());
        p.preview = Some((query.clone(), described));
    }
    let commit = p.preview.as_ref().and_then(|(_, d)| d.as_deref()).map(str::to_owned);
    let mut items = pick_items(branches, tags, &query, commit.as_deref());

    let (down, up, enter) =
        ui.input(|i| (i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::Enter)));
    let mut chosen = None;

    body(ui, |ui| {
        let edit = egui::Frame::new()
            .fill(pal.input)
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(10)
            .inner_margin(Margin { left: 10, right: 6, top: 2, bottom: 2 })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    // Lined up with the text, which sits a little above the field's middle.
                    Glyph::Search.drawing().paint(ui.painter(), rect.translate(vec2(0.0, -2.0)), Some(pal.fg_dim));
                    ui.add(
                        egui::TextEdit::singleline(&mut p.query)
                            .hint_text("Filter branches and tags, or enter a commit (SHA, HEAD~2, …)")
                            .frame(egui::Frame::NONE)
                            .margin(vec2(4.0, 7.0))
                            .desired_width(f32::INFINITY),
                    )
                })
                .inner
            })
            .inner;
        if !p.focused {
            edit.request_focus();
            p.focused = true;
        }
        if edit.changed() {
            p.cursor = 0;
        }
        if down {
            p.cursor += 1;
        }
        if up {
            p.cursor = p.cursor.saturating_sub(1);
        }
        p.cursor = p.cursor.min(items.len().saturating_sub(1));
        chosen = enter.then_some(p.cursor).filter(|_| !items.is_empty());

        ui.add_space(10.0);
        egui::ScrollArea::vertical().max_height(300.0).min_scrolled_height(240.0).auto_shrink([false, false]).show(
            ui,
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let mut section = "";
                for (i, item) in items.iter().enumerate() {
                    if item.section != section {
                        section = item.section;
                        ui.add_space(if i == 0 { 2.0 } else { 10.0 });
                        ui.label(RichText::new(section.to_uppercase()).size(10.5).strong().color(pal.fg_dim));
                        ui.add_space(2.0);
                    }
                    let r = pick_row(ui, item, i == p.cursor);
                    if (up || down) && i == p.cursor {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        p.cursor = i;
                    }
                    if r.double_clicked() {
                        chosen = Some(i);
                    }
                }
                if items.is_empty() {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        art::pick().show(ui, vec2(150.0, 100.0));
                        ui.add_space(4.0);
                        dim(ui, "No matching branch, tag, or commit");
                    });
                }
            },
        );

        if has_changes {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                dim(ui, "Uncommitted changes:");
                ui.add_space(4.0);
                segment(ui, &mut p.local, LocalChanges::Keep, "Keep them", "Git refuses if they would be overwritten");
                segment(ui, &mut p.local, LocalChanges::Stash, "Stash and re-apply", "Stash, check out, then re-apply");
            });
        }
        if items.get(p.cursor).is_some_and(|i| i.target.detaches()) {
            ui.add_space(10.0);
            detach_note(ui);
        }
    });

    footer(ui, &[("↑↓", "choose"), ("↵", "check out"), ("esc", "cancel")], |ui| {
        if theme::primary_button(ui, !items.is_empty(), "Check Out").clicked() {
            chosen = Some(p.cursor);
        }
        *close |= theme::plain_button(ui, "Cancel").clicked();
    });

    let chosen = chosen?;
    if chosen >= items.len() {
        return None;
    }
    let item = items.swap_remove(chosen);
    *close = true;
    Some(Action::Checkout(item.target, p.local))
}

/// A picker row: glyph, name, and the tip commit dimmed beside it.
fn pick_row(ui: &mut Ui, item: &PickItem, selected: bool) -> egui::Response {
    let p = Palette::of(ui.visuals());
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, 8.0, mix(p.ok, 0.12, p.card));
            let bar = Rect::from_min_size(rect.min + vec2(0.0, 7.0), vec2(3.0, rect.height() - 14.0));
            painter.rect_filled(bar, 2.0, p.ok);
        } else if resp.hovered() {
            painter.rect_filled(rect, 8.0, mix(p.fg, 0.05, p.card));
        }
        let icon = Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), vec2(16.0, 16.0));
        item.glyph.drawing().paint(painter, icon, Some(if selected { p.fg } else { p.fg_dim }));
        let x = icon.right() + 10.0;
        let label = painter.layout_no_wrap(item.label.clone(), FontId::proportional(13.5), p.fg);
        let label_w = label.size().x;
        painter.galley(egui::pos2(x, rect.center().y - label.size().y / 2.0), label, p.fg);
        let room = rect.right() - 12.0 - (x + label_w + 12.0);
        if room > 40.0 {
            let mut job =
                egui::text::LayoutJob::simple_singleline(item.detail.clone(), FontId::proportional(12.0), p.fg_dim);
            job.wrap = egui::text::TextWrapping::truncate_at_width(room);
            let detail = painter.layout_job(job);
            painter.galley(egui::pos2(x + label_w + 12.0, rect.center().y - detail.size().y / 2.0), detail, p.fg_dim);
        }
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, &item.label));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One half of a two-way switch.
fn segment<T: PartialEq>(ui: &mut Ui, current: &mut T, value: T, text: &str, hint: &str) {
    let p = Palette::of(ui.visuals());
    let on = *current == value;
    let button = egui::Button::new(RichText::new(text).color(if on { p.fg } else { p.fg_dim }))
        .fill(if on { mix(p.ok, 0.14, p.card) } else { Color32::TRANSPARENT })
        .stroke(Stroke::new(1.0, if on { mix(p.ok, 0.5, p.card) } else { p.border }))
        .corner_radius(255);
    if ui.add(button).on_hover_text(hint).clicked() {
        *current = value;
    }
}

#[cfg(test)]
mod tests {
    use super::branch_name_problem;

    #[test]
    fn spots_bad_branch_names() {
        for good in ["main", "feature/graph", "fix-12", "v1.2-rc"] {
            assert_eq!(branch_name_problem(good), None, "{good}");
        }
        for bad in ["-x", "a..b", "a@{b", "what?", "a:b", "dir/", "x.lock", "/x", "end."] {
            assert!(branch_name_problem(bad).is_some(), "{bad}");
        }
    }
}

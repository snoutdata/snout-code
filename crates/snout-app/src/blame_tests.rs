//! Annotate with Git Blame in the window: the column over a repository with several authors,
//! its lines following typing, a click lighting a commit and selecting it in the Git view,
//! the card and its previous revision, the author hints, the current line's blame, and a
//! folder without Git.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};
use gpui_component::input::RopeExt as _;

use crate::editor::{EditorPanel, Row};
use crate::tests::{open, scratch_dir, with_repo};
use crate::workspace::Workspace;

/// The file the fixture's history is about.
pub const FILE: &str = "src/main/java/com/example/inventory/Stock.java";

const V1: &str = "package com.example.inventory;

public class Inventory {
    private int count;

    public void add(int n) {
        count += n;
    }
}
";

const V2: &str = "package com.example.inventory;

import java.util.Objects;

public class Stock {
    private int count;
    private final String sku;

    public Stock(String sku) {
        this.sku = Objects.requireNonNull(sku);
    }

    public void add(int n) {
        count += n;
    }
}
";

const V3: &str = "package com.example.inventory;

import java.util.Objects;

public class Stock {
    private int count;
    private final String sku;

    public Stock(String sku) {
        this.sku = Objects.requireNonNull(sku);
    }

    public void add(int n) {
        count += n;
    }

    public boolean remove(int n) {
        if (n > count) {
            return false;
        }
        count -= n;
        return true;
    }
}
";

const V4: &str = "package com.example.inventory;

import java.util.Objects;

public class Stock {
    private int count;
    private final String sku;

    public Stock(String sku) {
        this.sku = Objects.requireNonNull(sku);
    }

    public void add(int n) {
        if (n < 0) {
            throw new IllegalArgumentException(\"Negative amount: \" + n);
        }
        count += n;
    }

    public boolean remove(int n) {
        if (n > count) {
            return false;
        }
        count -= n;
        return true;
    }
}
";

fn git_as(dir: &Path, who: (&str, &str), date: &str, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", who.0)
        .env("GIT_AUTHOR_EMAIL", who.1)
        .env("GIT_COMMITTER_NAME", who.0)
        .env("GIT_COMMITTER_EMAIL", who.1)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

/// A repository whose one Java file four people wrote over five years: Maya Chen started it
/// (as Inventory.java), Tomás Rivera renamed it to Stock.java and keyed it by SKU,
/// Priya Natarajan added `remove`, Sam Okafor checked `add`'s amount.
pub fn blame_repo(dir: &Path) -> PathBuf {
    let maya = ("Maya Chen", "maya@example.com");
    let tomas = ("Tom\u{e1}s Rivera", "tomas@example.com");
    let priya = ("Priya Natarajan", "priya@example.com");
    let sam = ("Sam Okafor", "sam@example.com");
    let old = "src/main/java/com/example/inventory/Inventory.java";
    std::fs::create_dir_all(dir.join("src/main/java/com/example/inventory")).unwrap();
    git_as(dir, maya, "2021-03-02T10:00:00", &["init", "-q", "-b", "main"]);
    git_as(dir, maya, "2021-03-02T10:00:00", &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join(old), V1).unwrap();
    git_as(dir, maya, "2021-03-02T10:00:00", &["add", "-A"]);
    git_as(
        dir,
        maya,
        "2021-03-02T10:00:00",
        &["commit", "-qm", "Count what is in stock\n\nA first inventory: one count, and a way to add to it."],
    );
    git_as(dir, tomas, "2023-06-14T15:30:00", &["mv", old, FILE]);
    std::fs::write(dir.join(FILE), V1.replace("class Inventory", "class Stock")).unwrap();
    git_as(dir, tomas, "2023-06-14T15:30:00", &["commit", "-qam", "Rename Inventory to Stock"]);
    std::fs::write(dir.join(FILE), V2).unwrap();
    git_as(
        dir,
        tomas,
        "2023-06-15T11:05:00",
        &["commit", "-qam", "Key stock by SKU\n\nEach stock item now knows its SKU; a null one is refused at once."],
    );
    std::fs::write(dir.join(FILE), V3).unwrap();
    git_as(
        dir,
        priya,
        "2025-01-20T09:12:00",
        &["commit", "-qam", "Take items out of stock\n\nremove() says whether there was enough to take."],
    );
    std::fs::write(dir.join(FILE), V4).unwrap();
    git_as(dir, sam, "2026-09-30T17:45:00", &["commit", "-qam", "Refuse negative amounts in add"]);
    dir.join(FILE)
}

fn pump(cx: &mut VisualTestContext, ms: u64) {
    for _ in 0..ms.div_ceil(16) {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(16));
        std::thread::sleep(Duration::from_millis(1));
    }
    cx.run_until_parked();
}

fn pump_until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let start = Instant::now();
    while !done(cx) {
        assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for {what}");
        pump(cx, 32);
    }
}

fn open_file(ws: &Entity<Workspace>, path: &Path, cx: &mut VisualTestContext) -> Entity<EditorPanel> {
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(path.to_owned(), None, false, w, cx)));
    let mut found = None;
    pump_until(cx, "the editor", |cx| {
        found = cx.update(|_, cx| {
            ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path && !e.read(cx).loading)
        });
        found.is_some()
    });
    found.unwrap()
}

fn labels(e: &Entity<EditorPanel>, cx: &mut VisualTestContext) -> Vec<(usize, String)> {
    cx.update(|_, cx| e.read(cx).annotation_labels(cx))
}

fn author_of(e: &Entity<EditorPanel>, line: usize, cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| e.read(cx).blame_commit(line).map(|c| c.author.clone()))
}

#[gpui::test]
fn the_column_says_who_changed_each_line_and_follows_typing(cx: &mut TestAppContext) {
    let repo = scratch_dir("blame-column");
    let file = blame_repo(&repo);
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let e = open_file(&ws, &file, cx);
    cx.update(|w, cx| w.dispatch_action(Box::new(crate::editor::AnnotateWithGitBlame), cx));
    pump_until(cx, "the blame", |cx| {
        cx.update(|_, cx| e.read(cx).blame.data.is_some() && !e.read(cx).blame.map.is_empty())
    });
    pump(cx, 200);
    assert!(cx.update(|_, cx| e.read(cx).blame.shown), "the action opened the column");
    // Each line has its author, the rename followed back to Maya's first lines.
    assert_eq!(author_of(&e, 0, cx).as_deref(), Some("Maya Chen"), "the package line came with the file");
    assert_eq!(author_of(&e, 4, cx).as_deref(), Some("Tom\u{e1}s Rivera"), "the class renamed");
    assert_eq!(author_of(&e, 13, cx).as_deref(), Some("Sam Okafor"));
    assert_eq!(author_of(&e, 20, cx).as_deref(), Some("Priya Natarajan"));
    // One label per run of lines from one commit.
    let l = labels(&e, cx);
    assert!(l.iter().any(|(line, s)| *line == 13 && s.ends_with("Sam Okafor") && s.starts_with("2026-09-30")), "{l:?}");
    let runs = l.iter().filter(|(_, s)| s.ends_with("Sam Okafor")).count();
    assert_eq!(runs, 1, "Sam's three lines read once: {l:?}");
    // The column took room left of the line numbers, and drew its rows.
    let (width, rows) = cx.update(|_, cx| (e.read(cx).blame_width(cx), e.read(cx).blame.painted.borrow().rows.len()));
    assert!(width > 80., "{width}");
    assert!(rows > 10, "{rows} rows drawn");
    assert!(
        cx.update(|_, cx| e.read(cx).lanes.get().bounds.left()
            >= e.read(cx).blame.painted.borrow().column.right() - gpui::px(1.))
    );

    // Typing a new line before Sam's: it is not committed, Sam's lines move down at once.
    cx.update(|w, cx| {
        let state = e.read(cx).state.clone();
        state.update(cx, |s, cx| {
            let at = s.text().line_start_offset(13);
            s.set_selected_range(at..at, cx);
            s.replace("        // Checked first.\n", w, cx);
        })
    });
    cx.run_until_parked();
    let rows: Vec<Row> = cx.update(|_, cx| (12..16).map(|l| e.read(cx).blame_row(l)).collect());
    assert_eq!(rows[1], Row::Uncommitted, "{rows:?}");
    assert_eq!(author_of(&e, 14, cx).as_deref(), Some("Sam Okafor"), "moved down with the text");
    pump(cx, 400);
    assert_eq!(author_of(&e, 14, cx).as_deref(), Some("Sam Okafor"), "and the settled diff agrees");
    assert!(labels(&e, cx).iter().any(|(l, s)| *l == 13 && s == "Not committed yet"));
    // Deleting it again: back as committed.
    cx.update(|w, cx| {
        let state = e.read(cx).state.clone();
        state.update(cx, |s, cx| {
            let (a, b) = (s.text().line_start_offset(13), s.text().line_start_offset(14));
            s.set_selected_range(a..b, cx);
            s.replace("", w, cx);
        })
    });
    cx.run_until_parked();
    assert_eq!(author_of(&e, 13, cx).as_deref(), Some("Sam Okafor"));

    // A click on a line's annotation lights its commit and selects it in the Git view.
    let at = cx
        .update(|_, cx| e.read(cx).blame.painted.borrow().rows.iter().find(|(l, _)| *l == 20).map(|(_, b)| b.center()));
    cx.simulate_click(at.expect("line 20 drawn"), gpui::Modifiers::none());
    pump_until(cx, "the commit selected", |cx| {
        cx.update(|_, cx| model.read(cx).commit_view.as_ref().and_then(|v| v.id().map(str::to_owned)))
            .is_some_and(|id| cx.update(|_, cx| e.read(cx).blame.selected.as_deref() == Some(id.as_str())))
    });
    assert!(cx.update(|_, cx| e.read(cx).blame_commit(20).is_some_and(|c| c.summary == "Take items out of stock")));

    // The card, by its line, and its previous revision: the file before Priya's commit.
    cx.update(|_, cx| {
        e.update(cx, |e, cx| {
            e.blame.selected = None;
            e.blame.card =
                Some(crate::editor::blame::LineCard { line: 20, at: gpui::point(gpui::px(200.), gpui::px(200.)) });
            cx.notify();
        })
    });
    pump(cx, 100);
    for b in ["blame-card", "blame-card-diff", "blame-card-log", "blame-card-copy", "blame-card-previous"] {
        assert!(cx.debug_bounds(b).is_some(), "{b}");
    }
    let c = cx.debug_bounds("blame-card-previous").unwrap().center();
    cx.simulate_click(c, gpui::Modifiers::none());
    pump_until(cx, "the previous revision", |cx| {
        cx.update(|_, cx| e.read(cx).blame.revisions.last().is_some_and(|r| r.data.is_some()))
    });
    pump(cx, 200);
    let rev_text = cx.update(|_, cx| e.read(cx).revision_state().unwrap().read(cx).value().to_string());
    assert_eq!(rev_text, V2, "the file as Tomás left it");
    assert!(cx.debug_bounds("blame-revision-bar").is_some());
    assert_eq!(author_of(&e, 4, cx).as_deref(), Some("Tom\u{e1}s Rivera"));
    // Further back: before Tomás's commit, under the file's old name.
    let tomas = cx.update(|_, cx| e.read(cx).blame_commit(4).cloned()).unwrap();
    assert!(tomas.previous.as_ref().is_some_and(|(_, p)| p.ends_with("Inventory.java")), "{:?}", tomas.previous);
    cx.update(|w, cx| e.update(cx, |e, cx| e.annotate_previous(&tomas, w, cx)));
    pump_until(cx, "the first revision", |cx| {
        cx.update(|_, cx| e.read(cx).blame.revisions.len() == 2 && e.read(cx).blame.revisions[1].data.is_some())
    });
    pump(cx, 100);
    assert_eq!(cx.update(|_, cx| e.read(cx).revision_state().unwrap().read(cx).value().to_string()), V1);
    assert_eq!(author_of(&e, 2, cx).as_deref(), Some("Maya Chen"));
    let back = cx.debug_bounds("blame-crumb-0").expect("the crumb back to now").center();
    cx.simulate_click(back, gpui::Modifiers::none());
    pump(cx, 100);
    assert!(cx.update(|_, cx| e.read(cx).blame.revisions.is_empty()));

    // Show Diff and Show in Git Log go to the Git view.
    cx.update(|_, cx| {
        e.update(cx, |e, cx| {
            e.blame.card =
                Some(crate::editor::blame::LineCard { line: 13, at: gpui::point(gpui::px(200.), gpui::px(200.)) });
            cx.notify();
        })
    });
    pump(cx, 100);
    let diff = cx.debug_bounds("blame-card-diff").unwrap().center();
    cx.simulate_click(diff, gpui::Modifiers::none());
    pump_until(cx, "the commit's diff", |cx| {
        cx.update(|_, cx| model.read(cx).commit_view.as_ref().and_then(|v| v.selected.clone())).as_deref() == Some(FILE)
            && cx
                .update(|_, cx| {
                    model
                        .read(cx)
                        .commit_view
                        .as_ref()
                        .and_then(|v| v.details.as_ref().ok().map(|d| d.summary().to_owned()))
                })
                .as_deref()
                == Some("Refuse negative amounts in add")
    });

    // Close Annotations.
    cx.update(|_, cx| e.update(cx, |e, cx| e.toggle_blame(cx)));
    assert!(cx.update(|_, cx| !e.read(cx).blame.shown && e.read(cx).blame_width(cx) == 0.));
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn author_hints_and_the_lines_blame_come_from_the_same_data(cx: &mut TestAppContext) {
    let repo = scratch_dir("blame-hints");
    let file = blame_repo(&repo);
    let mut settings = with_repo(&repo);
    settings.blame_current_line = true;
    settings.hover_delay_ms = 100;
    let (_model, ws, cx) = open(cx, settings);
    let e = open_file(&ws, &file, cx);
    // On by default: the hints need the blame without the column.
    pump_until(cx, "the declarations", |cx| {
        cx.update(|_, cx| !e.read(cx).blame.decls.is_empty() && e.read(cx).blame.data.is_some())
    });
    pump(cx, 200);
    let hints: Vec<(String, Option<String>)> = cx.update(|_, cx| {
        let e = e.read(cx);
        let data = e.blame.data.clone().unwrap();
        e.blame
            .decls
            .iter()
            .map(|d| {
                let a = crate::editor::blame::authors_of(d.lines.clone(), |l| e.blame_row(l), &data.blame);
                (d.name.clone(), crate::editor::blame::hint_label(&a))
            })
            .collect()
    });
    assert!(
        hints.iter().any(|(n, h)| n == "Stock" && h.as_ref().is_some_and(|h| h.ends_with(" +3"))),
        "four authors: {hints:?}"
    );
    assert!(hints.contains(&("add".into(), Some("Sam Okafor +1".into()))), "{hints:?}");
    assert!(hints.contains(&("remove".into(), Some("Priya Natarajan".into()))), "{hints:?}");
    assert!(!cx.update(|_, cx| e.read(cx).blame.painted.borrow().hints.is_empty()), "the hints were drawn");
    // A click on a hint lists the authors.
    let at = cx.update(|_, cx| e.read(cx).blame.painted.borrow().hints.first().map(|(_, b)| b.center())).unwrap();
    cx.simulate_click(at, gpui::Modifiers::none());
    pump(cx, 100);
    assert!(cx.debug_bounds("blame-authors").is_some(), "the authors' list");
    // The caret rests on Sam's line: its blame after the end, once the delay is over.
    cx.update(|w, cx| {
        let state = e.read(cx).state.clone();
        state.update(cx, |s, cx| {
            let at = s.text().line_start_offset(14) + 4;
            s.set_selected_range(at..at, cx);
            s.focus(w, cx);
        })
    });
    pump_until(cx, "the line's blame", |cx| cx.update(|_, cx| e.read(cx).blame.line == Some(14)));
    let text = cx.update(|_, cx| e.read(cx).line_blame_text(14, cx)).unwrap();
    assert!(text.starts_with("Sam Okafor, ") && text.ends_with("\u{b7} Refuse negative amounts in add"), "{text}");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn a_folder_without_git_says_why_annotate_does_nothing(cx: &mut TestAppContext) {
    let dir = scratch_dir("blame-no-git");
    let file = dir.join("Notes.java");
    std::fs::write(&file, "class Notes {}\n").unwrap();
    let (_model, ws, cx) = open(cx, crate::settings::Settings::default());
    let e = open_file(&ws, &file, cx);
    assert!(cx.update(|_, cx| e.read(cx).blame_unavailable().is_some()));
    cx.update(|_, cx| e.update(cx, |e, cx| e.toggle_blame(cx)));
    pump(cx, 50);
    assert!(cx.update(|_, cx| !e.read(cx).blame.shown), "nothing opened");
    assert!(cx.update(|_, cx| e.read(cx).hint.as_deref().is_some_and(|h| h.starts_with("Not a Git project"))));
    // The gutter's menu shows the item greyed out, with the reason.
    cx.update(|w, cx| {
        e.update(cx, |e, cx| e.open_gutter_menu(gpui::point(gpui::px(20.), gpui::px(80.)), Some(0), w, cx))
    });
    pump(cx, 50);
    assert!(cx.update(|_, cx| e.read(cx).context.is_open()));
    let _ = std::fs::remove_dir_all(dir);
}

//! The file tree of the open repository, in the Projects panel: folders open and close,
//! a click previews a file in an editor tab and a double click keeps it open. Files carry
//! their Git state in colour (modified, added, untracked, conflicted), what `.gitignore`
//! leaves out is listed dimmed, and indent guides show the nesting. The repository is
//! watched (`notify`), so files made or deleted elsewhere show up at once.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;
use notify::Watcher as _;
use snout_core::FileEntry;
use snout_core::art::Glyph;

use crate::art::glyph;
use crate::look::{self, separator, title};
use crate::model::{Action, Model};
use crate::theme::{Palette, mix, palette};

pub const ROW: f32 = 22.0;
const INDENT: f32 = 12.0;

/// A file or folder in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub dir: bool,
    /// `.gitignore` leaves it out (or it sits in a folder that is left out).
    pub ignored: bool,
}

/// The folders and files directly in `dir`, folders first, each sorted by name. What
/// `.gitignore` leaves out is listed too, marked; the repository's own `.git` is not.
pub fn list_dir(dir: &Path) -> Vec<Entry> {
    let kept: HashSet<PathBuf> = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .require_git(false)
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.depth() == 1)
        .map(|e| e.into_path())
        .collect();
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Entry> = read
        .filter_map(Result::ok)
        .filter(|e| e.file_name() != ".git")
        .map(|e| {
            let path = e.path();
            let dir = e.file_type().is_ok_and(|t| t.is_dir() || (t.is_symlink() && path.is_dir()));
            Entry { ignored: !kept.contains(&path), path, dir }
        })
        .collect();
    out.sort_by(|a, b| {
        b.dir.cmp(&a.dir).then_with(|| {
            let (x, y) = (a.path.file_name().unwrap_or_default(), b.path.file_name().unwrap_or_default());
            x.to_string_lossy().to_lowercase().cmp(&y.to_string_lossy().to_lowercase())
        })
    });
    out
}

/// A file's Git state as the tree shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GitMark {
    Untracked,
    Added,
    Modified,
    Conflicted,
}

impl GitMark {
    pub fn of(f: &FileEntry) -> Self {
        use snout_core::Change;
        if f.conflicted {
            Self::Conflicted
        } else if f.is_untracked() {
            Self::Untracked
        } else if f.index == Some(Change::Added) && f.worktree.is_none() {
            Self::Added
        } else {
            Self::Modified
        }
    }

    pub fn color(self, p: &Palette) -> Hsla {
        match self {
            Self::Untracked => p.ill_accent,
            Self::Added => p.ok,
            // The light palette's accent is its ink, so modified takes the amber there.
            Self::Modified if p.dark => p.accent,
            Self::Modified => p.warn,
            Self::Conflicted => p.error,
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            Self::Untracked => "U",
            Self::Added => "A",
            Self::Modified => "M",
            Self::Conflicted => "C",
        }
    }
}

/// The Git state of every changed file, and of every folder holding one (the strongest
/// state below it), keyed by full path.
pub fn git_marks(root: &Path, files: &[FileEntry]) -> HashMap<PathBuf, GitMark> {
    let mut marks: HashMap<PathBuf, GitMark> = HashMap::new();
    for f in files {
        if f.worktree == Some(snout_core::Change::Deleted) || f.index == Some(snout_core::Change::Deleted) {
            continue;
        }
        let mark = GitMark::of(f);
        let path = f.path.trim_end_matches('/');
        marks.insert(root.join(path), mark);
        let mut end = path.len();
        while let Some(slash) = path[..end].rfind('/') {
            let dir = root.join(&path[..slash]);
            let m = marks.entry(dir).or_insert(mark);
            *m = (*m).max(mark);
            end = slash;
        }
    }
    marks
}

pub struct FileTree {
    model: Entity<Model>,
    root: Option<PathBuf>,
    /// The folders open.
    pub expanded: HashSet<PathBuf>,
    /// The file of the editor tab in front, shown selected.
    pub active: Option<PathBuf>,
    /// What each folder read holds, until the watcher says it changed.
    listing: HashMap<PathBuf, Vec<Entry>>,
    watcher: Option<notify::RecommendedWatcher>,
    events: Option<mpsc::Receiver<PathBuf>>,
}

impl FileTree {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |this, model, cx| {
            let root = model.read(cx).repo.as_ref().map(|r| r.root().to_owned());
            if root != this.root {
                this.set_root(root);
            }
            cx.notify();
        })
        .detach();
        // The watcher runs on its own thread; look for what it saw on a short timer.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                if this.update(cx, |this, cx| this.take_events(cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        let root = model.read(cx).repo.as_ref().map(|r| r.root().to_owned());
        let mut this = Self {
            model,
            root: None,
            expanded: HashSet::new(),
            active: None,
            listing: HashMap::new(),
            watcher: None,
            events: None,
        };
        this.set_root(root);
        this
    }

    fn set_root(&mut self, root: Option<PathBuf>) {
        self.listing.clear();
        self.expanded.clear();
        self.watcher = None;
        self.events = None;
        self.root = root.clone();
        let Some(root) = root else { return };
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let _ = tx.send(path);
                }
            }
        });
        if let Ok(mut watcher) = watcher
            && watcher.watch(&root, notify::RecursiveMode::Recursive).is_ok()
        {
            self.watcher = Some(watcher);
            self.events = Some(rx);
        }
    }

    /// Forget the listings of the folders the watcher saw change.
    fn take_events(&mut self, cx: &mut Context<Self>) {
        let Some(rx) = &self.events else { return };
        let mut changed = false;
        while let Ok(path) = rx.try_recv() {
            if path.components().any(|c| c.as_os_str() == ".git") {
                // A .gitignore or the index changed: what is ignored may have too.
                changed |= path.ends_with(".git/index") && !self.listing.is_empty();
                if changed {
                    self.listing.clear();
                }
                continue;
            }
            if path.ends_with(".gitignore") {
                self.listing.clear();
                changed = true;
            }
            if let Some(parent) = path.parent() {
                changed |= self.listing.remove(parent).is_some();
            }
            changed |= self.listing.remove(&path).is_some();
        }
        if changed {
            cx.notify();
        }
    }

    /// Read every folder again.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.listing.clear();
        cx.notify();
    }

    /// What `dir` holds, read now if it is not known.
    pub fn children(&mut self, dir: &Path) -> Vec<Entry> {
        self.listing.entry(dir.to_owned()).or_insert_with(|| list_dir(dir)).clone()
    }

    pub fn toggle(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if !self.expanded.remove(&dir) {
            self.expanded.insert(dir);
        }
        cx.notify();
    }

    /// Show `path` selected, its folders open.
    pub fn reveal(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.active == path {
            return;
        }
        if let (Some(path), Some(root)) = (&path, &self.root) {
            for dir in path.ancestors().skip(1) {
                if !dir.starts_with(root) || dir == root {
                    break;
                }
                self.expanded.insert(dir.to_owned());
            }
        }
        self.active = path;
        cx.notify();
    }

    /// The rows showing: every entry under an open folder, with its depth.
    pub fn rows(&mut self) -> Vec<(Entry, usize)> {
        let Some(root) = self.root.clone() else { return Vec::new() };
        let mut out = Vec::new();
        self.walk(&root, 0, false, &mut out);
        out
    }

    fn walk(&mut self, dir: &Path, depth: usize, ignored: bool, out: &mut Vec<(Entry, usize)>) {
        for mut e in self.children(dir) {
            e.ignored |= ignored;
            let open = e.dir && self.expanded.contains(&e.path);
            let (path, ignored) = (e.path.clone(), e.ignored);
            out.push((e, depth));
            if open {
                self.walk(&path, depth + 1, ignored, out);
            }
        }
    }
}

impl Render for FileTree {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let name = self.root.as_ref().map(|r| Model::dir_name(r));
        let rows = self.rows();
        let marks = match &self.root {
            Some(root) => git_marks(root, &self.model.read(cx).status.files),
            None => HashMap::new(),
        };
        let model = self.model.clone();
        let guide = mix(p.fg, 0.1, p.panel);
        let items: Vec<AnyElement> = rows
            .into_iter()
            .enumerate()
            .map(|(i, (e, depth))| {
                let label = e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let open = self.expanded.contains(&e.path);
                let selected = self.active.as_ref() == Some(&e.path);
                let mark = marks.get(&e.path).copied().filter(|_| !e.ignored);
                let ink = match mark {
                    _ if e.ignored => p.fg_dim.opacity(0.6),
                    Some(m) if e.dir => mix(m.color(&p), 0.8, p.fg),
                    Some(m) => m.color(&p),
                    None => p.fg,
                };
                let path = e.path.clone();
                let model = model.clone();
                let tip: SharedString = e.path.display().to_string().into();
                let hover_bg = p.button_hover;
                let selector = label.clone();
                div()
                    .id(("file-row", i))
                    .debug_selector(move || format!("file-{selector}"))
                    .relative()
                    .flex_none()
                    .h(px(ROW))
                    .pl(px(2. + depth as f32 * INDENT))
                    .pr(px(4.))
                    .flex()
                    .items_center()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .text_color(ink)
                    .when(selected, |d| d.bg(p.selection()))
                    .when(!selected, |d| d.hover(move |s| s.bg(hover_bg)))
                    // Indent guides: a hairline under each open folder above this row.
                    .children((0..depth).map(|d| {
                        div().absolute().top_0().bottom_0().left(px(2. + d as f32 * INDENT + 7.)).w(px(1.)).bg(guide)
                    }))
                    .child(
                        div().w(px(14.)).h_full().flex_none().when(e.dir, |d| d.child(look::chevron(open, p.fg_dim))),
                    )
                    .child(div().w(px(2.)))
                    .map(|d| {
                        if e.dir {
                            d.child(glyph(Glyph::Folder, 15., if e.ignored { ink } else { p.ill_ink }))
                        } else {
                            d.child(
                                div()
                                    .opacity(if e.ignored { 0.55 } else { 1. })
                                    .child(crate::icons::file_icon(&e.path, 14., &p)),
                            )
                        }
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .ml(px(6.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(label),
                    )
                    .when_some(mark.filter(|_| !e.dir), |d, m| {
                        d.child(
                            div()
                                .flex_none()
                                .w(px(14.))
                                .text_size(px(11.))
                                .font_family(look::MONO)
                                .text_color(m.color(&p))
                                .child(m.letter()),
                        )
                    })
                    .when(mark.is_some() && e.dir, |d| {
                        d.child(div().flex_none().mr(px(4.)).size(px(5.)).rounded_full().bg(ink.opacity(0.7)))
                    })
                    .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    .on_click(cx.listener({
                        let path = path.clone();
                        let dir = e.dir;
                        move |this, e: &ClickEvent, _, cx| {
                            if dir {
                                this.toggle(path.clone(), cx);
                            } else {
                                // One click looks at the file; two keep its tab open.
                                let a = if e.click_count() >= 2 {
                                    Action::OpenFile(path.clone(), None)
                                } else {
                                    Action::PreviewFile(path.clone())
                                };
                                this.model.update(cx, |m, cx| m.handle(a, cx));
                            }
                        }
                    }))
                    .context_menu(move |menu, _, _| {
                        let send = |a: Action| {
                            let model = model.clone();
                            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                let a = a.clone();
                                model.update(cx, |m, cx| m.handle(a, cx))
                            }
                        };
                        let menu = if path.is_file() {
                            menu.item(PopupMenuItem::new("Open").on_click(send(Action::OpenFile(path.clone(), None))))
                                .separator()
                        } else {
                            menu
                        };
                        menu.item(PopupMenuItem::new("Reveal in Finder").on_click(send(Action::Reveal(path.clone()))))
                            .item(
                                PopupMenuItem::new("Copy path")
                                    .on_click(send(Action::Copy(path.display().to_string()))),
                            )
                    })
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .child(title(name.clone().unwrap_or_else(|| "Files".into()))),
            )
            .child(separator(&p))
            .when(name.is_none(), |d| d.child(look::weak("Open a repository to see its files", &p)))
            .child(div().id("file-tree").flex_1().min_h_0().flex().flex_col().overflow_y_scrollbar().children(items))
    }
}

//! The file tree of the open repository, in the Projects panel: folders open and close,
//! a click opens a file in an editor tab. Files `.gitignore` leaves out stay out. The
//! repository is watched (`notify`), so files made or deleted elsewhere show up at once.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;
use notify::Watcher as _;
use snout_core::art::Glyph;

use crate::art::glyph;
use crate::look::{self, separator, title};
use crate::model::{Action, Model};
use crate::theme::palette;

const ROW: f32 = 22.0;
const INDENT: f32 = 14.0;

/// A file or folder in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub dir: bool,
}

/// The folders and files directly in `dir`, folders first, each sorted by name. What
/// `.gitignore` (and the repository's own `.git`) leaves out is not listed.
pub fn list_dir(dir: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.depth() == 1)
        .map(|e| Entry { dir: e.file_type().is_some_and(|t| t.is_dir()), path: e.into_path() })
        .collect();
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.path.file_name().cmp(&b.path.file_name())));
    out
}

pub struct FileTree {
    model: Entity<Model>,
    root: Option<PathBuf>,
    /// The folders open.
    pub expanded: HashSet<PathBuf>,
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
        let mut this =
            Self { model, root: None, expanded: HashSet::new(), listing: HashMap::new(), watcher: None, events: None };
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
                continue;
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

    /// The rows showing: every entry under an open folder, with its depth.
    pub fn rows(&mut self) -> Vec<(Entry, usize)> {
        let Some(root) = self.root.clone() else { return Vec::new() };
        let mut out = Vec::new();
        self.walk(&root, 0, &mut out);
        out
    }

    fn walk(&mut self, dir: &Path, depth: usize, out: &mut Vec<(Entry, usize)>) {
        for e in self.children(dir) {
            let open = e.dir && self.expanded.contains(&e.path);
            let path = e.path.clone();
            out.push((e, depth));
            if open {
                self.walk(&path, depth + 1, out);
            }
        }
    }
}

impl Render for FileTree {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let name = self.root.as_ref().map(|r| Model::dir_name(r));
        let rows = self.rows();
        let hover_bg = p.button_hover;
        let model = self.model.clone();
        let items: Vec<AnyElement> = rows
            .into_iter()
            .enumerate()
            .map(|(i, (e, depth))| {
                let label = e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let open = self.expanded.contains(&e.path);
                let path = e.path.clone();
                let model = model.clone();
                let tip: SharedString = e.path.display().to_string().into();
                div()
                    .id(("file-row", i))
                    .debug_selector(move || format!("file-{label}"))
                    .flex_none()
                    .h(px(ROW))
                    .pl(px(2. + depth as f32 * INDENT))
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .child(
                        div().w(px(14.)).h_full().flex_none().when(e.dir, |d| d.child(look::chevron(open, p.fg_dim))),
                    )
                    .when(e.dir, |d| d.child(glyph(Glyph::Folder, 16., p.fg)).child(div().w(px(6.))))
                    .when(!e.dir, |d| d.child(div().w(px(4.))))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                    )
                    .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    .on_click(cx.listener({
                        let path = path.clone();
                        let dir = e.dir;
                        move |this, _, _, cx| {
                            if dir {
                                this.toggle(path.clone(), cx);
                            } else {
                                let a = Action::OpenFile(path.clone(), None);
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
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .child(title(name.clone().unwrap_or_else(|| "Files".into()))),
            )
            .child(separator(&p))
            .when(name.is_none(), |d| d.child(look::weak("Open a repository to see its files", &p)))
            .child(div().id("file-tree").flex_1().min_h_0().flex().flex_col().overflow_y_scrollbar().children(items))
    }
}

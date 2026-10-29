//! The Projects panel: the repository list, in groups, with the open repository
//! expandable into its directory tree. Picking a directory narrows the Git Panel's changed
//! files to it; picking the repository shows them all again. Repositories drag onto groups.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;
use snout_core::Tracking;
use snout_core::groups::{arrange, dir_name, short};

use crate::model::{Action, Model};
use crate::theme::{Palette, palette};
use snout_core::art::Glyph;

use crate::art::glyph;
use crate::look::{self, Trigger, chip, separator, title};

/// What a repository row carries while it is dragged.
#[derive(Clone)]
pub struct Dragged(pub PathBuf);

struct DragPreview(String);

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .h(px(26.))
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .text_color(p.fg)
            .child(glyph(Glyph::Repo, 16., p.fg))
            .child(self.0.clone())
    }
}

const ROW: f32 = 24.0;
const INDENT: f32 = 14.0;

pub struct ProjectsView {
    model: Entity<Model>,
    filter: Entity<InputState>,
    /// Groups and directories folded or unfolded against their default.
    toggled: HashSet<String>,
    /// Which listed repositories are still there, as the host says.
    present: crate::host::Facts<bool>,
}

impl ProjectsView {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.subscribe(&filter, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().to_string();
                this.model.update(cx, |m, cx| {
                    m.repo_filter = text;
                    cx.notify();
                });
            }
        })
        .detach();
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self { model, filter, toggled: HashSet::new(), present: Default::default() }
    }

    fn is_open(&self, key: &str, default: bool) -> bool {
        default != self.toggled.contains(key)
    }

    fn toggle(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.toggled.remove(&key) {
            self.toggled.insert(key);
        }
        cx.notify();
    }
}

fn send(model: &Entity<Model>, action: Action) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let model = model.clone();
    move |_, _, cx| {
        let action = action.clone();
        model.update(cx, |m, cx| m.handle(action, cx));
    }
}

/// The items to open or find repositories, in the + menu and on the welcome page.
pub fn open_items(menu: PopupMenu, model: &Entity<Model>) -> PopupMenu {
    menu.item(PopupMenuItem::new("Open local repository…").on_click(send(model, Action::PickFolder)))
        .item(PopupMenuItem::new("Find repositories in folder…").on_click(send(model, Action::PickScanFolder)))
        .item(PopupMenuItem::new("Clone from GitHub…").on_click(send(model, Action::ShowClone)))
}

impl Render for ProjectsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let model = self.model.clone();
        let m = self.model.read(cx);
        let (groups, loose) = arrange(&m.settings.repos, &m.settings.groups);
        let names: Vec<String> = groups.iter().map(|g| g.name.clone()).collect();
        let needle = m.repo_filter.trim().to_lowercase();
        let many = m.settings.repos.len() > 6;
        let empty = m.settings.repos.is_empty();

        let mut rows: Vec<AnyElement> = Vec::new();
        if !needle.is_empty() {
            // Matches in one flat list, each still filed under its group.
            let hits = groups
                .iter()
                .flat_map(|g| g.repos.iter().map(move |p| (p.clone(), Some(g.name.clone()))))
                .chain(loose.iter().map(|p| (p.clone(), None)))
                .filter(|(p, g)| {
                    dir_name(p).to_lowercase().contains(&needle)
                        || g.as_ref().is_some_and(|g| g.to_lowercase().contains(&needle))
                })
                .collect::<Vec<_>>();
            if hits.is_empty() {
                rows.push(div().text_color(p.fg_dim).child("No matches").into_any_element());
            }
            for (path, group) in hits {
                self.repo_entry(&mut rows, &path, group.as_deref(), 0, &names, window, cx);
            }
        } else {
            for g in &groups {
                let key = format!("group:{}", g.name);
                let open = self.is_open(&key, true);
                let mut chips = vec![(g.repos.len().to_string(), p.fg_dim)];
                // Closed, the group still says when something in it has commits to pull.
                let m = self.model.read(cx);
                let behind: u32 =
                    g.repos.iter().filter_map(|r| m.tracking.get(r).copied().flatten()).map(|t| t.behind).sum();
                if let Some((text, _)) = snout_core::survey::describe(Tracking { ahead: 0, behind }).filter(|_| !open) {
                    chips.insert(0, (text, p.accent));
                }
                let name = g.name.clone();
                let row = list_row(
                    SharedString::from(key.clone()),
                    Glyph::Folder,
                    &g.name,
                    0,
                    Some(open),
                    false,
                    false,
                    &chips,
                    &p,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(key.clone(), cx)))
                .drag_over::<Dragged>(move |s, _, _, _| s.bg(p.wash(p.ok)).border_color(p.ok))
                .on_drop({
                    let model = model.clone();
                    let name = name.clone();
                    move |d: &Dragged, _, cx| {
                        let a = Action::MoveToGroup(d.0.clone(), Some(name.clone()));
                        model.update(cx, |m, cx| m.handle(a, cx));
                    }
                })
                .context_menu({
                    let model = model.clone();
                    move |menu, _, _| {
                        menu.item(
                            PopupMenuItem::new("Rename…").on_click(send(&model, Action::ShowRenameGroup(name.clone()))),
                        )
                        .item(
                            PopupMenuItem::new("Delete group")
                                .on_click(send(&model, Action::DeleteGroup(name.clone()))),
                        )
                        .separator()
                        .item(PopupMenuItem::new("New group…").on_click(send(&model, Action::ShowNewGroup(None))))
                    }
                });
                rows.push(row.into_any_element());
                if open {
                    if g.repos.is_empty() {
                        rows.push(
                            div()
                                .pl(px(INDENT + 30.))
                                .text_color(p.fg_dim)
                                .child("Drag repositories here")
                                .into_any_element(),
                        );
                    }
                    for path in &g.repos {
                        self.repo_entry(&mut rows, path, Some(&g.name), 1, &names, window, cx);
                    }
                }
            }
            if !groups.is_empty() && !loose.is_empty() {
                rows.push(div().h(px(6.)).into_any_element());
            }
            for path in &loose {
                self.repo_entry(&mut rows, path, None, 0, &names, window, cx);
            }
        }

        let add = Trigger(look::icon_button("add-repo", Glyph::Plus, "Add a repository or a group", true, &p))
            .dropdown_menu({
                let model = model.clone();
                move |menu, _, _| {
                    open_items(menu, &model)
                        .separator()
                        .item(PopupMenuItem::new("New group…").on_click(send(&model, Action::ShowNewGroup(None))))
                }
            });
        div()
            .id("projects")
            .debug_selector(|| "repo-list".into())
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(title("Repositories"))
                    .child(add),
            )
            .child(separator(&p))
            .when(many, |d| d.child(look::field(&self.filter, false, None, &p)).child(div().h(px(4.))))
            .child(
                div()
                    .id("projects-list")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .overflow_y_scrollbar()
                    .when(empty, |d| d.child(div().text_color(p.fg_dim).child("No repositories yet. Use + or ⌘O.")))
                    .children(rows)
                    // The space below the list takes a repository out of its group.
                    .child(
                        div()
                            .id("ungroup")
                            .flex_1()
                            .min_h(px(ROW + 8.))
                            .rounded(px(6.))
                            .drag_over::<Dragged>(move |s, _, _, _| s.bg(p.wash(p.ok)).border_1().border_color(p.ok))
                            .on_drop({
                                let model = model.clone();
                                move |d: &Dragged, _, cx| {
                                    let a = Action::MoveToGroup(d.0.clone(), None);
                                    model.update(cx, |m, cx| m.handle(a, cx));
                                }
                            }),
                    ),
            )
    }
}

impl ProjectsView {
    #[allow(clippy::too_many_arguments)]
    fn repo_entry(
        &mut self,
        rows: &mut Vec<AnyElement>,
        path: &Path,
        group: Option<&str>,
        depth: u8,
        names: &[String],
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Unknown yet counts as there.
        let missing = !self.present.get(path, cx, |v| &mut v.present, |h, p| h.is_dir(p)).unwrap_or(true);
        let p = palette(cx);
        let m = self.model.read(cx);
        let model = self.model.clone();
        let name = dir_name(path);
        let current = m.repo.as_ref().is_some_and(|r| r.root() == path);
        let mut chips = Vec::new();
        if missing {
            chips.push(("missing".to_owned(), p.error));
        }
        let mut tree = None;
        if current {
            let b = &m.status.branch;
            let mut head = short(&m.head_label(), 20);
            if b.ahead > 0 {
                head += &format!(" ↑{}", b.ahead);
            }
            if b.behind > 0 {
                head += &format!(" ↓{}", b.behind);
            }
            chips.push((head, if b.head.is_some() { p.branch() } else { p.warn }));
            if !m.status.files.is_empty() {
                chips.push((m.status.files.len().to_string(), p.accent));
            }
            tree = Some(dir_tree(m));
        } else if let Some((text, _)) = m.tracking.get(path).copied().flatten().and_then(snout_core::survey::describe) {
            chips.push((text, p.accent));
        }
        let key = format!("repo:{}", path.display());
        let has_dirs = tree.as_ref().is_some_and(|(children, _)| children.contains_key(""));
        let open = has_dirs.then(|| self.is_open(&key, true));
        let selected = current && m.dir_filter.is_none();
        let g = if missing { Glyph::Alert } else { Glyph::Repo };
        let id = SharedString::from(key.clone());
        let path_buf = path.to_owned();
        let row = list_row(id, g, &name, depth, open, selected, missing, &chips, &p)
            .on_click(cx.listener({
                let path = path_buf.clone();
                move |this, e: &ClickEvent, _, cx| {
                    if current && open.is_some() && e.position().x < px(8. + f32::from(depth) * INDENT + 18.) {
                        this.toggle(key.clone(), cx);
                    } else if current {
                        this.model.update(cx, |m, cx| m.handle(Action::FilterDir(None), cx));
                    } else {
                        let a = Action::OpenRepo(path.clone());
                        this.model.update(cx, |m, cx| m.handle(a, cx));
                    }
                }
            }))
            .on_drag(Dragged(path_buf.clone()), {
                let name = name.clone();
                move |_, _, _, cx| cx.new(|_| DragPreview(name.clone()))
            })
            .drag_over::<Dragged>(move |s, _, _, _| s.bg(p.wash(p.ok)).border_color(p.ok))
            .on_drop({
                let model = model.clone();
                let group = group.map(str::to_owned);
                move |d: &Dragged, _, cx| {
                    let a = Action::MoveToGroup(d.0.clone(), group.clone());
                    model.update(cx, |m, cx| m.handle(a, cx));
                }
            })
            .context_menu({
                let (model, path, group, names) =
                    (model.clone(), path_buf.clone(), group.map(str::to_owned), names.to_vec());
                move |menu, window, cx| repo_menu(menu, &model, &path, group.as_deref(), &names, !current, window, cx)
            });
        rows.push(row.into_any_element());
        if let (Some((children, changed)), Some(true)) = (tree, open) {
            self.dir_rows(rows, path, &children, &changed, "", depth + 1, cx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dir_rows(
        &mut self,
        rows: &mut Vec<AnyElement>,
        root: &Path,
        children: &BTreeMap<String, Vec<String>>,
        changed: &HashMap<String, usize>,
        parent: &str,
        depth: u8,
        cx: &mut Context<Self>,
    ) {
        let Some(dirs) = children.get(parent) else { return };
        let p = palette(cx);
        let filter = self.model.read(cx).dir_filter.clone();
        for dir in dirs {
            let name = dir.rsplit('/').next().unwrap_or(dir);
            let n = changed.get(dir).copied().unwrap_or(0);
            let key = format!("dir:{}:{dir}", root.display());
            let open = children.contains_key(dir).then(|| self.is_open(&key, false));
            let chips = if n > 0 { vec![(n.to_string(), p.accent)] } else { Vec::new() };
            let selected = filter.as_deref() == Some(dir.as_str());
            let full = root.join(dir);
            let row = list_row(
                SharedString::from(key.clone()),
                Glyph::Folder,
                name,
                depth,
                open,
                selected,
                false,
                &chips,
                &p,
            )
            .on_click(cx.listener({
                let dir = dir.clone();
                move |this, e: &ClickEvent, _, cx| {
                    if open.is_some() && e.position().x < px(8. + f32::from(depth) * INDENT + 18.) {
                        this.toggle(key.clone(), cx);
                    } else {
                        let a = Action::FilterDir(Some(dir.clone()));
                        this.model.update(cx, |m, cx| m.handle(a, cx));
                    }
                }
            }))
            .context_menu({
                let model = self.model.clone();
                move |menu, _, _| reveal_items(menu, &model, &full)
            });
            rows.push(row.into_any_element());
            if open == Some(true) {
                self.dir_rows(rows, root, children, changed, dir, depth + 1, cx);
            }
        }
    }
}

/// Directories by parent, and changed files under each directory (recursively).
#[allow(clippy::type_complexity)]
fn dir_tree(m: &Model) -> (BTreeMap<String, Vec<String>>, HashMap<String, usize>) {
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for dir in &m.dirs {
        let parent = dir.rfind('/').map_or("", |i| &dir[..i]);
        children.entry(parent.to_owned()).or_default().push(dir.clone());
    }
    let mut changed: HashMap<String, usize> = HashMap::new();
    for f in &m.status.files {
        let mut end = f.path.len();
        while let Some(slash) = f.path[..end].rfind('/') {
            *changed.entry(f.path[..slash].to_owned()).or_default() += 1;
            end = slash;
        }
    }
    (children, changed)
}

pub fn reveal_items(menu: PopupMenu, model: &Entity<Model>, path: &Path) -> PopupMenu {
    menu.item(PopupMenuItem::new("Reveal in Finder").on_click(send(model, Action::Reveal(path.to_owned()))))
        .item(PopupMenuItem::new("Copy path").on_click(send(model, Action::Copy(path.display().to_string()))))
}

#[allow(clippy::too_many_arguments)]
fn repo_menu(
    menu: PopupMenu,
    model: &Entity<Model>,
    path: &Path,
    group: Option<&str>,
    names: &[String],
    openable: bool,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let menu = if openable {
        menu.item(PopupMenuItem::new("Open").on_click(send(model, Action::OpenRepo(path.to_owned()))))
    } else {
        menu
    };
    let (m2, path2, group2, names2) = (model.clone(), path.to_owned(), group.map(str::to_owned), names.to_vec());
    reveal_items(menu, model, path)
        .separator()
        .submenu("Move to group", window, cx, move |mut menu, _, _| {
            for name in names2.iter().filter(|n| Some(n.as_str()) != group2.as_deref()) {
                menu = menu.item(
                    PopupMenuItem::new(name.clone())
                        .on_click(send(&m2, Action::MoveToGroup(path2.clone(), Some(name.clone())))),
                );
            }
            if group2.is_some() {
                menu = menu
                    .item(PopupMenuItem::new("No group").on_click(send(&m2, Action::MoveToGroup(path2.clone(), None))));
            }
            if names2.iter().any(|n| Some(n.as_str()) != group2.as_deref()) || group2.is_some() {
                menu = menu.separator();
            }
            menu.item(PopupMenuItem::new("New group…").on_click(send(&m2, Action::ShowNewGroup(Some(path2.clone())))))
        })
        .separator()
        .item(PopupMenuItem::new("Remove from list").on_click(send(model, Action::RemoveRepo(path.to_owned()))))
}

/// A row of the tree: arrow column, icon, name (cut to fit), and chips at the right end.
#[allow(clippy::too_many_arguments)]
fn list_row(
    id: SharedString,
    g: Glyph,
    name: &str,
    depth: u8,
    open: Option<bool>,
    selected: bool,
    dim: bool,
    chips: &[(String, gpui::Hsla)],
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let ink = if dim { p.fg_dim } else { p.fg };
    let hover = p.button_hover;
    div()
        .id(id)
        .flex_none()
        .h(px(ROW))
        .pl(px(2. + f32::from(depth) * INDENT))
        .pr(px(3.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_color(ink)
        .when(selected, |d| d.bg(p.selection()))
        .when(!selected, |d| d.hover(move |s| s.bg(hover)))
        .child(div().w(px(14.)).h_full().flex_none().when_some(open, |d, open| d.child(look::chevron(open, p.fg_dim))))
        .child(div().w(px(1.)))
        .child(glyph(g, 16., ink))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .ml(px(7.))
                .mr(px(4.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(name.to_owned()),
        )
        .child(div().flex().gap(px(4.)).children(chips.iter().map(|(t, c)| chip(t.clone(), *c, p))))
}

//! The Welcome page: the center's tab while no file is open. With no project: the wordmark
//! and version with a small drawing; Open Project, Clone Repository and Remote Project, with
//! New Project in small above them; and the recent projects, each with its path, branch and
//! when it was last opened, narrowed by a search box. With remote projects there too, they
//! come under headings: this machine's first, then (in a remote window) the host's, then
//! those of other machines with their host. With a project open and no tab: the empty
//! editor, its drawing, the ways to a file with their keys, and the files opened lately.
//! The first file opened takes its place.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;

use crate::glyphs::{Icon, icon};
use crate::model::{Action, Model};
use crate::theme::{mix, palette};

/// What a tile does: an action of the model, or one the window dispatches (Connect to SSH
/// Host, which the workspace handles).
enum Tile {
    Model(Action),
    Window(Box<dyn gpui::Action>),
}

pub struct WelcomePanel {
    model: Entity<Model>,
    pub search: Entity<InputState>,
    focus: FocusHandle,
    /// The branch of each recent project, read from the host; asked again when the open
    /// project's head moves.
    branches: crate::host::Facts<Option<String>>,
    head: Option<String>,
    /// How wide the page was last drawn: a narrow one stacks its actions.
    width: f32,
    /// This machine, for the heading over its projects and, in a remote window, its list.
    here: Here,
}

/// This machine's recent projects, as a remote window shows them (its own list is the host's),
/// and its heading: `This computer (my-laptop)`.
#[derive(Default)]
struct Here {
    label: String,
    recent: Vec<PathBuf>,
    opened: std::collections::BTreeMap<PathBuf, u64>,
    home: PathBuf,
}

/// `This computer (my-laptop)`: the machine's kind, and its name if it has one.
fn here_label() -> String {
    let mut kind = crate::platform::this_machine().chars();
    let kind = kind.next().map(|c| c.to_uppercase().chain(kind).collect::<String>()).unwrap_or_default();
    match crate::platform::machine_name() {
        Some(name) => t!("welcome-recent-here", machine = kind, name = name),
        None => kind,
    }
}

/// Below this the page stacks its actions and leaves out the drawing and the branches.
const NARROW_W: f32 = 620.;

/// Whether a page `width` wide is laid out for a narrow pane.
fn is_narrow(width: f32) -> bool {
    width < NARROW_W
}

impl WelcomePanel {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&model, |this: &mut Self, model, cx| {
            let head = model.read(cx).branch_summary();
            if head != this.head {
                this.head = head;
                this.branches.clear();
            }
            cx.notify()
        })
        .detach();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(ts!("welcome-search-recent-projects")));
        cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        let here = Here { label: here_label(), ..Default::default() };
        let mut this = Self {
            model,
            search,
            focus: cx.focus_handle(),
            branches: Default::default(),
            head: None,
            width: 1000.,
            here,
        };
        this.read_here(cx);
        this
    }

    /// In a remote window, read this machine's recent projects from its settings, off the UI
    /// thread. The window is made again when the connection changes, so once is enough.
    pub(crate) fn read_here(&mut self, cx: &mut Context<Self>) {
        let Some(r) = cx.try_global::<crate::remote::Remote>().filter(|r| r.connection.is_some()) else { return };
        let (Some(store), Some(local)) = (r.main_store.clone(), r.local.clone()) else { return };
        let settings = cx.background_executor().spawn(async move { store.load() });
        cx.spawn(async move |this, cx| {
            let settings = settings.await;
            let _ = this.update(cx, |this, cx| {
                this.here.recent = crate::model::recent_projects(&settings);
                this.here.opened = settings.opened;
                this.here.home = local.info().home.clone();
                cx.notify();
            });
        })
        .detach();
    }

    /// The recent projects the search box lets through.
    pub fn shown(&self, cx: &gpui::App) -> Vec<PathBuf> {
        self.shown_by(&snout_find::Pattern::new(&self.search.read(cx).value()), cx)
    }

    /// The recent projects `pattern` lets through.
    fn shown_by(&self, pattern: &snout_find::Pattern, cx: &gpui::App) -> Vec<PathBuf> {
        self.model
            .read(cx)
            .recent_projects()
            .into_iter()
            .filter(|p| pattern.filter_path(&snout_host::slash_path(p)))
            .collect()
    }
}

crate::panels::panel!(WelcomePanel, "Welcome", |_, _| ts!("welcome-welcome").into(), true);

/// The branch checked out in the repository holding `dir` on `host`, read from its `HEAD`.
pub fn branch_of(host: &dyn snout_host::Host, dir: &Path) -> Option<String> {
    let (repo, dot_git) =
        dir.ancestors().find_map(|d| host.metadata(&d.join(".git")).ok().map(|m| (d, m.is_file())))?;
    let git_dir = if dot_git {
        let text = host.read_to_string(&repo.join(".git")).ok()?;
        let rel = text.trim().strip_prefix("gitdir:")?.trim().to_owned();
        repo.join(rel)
    } else {
        repo.join(".git")
    };
    let head = host.read_to_string(&git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: refs/heads/") {
        Some(branch) => Some(branch.to_owned()),
        None => Some(head.chars().take(8).collect()),
    }
}

/// "just now", "5 minutes ago", "yesterday", "3 weeks ago".
pub fn ago(secs: u64, now: u64) -> String {
    let d = now.saturating_sub(secs);
    let per = |unit: u64| d / unit;
    match d {
        0..60 => ts!("welcome-just-now").into(),
        60..3600 => t!("welcome-minutes-ago", count = per(60)),
        3600..86_400 => t!("welcome-hours-ago", count = per(3600)),
        86_400..172_800 => ts!("welcome-yesterday").into(),
        172_800..1_209_600 => t!("welcome-days-ago", count = per(86_400)),
        1_209_600..5_184_000 => t!("welcome-weeks-ago", count = per(604_800)),
        _ => t!("welcome-months-ago", count = per(2_592_000)),
    }
}

/// `path` with the home folder (`home`) as `~`.
pub(crate) fn tilde(path: &Path, home: &Path) -> String {
    if path.starts_with(home) && !home.as_os_str().is_empty() {
        format!("~/{}", path.strip_prefix(home).unwrap_or(path).display())
    } else {
        path.display().to_string()
    }
}

impl WelcomePanel {
    /// The editor with no tab open: the drawing, the ways to a file (each with its keys),
    /// and the files opened lately.
    fn empty_editor(&self, root: &Path, window: &Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::components::IconName;
        let p = palette(cx);
        let hover = p.row_hover();
        let keys =
            |a: &dyn gpui::Action| window.bindings_for_action(a).last().map(|b| crate::keymap::binding_label(b, cx));
        let ways: [(&'static str, IconName, &'static str, Box<dyn gpui::Action>); 5] = [
            ("empty-go-to-file", IconName::FileSearch, ts!("welcome-go-to-file"), Box::new(crate::menus::GoToFile)),
            (
                "empty-recent-files",
                IconName::FileClock,
                ts!("welcome-recent-files"),
                Box::new(crate::menus::RecentFiles),
            ),
            (
                "empty-search-everywhere",
                IconName::Search,
                ts!("welcome-search-everywhere"),
                Box::new(crate::menus::SearchEverywhere),
            ),
            (
                "empty-find-in-files",
                IconName::TextSearch,
                ts!("welcome-find-in-files"),
                Box::new(crate::menus::FindInFiles),
            ),
            ("empty-new-file", IconName::FilePlus, ts!("welcome-new-file"), Box::new(crate::menus::NewFile)),
        ];
        let rows = ways.into_iter().map(|(id, ico, label, action)| {
            let k = keys(action.as_ref()).unwrap_or_default();
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .w(px(300.))
                .h(px(30.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(crate::components::icon(ico, 14., p.accent))
                .child(div().flex_1().text_color(p.fg).child(label))
                .when(!k.is_empty(), |d| {
                    d.child(
                        div()
                            .px(px(6.))
                            .h(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .bg(p.card)
                            .border_1()
                            .border_color(mix(p.border, 0.8, p.card))
                            .text_size(px(11.5))
                            .text_color(p.fg_dim)
                            .child(k),
                    )
                })
                .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
        });
        let recent: Vec<PathBuf> = self.model.read(cx).recent_project_files(cx).into_iter().take(5).collect();
        let files = recent.into_iter().enumerate().map(|(i, path)| {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let dir = path.parent().and_then(|d| crate::search::relative_to(d, root)).unwrap_or_default();
            let model = self.model.clone();
            div()
                .id(("empty-recent", i))
                .debug_selector(move || format!("empty-recent-{i}"))
                .w(px(300.))
                .h(px(28.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(crate::icons::file_icon(&path, 14., &p))
                .child(div().flex_none().text_color(p.fg).child(name))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(11.5))
                        .text_color(p.fg_dim)
                        .child(dir),
                )
                .on_click(move |_, _, cx| {
                    let path = path.clone();
                    model.update(cx, |m, cx| m.handle(Action::OpenFile(path, None), cx));
                })
        });
        let files: Vec<_> = files.collect();
        div()
            .id("empty-editor")
            .debug_selector(|| "empty-editor".into())
            .size_full()
            .overflow_y_scrollbar()
            .bg(p.panel)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py(px(24.))
            .gap(px(4.))
            .child(crate::art::drawing_on(snout_core::art::spots::editor(), 176., 176. / 1.5, p.panel))
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(crate::look::text(ts!("welcome-no-files-open"))),
            )
            .child(
                div()
                    .mb(px(10.))
                    .text_size(px(12.5))
                    .text_color(p.fg_dim)
                    .child(crate::look::text(ts!("welcome-open-one-from-the-project-tree-or"))),
            )
            .children(rows)
            .when(!files.is_empty(), |d| {
                d.child(
                    div()
                        .w(px(300.))
                        .mt(px(14.))
                        .mb(px(4.))
                        .px(px(10.))
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.fg_dim)
                        .child(crate::look::text(ts!("welcome-recent-files-2"))),
                )
                .children(files)
            })
            .into_any_element()
    }
}

impl Render for WelcomePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        // A project open with no tab: the empty editor.
        if let Some(root) = self.model.read(cx).project_root() {
            return self.empty_editor(&root, window, cx);
        }
        let model = self.model.clone();
        let narrow = is_narrow(self.width);
        let view = cx.entity();
        let pattern = snout_find::Pattern::new(&self.search.read(cx).value());
        let recent = self.shown_by(&pattern, cx);
        let branches: std::collections::HashMap<PathBuf, Option<String>> = recent
            .iter()
            .filter_map(|p| {
                let b = self.branches.get(p, cx, |v| &mut v.branches, |h, p| branch_of(h, p))?;
                Some((p.clone(), b))
            })
            .collect();
        // The remote projects opened lately, by their folders or their host, but for those of
        // the host the window is on (its own list has them).
        let connected = crate::remote::connection(cx).map(|(name, _)| name);
        let mut all_remote = crate::remote::recents(cx).projects;
        all_remote.retain(|r| Some(&r.host) != connected.as_ref());
        let remote: Vec<_> = all_remote
            .iter()
            .filter(|r| pattern.filter_path(&snout_host::slash_path(&r.path)) || pattern.filter(&r.host))
            .collect();
        let here: Vec<_> =
            self.here.recent.iter().filter(|p| pattern.filter_path(&snout_host::slash_path(p))).cloned().collect();
        let any_recent =
            !self.model.read(cx).recent_projects().is_empty() || !all_remote.is_empty() || !self.here.recent.is_empty();
        let opened = self.model.read(cx).settings.opened.clone();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let hover = p.button_hover;
        let row_hover = p.row_hover();
        let host = crate::host::host(cx);

        let action = |id: &'static str, g: Icon, title: &'static str, note: String, a: Option<Tile>| {
            let model = model.clone();
            let enabled = a.is_some();
            // Each action in its own colour when the palette is in colour.
            let fill = match (p.tinted, id) {
                (true, "welcome-clone") => p.tile(p.hues[1]),
                (true, "welcome-remote") => p.tile(p.hues[0]),
                _ => (p.accent, p.on_accent),
            };
            let (fill, ink) = fill;
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .when(narrow, |d| d.flex_none().w_full())
                .when(!narrow, |d| d.flex_1())
                .min_w_0()
                .h(px(if narrow { 58. } else { 74. }))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(12.))
                .rounded(px(8.))
                .bg(p.card)
                .border_1()
                .border_color(mix(p.border, 0.7, p.card))
                .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
                .when(!enabled, |d| d.opacity(0.55))
                .child(
                    div()
                        .flex_none()
                        .size(px(34.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .bg(if enabled { fill } else { p.button })
                        .when(enabled, |d| d.shadow(p.glow_of(fill)))
                        .child(icon(g, 17., if enabled { ink } else { p.fg_dim })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(title))
                        .child(div().min_w_0().truncate().text_size(px(12.)).text_color(p.fg_dim).child(note)),
                )
                .when_some(a, |d, a| {
                    d.on_click(move |_, window, cx| match &a {
                        Tile::Model(a) => {
                            let a = a.clone();
                            model.update(cx, |m, cx| m.handle(a, cx));
                        }
                        Tile::Window(a) => window.dispatch_action(a.boxed_clone(), cx),
                    })
                })
        };

        let header = div()
            .flex()
            .items_center()
            .gap(px(20.))
            .when(!narrow, |d| d.child(crate::art::drawing(snout_core::art::welcome(), 165., 110., None)))
            .child(div().flex().flex_col().gap(px(2.)).child(crate::git_view::wordmark(&p)).child(
                div().pl(px(62.)).text_size(px(12.5)).text_color(p.fg_dim).child(crate::look::text(t!(
                    "dialogs-version",
                    cargo_pkg_version = (env!("CARGO_PKG_VERSION")).to_string()
                ))),
            ));

        let actions = div()
            .flex()
            .when(narrow, |d| d.flex_col())
            .gap(px(10.))
            .child(action(
                "welcome-open",
                Icon::Folder,
                ts!("cmd-open-project"),
                match crate::remote::connection(cx) {
                    Some((host, _)) => t!("welcome-a-folder-on", host = host.to_string()),
                    None => t!("welcome-folder-on-machine", machine = crate::platform::this_machine()),
                },
                Some(Tile::Model(Action::PickFolder)),
            ))
            .child(action(
                "welcome-clone",
                Icon::Fetch,
                ts!("welcome-clone-repository"),
                ts!("welcome-from-a-git-url").into(),
                Some(Tile::Model(Action::ShowClone)),
            ))
            .child(action(
                "welcome-remote",
                Icon::Terminal,
                ts!("welcome-remote-project"),
                if cfg!(windows) {
                    ts!("welcome-over-ssh-or-in-wsl")
                } else {
                    ts!("welcome-on-another-machine-over-ssh")
                }
                .into(),
                Some(Tile::Window(Box::new(crate::menus::ConnectSsh))),
            ));

        // + New Project, in small above the tiles' right end.
        let new_project = {
            let model = model.clone();
            let accent = p.accent;
            div().flex().justify_end().child(
                div()
                    .id("welcome-new")
                    .debug_selector(|| "welcome-new".into())
                    .px(px(6.))
                    .py(px(2.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .text_color(p.fg_dim)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover).text_color(accent))
                    .child(icon(Icon::Plus, 11., p.fg_dim))
                    .child(ts!("welcome-new"))
                    .on_click(move |_, _, cx| model.update(cx, |m, cx| m.handle(Action::ShowNewProject, cx))),
            )
        };

        // A project's initials on its own colour.
        let badge = |name: &str| {
            div()
                .flex_none()
                .size(px(28.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .bg(crate::workspace::badge_color(name, &p))
                .text_color(p.on_badge)
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(crate::workspace::initials(name))
        };
        // The small heading over a group of recent projects.
        let section = |id: &'static str, g: Icon, label: String| {
            div()
                .debug_selector(move || id.to_owned())
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .pb(px(2.))
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.fg_dim)
                .child(icon(g, 12., p.fg_dim))
                .child(crate::look::text(label))
        };

        // One recent project: its initials, name and path, what is on its right (its branch or
        // host), and when it was opened.
        let row = |key: &'static str,
                   i: usize,
                   name: String,
                   path: String,
                   right: Option<gpui::AnyElement>,
                   when: Option<String>| {
            div()
                .id((key, i))
                .debug_selector(move || format!("{key}-{i}"))
                .h(px(46.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(12.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |s| s.bg(row_hover))
                .child(badge(&name))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(name))
                        .child(div().h(px(16.)).flex().text_size(px(12.)).child(crate::components::path_text(
                            ("path", i),
                            path,
                            p.fg_dim,
                        ))),
                )
                .children(right)
                .child(
                    div()
                        .flex_none()
                        .w(px(if narrow { 84. } else { 110. }))
                        .whitespace_nowrap()
                        .text_right()
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child(when.unwrap_or_default()),
                )
        };
        // A branch or a host, with its icon.
        let tag = |g: Icon, text: String| {
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(4.))
                .max_w(px(180.))
                .text_size(px(12.))
                .text_color(p.fg_dim)
                .child(icon(g, 12., p.fg_dim))
                .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(text))
                .into_any_element()
        };

        // The projects of the window's machine: this one, or the host it is connected to.
        let rows: Vec<_> = recent
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let branch = branches.get(path).cloned().flatten();
                let model = model.clone();
                let target = path.clone();
                row(
                    "welcome-recent",
                    i,
                    Model::dir_name(path),
                    tilde(path, &host.info().home),
                    branch.filter(|_| !narrow).map(|b| tag(Icon::Branch, b)),
                    opened.get(path).map(|t| ago(*t, now)),
                )
                .on_click(move |_, _, cx| {
                    let a = Action::OpenRepo(target.clone());
                    model.update(cx, |m, cx| m.handle(a, cx));
                })
                .into_any_element()
            })
            .collect();

        // In a remote window, this machine's projects: one closes the connection and opens here.
        let here_rows: Vec<_> = here
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let target = path.clone();
                row(
                    "welcome-local-recent",
                    i,
                    Model::dir_name(path),
                    tilde(path, &self.here.home),
                    None,
                    self.here.opened.get(path).map(|t| ago(*t, now)),
                )
                .on_click(move |_, window, cx| {
                    window.dispatch_action(Box::new(crate::menus::OpenLocalRecent(target.clone())), cx)
                })
                .into_any_element()
            })
            .collect();

        // The projects of other machines, opened over a new connection.
        let remote_rows: Vec<_> = remote
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let target = (*r).clone();
                row(
                    "welcome-remote-recent",
                    i,
                    Model::dir_name(&r.path),
                    snout_host::slash_path(&r.path),
                    Some(tag(Icon::Terminal, r.host.clone())),
                    None,
                )
                .on_click(move |_, window, cx| {
                    window.dispatch_action(Box::new(crate::menus::OpenRemoteRecent(target.clone())), cx)
                })
                .into_any_element()
            })
            .collect();

        let list = if rows.is_empty() && here_rows.is_empty() && remote_rows.is_empty() {
            let text = if any_recent {
                ts!("welcome-no-recent-project-matches")
            } else {
                ts!("welcome-no-recent-projects-yet-open-a-folder-to")
            };
            div().py(px(18.)).text_color(p.fg_dim).text_center().child(crate::look::text(text)).into_any_element()
        } else {
            // With another machine in the list, each group under its own heading: this machine
            // first, then the one the window is on, then the others.
            let this_machine = self.here.label.clone();
            let grouped = connected.is_some() || !all_remote.is_empty();
            let (first, then) = match &connected {
                Some(name) => (here_rows, Some((t!("welcome-recent-on", place = name.clone()), rows))),
                None => (rows, None),
            };
            let mut gap = false;
            let mut list = div().flex().flex_col();
            if !first.is_empty() {
                list = list
                    .when(grouped, |d| d.child(section("welcome-heading-here", Icon::Folder, this_machine)))
                    .children(first);
                gap = true;
            }
            if let Some((label, rows)) = then.filter(|(_, r)| !r.is_empty()) {
                list = list
                    .child(section("welcome-heading-host", Icon::Terminal, label).when(gap, |d| d.pt(px(12.))))
                    .children(rows);
                gap = true;
            }
            if !remote_rows.is_empty() {
                list = list
                    .child(
                        section("welcome-heading-remote", Icon::Terminal, ts!("welcome-recent-remote").into())
                            .when(gap, |d| d.pt(px(12.))),
                    )
                    .children(remote_rows);
            }
            list.into_any_element()
        };

        div()
            .id("welcome")
            .debug_selector(|| "welcome".into())
            .size_full()
            .bg(p.panel)
            .text_color(p.fg)
            .overflow_y_scrollbar()
            .child(
                div()
                    .relative()
                    .w_full()
                    .flex()
                    .justify_center()
                    .px(px(if narrow { 16. } else { 32. }))
                    .pt(px(if narrow { 20. } else { 48. }))
                    .pb(px(32.))
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                let w = f32::from(bounds.size.width);
                                view.update(cx, |this, cx| {
                                    let was = is_narrow(this.width);
                                    this.width = w;
                                    if was != is_narrow(w) {
                                        cx.notify();
                                    }
                                });
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    )
                    .child(
                        div()
                            .w_full()
                            .max_w(px(760.))
                            .flex()
                            .flex_col()
                            .gap(px(22.))
                            .child(header)
                            // The small link sits in the gap above the tiles, not below a new one.
                            .child(div().mt(px(-16.)).flex().flex_col().gap(px(4.)).child(new_project).child(actions))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(12.))
                                            .child(
                                                div()
                                                    .flex_none()
                                                    .whitespace_nowrap()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(crate::look::text(ts!("welcome-recent-projects"))),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                crate::look::field(&self.search, true, Some(260.), &p)
                                                    .flex_shrink(1.)
                                                    .min_w(px(100.)),
                                            ),
                                    )
                                    .child(list),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_when_in_words() {
        assert_eq!(ago(1000, 1010), "just now");
        assert_eq!(ago(0, 125), "2 minutes ago");
        assert_eq!(ago(0, 3600), "1 hour ago");
        assert_eq!(ago(0, 90_000), "yesterday");
        assert_eq!(ago(0, 3 * 86_400), "3 days ago");
        assert_eq!(ago(0, 15 * 86_400), "2 weeks ago");
    }

    #[test]
    fn reads_the_branch_from_head() {
        let dir = std::env::temp_dir().join(format!("snout-welcome-branch-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::create_dir_all(dir.join("app")).unwrap();
        std::fs::write(dir.join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(branch_of(&*snout_host::local(), &dir.join("app")).as_deref(), Some("feature/x"));
        std::fs::write(dir.join(".git/HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(branch_of(&*snout_host::local(), &dir).as_deref(), Some("01234567"));
        let _ = std::fs::remove_dir_all(dir);
    }
}

//! The window's content: a toolbar, the activity bar beside GPUI Kit's dock area (the left
//! dock holding the panel the activity bar picked, tabs in the center, the Claude Code
//! console and Output in the bottom dock), and a status bar.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::dock::{DockArea, DockLayout, DockPlacement, DockSkin, PanelId, panel_handle};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::spinner::Spinner;
use gpui_component::{Disableable as _, Selectable as _, Sizable as _, Theme};
use snout_core::{FileEntry, PullMode, on_disk, stage_paths, unstage_paths};

use crate::menus::{self, *};
use crate::model::{Action, Dialog, Model, ModelEvent};
use crate::panels::{Activity, OutputPanel, SidePanel, WelcomePanel};
use crate::projects::ProjectsView;
use crate::settings::ThemeChoice;
use crate::theme::{self, palette};
use crate::ui::{icon, icons};

/// The center and bottom tabs, so events can bring one forward.
pub struct Tabs {
    pub welcome: PanelId,
    pub diff: Option<PanelId>,
    pub journal: Option<PanelId>,
    pub commit: Option<PanelId>,
    pub console: Option<PanelId>,
}

pub struct Workspace {
    pub model: Entity<Model>,
    pub dock: Entity<DockArea>,
    pub side: Entity<SidePanel>,
    pub tabs: Tabs,
    menu_checks: Option<(bool, bool, bool, ThemeChoice)>,
    title: String,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let projects = cx.new(|cx| ProjectsView::new(model.clone(), window, cx));
        let git = crate::git_panel::build(&model, window, cx);
        let side = cx.new(|cx| SidePanel::new(projects.into(), git, cx));
        let welcome = cx.new(|cx| WelcomePanel::new(model.clone(), cx));
        let output = cx.new(|cx| OutputPanel::new(model.clone(), cx));
        let center = crate::center::build(&model, window, cx);
        let console = crate::console::build(&model, window, cx);
        let (dock, _skin) = DockSkin::dock_area("snout-code", Some(1), window, cx);

        let welcome_id = PanelId::from(welcome.entity_id());
        let mut tabs = Tabs { welcome: welcome_id, diff: None, journal: None, commit: None, console: None };
        dock.update(cx, |d, cx| {
            let left = DockLayout::tabs().panel_view(panel_handle(side.clone()), cx);
            d.set_dock(DockPlacement::Left, left, window, cx);
            d.set_dock_size(DockPlacement::Left, px(320.), window, cx);
            let mut center_tabs = DockLayout::tabs().panel_view(panel_handle(welcome.clone()), cx);
            for (kind, view) in center {
                let id = view.panel_id(cx);
                match kind {
                    crate::center::Kind::Diff => tabs.diff = Some(id),
                    crate::center::Kind::Journal => tabs.journal = Some(id),
                    crate::center::Kind::Commit => tabs.commit = Some(id),
                }
                center_tabs = center_tabs.panel_view(view, cx);
            }
            d.set_center(center_tabs, window, cx);
            let mut bottom = DockLayout::tabs();
            if let Some(console) = console {
                tabs.console = Some(console.panel_id(cx));
                bottom = bottom.panel_view(console, cx);
            }
            bottom = bottom.panel_view(panel_handle(output.clone()), cx);
            d.set_dock(DockPlacement::Bottom, bottom, window, cx);
            d.set_dock_size(DockPlacement::Bottom, px(260.), window, cx);
            d.set_dock_collapsible(DockPlacement::Bottom, true, window, cx);
        });
        let open = model.read(cx).settings.console_open;
        if !open {
            dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Bottom, window, cx));
        }

        let subscriptions =
            vec![cx.observe(&model, |_, _, cx| cx.notify()), cx.subscribe_in(&model, window, Self::on_model_event)];
        let mut this =
            Self { model, dock, side, tabs, menu_checks: None, title: String::new(), _subscriptions: subscriptions };
        this.show_repo_tabs(window, cx);
        this
    }

    fn on_model_event(&mut self, model: &Entity<Model>, e: &ModelEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e {
            ModelEvent::Dialog => crate::dialogs::open(model, window, cx),
            ModelEvent::Theme => {
                let choice = model.read(cx).settings.theme;
                theme::apply(choice, Some(window), cx);
            }
            ModelEvent::ToggleConsole => {
                let open = model.read(cx).settings.console_open;
                self.dock.update(cx, |d, cx| {
                    if d.is_dock_open(DockPlacement::Bottom) != open {
                        d.toggle_dock(DockPlacement::Bottom, window, cx);
                    }
                    if let Some(id) = self.tabs.console.filter(|_| open) {
                        d.select_panel(id, window, cx);
                    }
                });
            }
            ModelEvent::ShowCommit => self.select(self.tabs.commit, window, cx),
            ModelEvent::ShowDiff => self.select(self.tabs.diff, window, cx),
            ModelEvent::Message => {}
        }
    }

    fn select(&self, id: Option<PanelId>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = id {
            self.dock.update(cx, |d, cx| d.select_panel(id, window, cx));
        }
    }

    /// With a repository open, start on the Journal; without one, on the welcome page.
    fn show_repo_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let has_repo = self.model.read(cx).repo.is_some();
        let id = if has_repo { self.tabs.journal.or(self.tabs.diff) } else { Some(self.tabs.welcome) };
        self.select(id, window, cx);
    }

    /// Show `activity` in the left dock, opening the dock if it is closed.
    pub fn show_activity(&mut self, activity: Activity, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dock.read(cx).is_dock_open(DockPlacement::Left) {
            self.dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
        }
        self.side.update(cx, |s, cx| {
            s.activity = activity;
            cx.notify();
        });
        cx.notify();
    }

    /// The activity bar: show `activity`, or close the dock when it already shows it.
    pub fn set_activity(&mut self, activity: Activity, window: &mut Window, cx: &mut Context<Self>) {
        let same = self.side.read(cx).activity == activity;
        let open = self.dock.read(cx).is_dock_open(DockPlacement::Left);
        if same && open {
            self.dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
        } else {
            if !open {
                self.dock.update(cx, |d, cx| d.toggle_dock(DockPlacement::Left, window, cx));
            }
            self.side.update(cx, |s, cx| {
                s.activity = activity;
                cx.notify();
            });
        }
        cx.notify();
    }

    fn send(&mut self, action: Action, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.handle(action, cx));
    }

    fn sync_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let title = m.title();
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
        let checks = menus::checks(&m.settings);
        if self.menu_checks != Some(checks) {
            cx.set_menus(menus::bar(&m.settings));
            self.menu_checks = Some(checks);
        }
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let has_repo = m.repo.is_some();
        let net = has_repo && m.job.is_none();
        let entries = m.selected_entries();
        let in_tree = m.commit_view.is_none();
        let push = match m.status.branch.ahead {
            0 => "Push".to_owned(),
            n => format!("Push ({n})"),
        };
        let console = if m.settings.console_open { "Hide Claude" } else { "Claude" };
        let tool = |id: &'static str, glyph: &'static str, label: String, enabled: bool, action: Action| {
            Button::new(id)
                .ghost()
                .small()
                .icon(icon(glyph))
                .label(label)
                .disabled(!enabled)
                .on_click(cx.listener(move |this, _, _, cx| this.send(action.clone(), cx)))
        };
        let model = self.model.clone();
        let theme = m.settings.theme;
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(8.))
            .h(px(36.))
            .child(tool("tb-open", icons::OPEN, "Open".into(), true, Action::PickFolder))
            .child(tool("tb-pull", icons::PULL, "Pull".into(), net, Action::Pull(PullMode::Default)))
            .child(tool("tb-fetch", icons::FETCH, "Fetch".into(), net, Action::Fetch))
            .child(tool("tb-push", icons::PUSH, push, net, Action::Push))
            .child(tool(
                "tb-stage",
                icons::PLUS,
                "Stage".into(),
                in_tree && entries.iter().any(FileEntry::has_unstaged),
                Action::Stage(stage_paths(&entries)),
            ))
            .child(tool(
                "tb-unstage",
                icons::DISCARD,
                "Unstage".into(),
                in_tree && entries.iter().any(FileEntry::has_staged),
                Action::Unstage(unstage_paths(&entries)),
            ))
            .child(tool(
                "tb-discard",
                icons::DISCARD,
                "Discard".into(),
                in_tree && !entries.is_empty(),
                Action::AskDiscard(entries.clone()),
            ))
            .child(tool(
                "tb-trash",
                icons::TRASH,
                "Delete".into(),
                in_tree && entries.iter().any(on_disk),
                Action::AskDeleteFiles(entries.clone()),
            ))
            .child(tool("tb-refresh", icons::FETCH, "Refresh".into(), has_repo, Action::Refresh))
            .child(tool("tb-checkout", icons::SWITCH, "Check Out…".into(), has_repo, Action::ShowCheckout))
            .child(tool("tb-branch", icons::BRANCH, "New Branch".into(), has_repo, Action::ShowNewBranch(None)))
            .child(div().flex_1())
            .child(
                Button::new("tb-theme")
                    .ghost()
                    .small()
                    .icon(icon("icons/sun-moon.svg"))
                    .tooltip("Appearance")
                    .dropdown_menu(move |mut menu, _, _| {
                        for choice in ThemeChoice::ALL {
                            let model = model.clone();
                            menu = menu.item(PopupMenuItem::new(choice.label()).checked(theme == choice).on_click(
                                move |_, _, cx| model.update(cx, |m, cx| m.handle(Action::SetTheme(choice), cx)),
                            ));
                        }
                        menu
                    }),
            )
            .child(tool("tb-github", icons::WEB, "GitHub".into(), m.github_url.is_some(), Action::OpenGithub))
            .child(tool("tb-claude", icons::SPARK, console.into(), true, Action::ToggleConsole))
    }

    fn activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let current = self.side.read(cx).activity;
        let open = self.dock.read(cx).is_dock_open(DockPlacement::Left);
        let changed = self.model.read(cx).status.files.len();
        let item = |id: &'static str, glyph: &'static str, tip: &'static str, a: Activity| {
            let on = open && current == a;
            Button::new(id)
                .ghost()
                .icon(icon(glyph))
                .tooltip(tip)
                .when(on, |b| b.selected(true))
                .on_click(cx.listener(move |this, _, w, cx| this.set_activity(a, w, cx)))
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .py(px(8.))
            .w(px(44.))
            .h_full()
            .bg(p.ground)
            .border_r_1()
            .border_color(p.border)
            .child(item("act-projects", icons::FOLDERS, "Projects (⌘1)", Activity::Projects))
            .child(div().relative().child(item("act-git", icons::BRANCH, "Git (⌘0)", Activity::Git)).when(
                changed > 0,
                |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(-2.))
                            .right(px(-4.))
                            .px(px(4.))
                            .rounded(px(7.))
                            .bg(p.accent)
                            .text_color(p.on_accent)
                            .text_size(px(10.))
                            .child(changed.to_string()),
                    )
                },
            ))
            .child(div().flex_1())
            .child(
                Button::new("act-console")
                    .ghost()
                    .icon(icon(icons::TERMINAL))
                    .tooltip("Claude Code console (⌘J)")
                    .on_click(cx.listener(|this, _, _, cx| this.send(Action::ToggleConsole, cx))),
            )
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let m = self.model.read(cx);
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .h(px(26.))
            .bg(p.ground)
            .border_t_1()
            .border_color(p.border)
            .text_size(px(12.))
            .when(m.job.is_some(), |d| d.child(Spinner::new().xsmall()))
            .child(m.message.clone())
            .child(div().flex_1())
            .when_some(m.branch_summary(), |d, s| {
                d.child(icon(icons::BRANCH).xsmall().text_color(p.fg_dim)).child(div().text_color(p.fg_dim).child(s))
            })
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_window(window, cx);
        let p = palette(cx);
        let m = self.model.read(cx);
        let free = m.dialog.is_none();
        let has_repo = m.repo.is_some();
        let idle = m.job.is_none();
        let entries = m.selected_entries();
        let in_tree = m.commit_view.is_none();
        let can_commit = m.can_commit();
        let can_draft = m.can_draft();
        let has_github = m.github_url.is_some();
        let stage = in_tree && entries.iter().any(FileEntry::has_unstaged);
        let unstage = in_tree && entries.iter().any(FileEntry::has_staged);
        let discard = in_tree && !entries.is_empty();
        let trash = in_tree && entries.iter().any(on_disk);

        macro_rules! act {
            ($el:expr, $ty:ty, $on:expr, $action:expr) => {
                $el.when($on, |d| {
                    d.on_action(cx.listener(|this, _: &$ty, _, cx| {
                        let entries = this.model.read(cx).selected_entries();
                        let _ = &entries;
                        let action: Action = $action(entries);
                        this.send(action, cx);
                    }))
                })
            };
        }
        let root = div()
            .id("workspace")
            .key_context("Workspace")
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_color(p.fg)
            .font_family(Theme::global(cx).font_family.clone())
            .on_action(cx.listener(|this, _: &ShowProjects, w, cx| this.show_activity(Activity::Projects, w, cx)))
            .on_action(cx.listener(|this, _: &ShowGit, w, cx| this.show_activity(Activity::Git, w, cx)))
            .on_action(cx.listener(|this, _: &ToggleConsole, _, cx| this.send(Action::ToggleConsole, cx)))
            .on_action(cx.listener(|this, _: &About, _, cx| {
                this.model.update(cx, |m, cx| {
                    m.dialog = Some(Dialog::About);
                    cx.emit(ModelEvent::Dialog);
                })
            }))
            .on_action(cx.listener(|this, _: &SideBySide, _, cx| {
                this.model.update(cx, |m, cx| m.set_diff_options(Some(true), None, cx))
            }))
            .on_action(cx.listener(|this, _: &Unified, _, cx| {
                this.model.update(cx, |m, cx| m.set_diff_options(Some(false), None, cx))
            }))
            .on_action(cx.listener(|this, _: &Compact, _, cx| {
                let c = this.model.read(cx).settings.compact;
                this.model.update(cx, |m, cx| m.set_diff_options(None, Some(!c), cx))
            }))
            .on_action(cx.listener(|this, _: &ThemeSystem, _, cx| this.send(Action::SetTheme(ThemeChoice::System), cx)))
            .on_action(cx.listener(|this, _: &ThemeDark, _, cx| this.send(Action::SetTheme(ThemeChoice::Dark), cx)))
            .on_action(cx.listener(|this, _: &ThemeLight, _, cx| this.send(Action::SetTheme(ThemeChoice::Light), cx)))
            .on_action(cx.listener(|_, _: &Minimize, w, _| w.minimize_window()))
            .on_action(cx.listener(|_, _: &Zoom, w, _| w.zoom_window()));
        let root = act!(root, Open, free, |_| Action::PickFolder);
        let root = act!(root, FindRepos, free, |_| Action::PickScanFolder);
        let root = act!(root, CloneRepo, free, |_| Action::ShowClone);
        let root = act!(root, NewGroup, free, |_| Action::ShowNewGroup(None));
        let root = act!(root, Refresh, free && has_repo, |_| Action::Refresh);
        let root = act!(root, Checkout, free && has_repo, |_| Action::ShowCheckout);
        let root = act!(root, NewBranch, free && has_repo, |_| Action::ShowNewBranch(None));
        let root = act!(root, Pull, free && has_repo && idle, |_| Action::Pull(PullMode::Default));
        let root = act!(root, Fetch, free && has_repo && idle, |_| Action::Fetch);
        let root = act!(root, Push, free && has_repo && idle, |_| Action::Push);
        let root = act!(root, Commit, free && can_commit, |_| Action::Commit);
        let root = act!(root, Draft, free && can_draft, |_| Action::DraftMessage);
        let root = act!(root, Stage, free && stage, |e: Vec<FileEntry>| Action::Stage(stage_paths(&e)));
        let root = act!(root, Unstage, free && unstage, |e: Vec<FileEntry>| Action::Unstage(unstage_paths(&e)));
        let root = act!(root, Discard, free && discard, Action::AskDiscard);
        let root = act!(root, Trash, free && trash, Action::AskDeleteFiles);
        let root = act!(root, Github, has_github, |_| Action::OpenGithub);
        let root = root.when(free && has_repo, |d| {
            d.on_action(cx.listener(|this, _: &Reveal, _, cx| {
                if let Some(root) = this.model.read(cx).repo.as_ref().map(|r| r.root().to_owned()) {
                    this.send(Action::Reveal(root), cx);
                }
            }))
        });

        root.child(self.toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.activity_bar(cx))
                    .child(div().flex_1().min_w_0().h_full().child(self.dock.clone())),
            )
            .child(self.status_bar(cx))
    }
}

/// Actions every window answers, whatever has focus.
pub fn app_actions(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Source, cx| cx.open_url(SOURCE_URL));
    cx.on_action(|_: &Issues, cx| cx.open_url(ISSUES_URL));
    cx.on_action(|_: &Website, cx| cx.open_url(WEBSITE_URL));
}

//! The Welcome page: the center's tab while no file is open. The wordmark and version with a
//! small drawing; Open Project, Clone Repository and New Project; and the recent projects,
//! each with its path, branch and when it was last opened, narrowed by a search box. The
//! first file opened takes its place.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;

use crate::glyphs::{Icon, icon};
use crate::model::{Action, Model};
use crate::theme::{mix, palette};

pub struct WelcomePanel {
    model: Entity<Model>,
    pub search: Entity<InputState>,
    focus: FocusHandle,
    /// The branch of each recent project, read from the host; asked again when the open
    /// project's head moves.
    branches: crate::host::Facts<Option<String>>,
    head: Option<String>,
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
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search recent projects"));
        cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        Self { model, search, focus: cx.focus_handle(), branches: Default::default(), head: None }
    }

    /// The recent projects the search box lets through.
    pub fn shown(&self, cx: &gpui::App) -> Vec<PathBuf> {
        let query = self.search.read(cx).value().to_lowercase();
        let query = query.trim();
        self.model
            .read(cx)
            .recent_projects()
            .into_iter()
            .filter(|p| query.is_empty() || p.to_string_lossy().to_lowercase().contains(query))
            .collect()
    }
}

crate::panels::panel!(WelcomePanel, "Welcome", |_, _| "Welcome".into(), true);

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
    let plural = |n: u64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match d {
        0..60 => "just now".into(),
        60..3600 => plural(d / 60, "minute"),
        3600..86_400 => plural(d / 3600, "hour"),
        86_400..172_800 => "yesterday".into(),
        172_800..1_209_600 => plural(d / 86_400, "day"),
        1_209_600..5_184_000 => plural(d / 604_800, "week"),
        _ => plural(d / 2_592_000, "month"),
    }
}

/// `path` with the home folder (`home`) as `~`.
fn tilde(path: &Path, home: &Path) -> String {
    if path.starts_with(home) && !home.as_os_str().is_empty() {
        format!("~/{}", path.strip_prefix(home).unwrap_or(path).display())
    } else {
        path.display().to_string()
    }
}

impl Render for WelcomePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let model = self.model.clone();
        let recent = self.shown(cx);
        let branches: std::collections::HashMap<PathBuf, Option<String>> = recent
            .iter()
            .filter_map(|p| {
                let b = self.branches.get(p, cx, |v| &mut v.branches, |h, p| branch_of(h, p))?;
                Some((p.clone(), b))
            })
            .collect();
        let any_recent = !self.model.read(cx).recent_projects().is_empty();
        let opened = self.model.read(cx).settings.opened.clone();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let hover = p.button_hover;
        let row_hover = p.row_hover();
        let host = crate::host::host(cx);

        let action = |id: &'static str, g: Icon, title: &'static str, note: String, a: Option<Action>| {
            let model = model.clone();
            let enabled = a.is_some();
            // Each action in its own colour when the palette is in colour.
            let fill = match (p.tinted, id) {
                (true, "welcome-clone") => p.tile(p.hues[1]),
                (true, "welcome-new") => p.tile(p.hues[0]),
                _ => (p.accent, p.on_accent),
            };
            let (fill, ink) = fill;
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .flex_1()
                .min_w_0()
                .h(px(74.))
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
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .child(div().text_size(px(12.)).text_color(p.fg_dim).whitespace_nowrap().child(note)),
                )
                .when_some(a, |d, a| {
                    d.on_click(move |_, _, cx| {
                        let a = a.clone();
                        model.update(cx, |m, cx| m.handle(a, cx));
                    })
                })
        };

        let header = div()
            .flex()
            .items_center()
            .gap(px(20.))
            .child(crate::art::drawing(snout_core::art::welcome(), 165., 110., None))
            .child(
                div().flex().flex_col().gap(px(2.)).child(crate::git_view::wordmark(&p)).child(
                    div()
                        .pl(px(62.))
                        .text_size(px(12.5))
                        .text_color(p.fg_dim)
                        .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
                ),
            );

        let actions = div()
            .flex()
            .gap(px(10.))
            .child(action(
                "welcome-open",
                Icon::Folder,
                "Open Project…",
                match crate::remote::connection(cx) {
                    Some((host, _)) => format!("A folder on {host}"),
                    None => "A folder on this Mac".into(),
                },
                Some(Action::PickFolder),
            ))
            .child(action(
                "welcome-clone",
                Icon::Fetch,
                "Clone Repository…",
                "From a Git URL".into(),
                Some(Action::ShowClone),
            ))
            .child(action(
                "welcome-new",
                Icon::Plus,
                "New Project…",
                "Java or Spring Boot, with Gradle".into(),
                Some(Action::ShowNewProject),
            ));

        let rows: Vec<_> = recent
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let name = Model::dir_name(path);
                let branch = branches.get(path).cloned().flatten();
                let when = opened.get(path).map(|t| ago(*t, now));
                let model = model.clone();
                let target = path.clone();
                div()
                    .id(("welcome-recent", i))
                    .debug_selector(move || format!("welcome-recent-{i}"))
                    .h(px(46.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(row_hover))
                    .child(
                        div()
                            .flex_none()
                            .size(px(28.))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(crate::workspace::badge_color(&name, &p))
                            .text_color(p.on_badge)
                            .text_size(px(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(crate::workspace::initials(&name)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(FontWeight::MEDIUM).child(name))
                            .child(div().h(px(16.)).flex().text_size(px(12.)).child(crate::components::path_text(
                                ("welcome-recent-path", i),
                                tilde(path, &host.info().home),
                                p.fg_dim,
                            ))),
                    )
                    .children(branch.map(|b| {
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .max_w(px(180.))
                            .text_size(px(12.))
                            .text_color(p.fg_dim)
                            .child(icon(Icon::Branch, 12., p.fg_dim))
                            .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(b))
                    }))
                    .child(
                        div()
                            .flex_none()
                            .w(px(110.))
                            .text_right()
                            .text_size(px(12.))
                            .text_color(p.fg_dim)
                            .child(when.unwrap_or_default()),
                    )
                    .on_click(move |_, _, cx| {
                        let a = Action::OpenRepo(target.clone());
                        model.update(cx, |m, cx| m.handle(a, cx));
                    })
                    .into_any_element()
            })
            .collect();

        let list = if rows.is_empty() {
            let text = if any_recent {
                "No recent project matches."
            } else {
                "No recent projects yet. Open a folder to start."
            };
            div().py(px(18.)).text_color(p.fg_dim).text_center().child(text).into_any_element()
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };

        div()
            .id("welcome")
            .debug_selector(|| "welcome".into())
            .size_full()
            .bg(p.panel)
            .text_color(p.fg)
            .overflow_y_scrollbar()
            .child(
                div().w_full().flex().justify_center().px(px(32.)).pt(px(48.)).pb(px(32.)).child(
                    div().w_full().max_w(px(760.)).flex().flex_col().gap(px(22.)).child(header).child(actions).child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(div().flex_1().font_weight(FontWeight::MEDIUM).child("Recent Projects"))
                                    .child(crate::look::field(&self.search, true, Some(260.), &p)),
                            )
                            .child(list),
                    ),
                ),
            )
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

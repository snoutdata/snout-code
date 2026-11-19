//! Settings ▸ Version Control ▸ Credentials: every repository of the list, how Git signs in to
//! its remote (the helpers in effect, where each is set, whether Git finds what they call),
//! the account they give, the repair of a helper whose program Git does not find, and a token
//! to keep in the system's keychain for a remote with none. See [`crate::credentials`].

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Task, Window, div, px,
};
use gpui_component::input::{Input, InputState};

use crate::components::{IconName, Tone, badge, callout};
use crate::credentials::{Repair, Report};
use crate::look;
use crate::model::Model;
use crate::theme::palette;

/// What is known about one repository.
#[derive(Default)]
struct Entry {
    report: Option<Report>,
    /// The account the helpers give, once asked: `Some(None)` when they give none.
    account: Option<Option<String>>,
    /// What the last repair or save said.
    note: Option<(Tone, String)>,
}

/// The token form open for one repository.
struct TokenForm {
    repo: PathBuf,
    user: Entity<InputState>,
    token: Entity<InputState>,
}

pub struct CredentialsPanel {
    entries: HashMap<PathBuf, Entry>,
    /// The repositories, in the list's order (the open project's first when it is not in it).
    repos: Vec<PathBuf>,
    started: bool,
    form: Option<TokenForm>,
    _tasks: Vec<Task<()>>,
}

impl CredentialsPanel {
    pub fn new(model: &Entity<Model>, cx: &mut Context<Self>) -> Self {
        let m = model.read(cx);
        let mut repos: Vec<PathBuf> = m.settings.repos.clone();
        if let Some(root) = m.repo_root().or_else(|| m.project_root())
            && !repos.contains(&root)
        {
            repos.insert(0, root);
        }
        Self { entries: HashMap::new(), repos, started: false, form: None, _tasks: Vec::new() }
    }

    /// Look at every repository, one after the other, off the UI thread.
    fn start(&mut self, cx: &mut Context<Self>) {
        if self.started {
            return;
        }
        self.started = true;
        let (host, extra) = (crate::host::host(cx), crate::credentials::ProgramDirs::of(cx));
        let repos = self.repos.clone();
        self._tasks.push(cx.spawn(async move |this, cx| {
            for repo in repos {
                let (host, extra, r) = (host.clone(), extra.clone(), repo.clone());
                let report = cx
                    .background_executor()
                    .spawn(async move { crate::credentials::diagnose(&*host, &r, &extra) })
                    .await;
                let gone = this
                    .update(cx, |this, cx| {
                        this.entries.entry(repo).or_default().report = Some(report);
                        cx.notify();
                    })
                    .is_err();
                if gone {
                    return;
                }
            }
        }));
    }

    /// Look at `repo` again (after a repair or a save).
    fn refresh(&mut self, repo: PathBuf, cx: &mut Context<Self>) {
        let (host, extra) = (crate::host::host(cx), crate::credentials::ProgramDirs::of(cx));
        self._tasks.push(cx.spawn(async move |this, cx| {
            let r = repo.clone();
            let report =
                cx.background_executor().spawn(async move { crate::credentials::diagnose(&*host, &r, &extra) }).await;
            let _ = this.update(cx, |this, cx| {
                let e = this.entries.entry(repo).or_default();
                e.report = Some(report);
                e.account = None;
                cx.notify();
            });
        }));
    }

    /// Ask the helpers which account they give for `repo` (they may show the keychain's own
    /// question, so only when the user asks).
    fn check(&mut self, repo: PathBuf, cx: &mut Context<Self>) {
        let Some(report) = self.entries.get(&repo).and_then(|e| e.report.clone()) else { return };
        let host = crate::host::host(cx);
        self._tasks.push(cx.spawn(async move |this, cx| {
            let account =
                cx.background_executor().spawn(async move { crate::credentials::account(&*host, &report) }).await;
            let _ = this.update(cx, |this, cx| {
                this.entries.entry(repo).or_default().account = Some(account);
                cx.notify();
            });
        }));
    }

    fn repair(&mut self, repair: Repair, cx: &mut Context<Self>) {
        let host = crate::host::host(cx);
        self._tasks.push(cx.spawn(async move |this, cx| {
            let r = repair.clone();
            let done = cx.background_executor().spawn(async move { crate::credentials::apply(&*host, &r) }).await;
            let _ = this.update(cx, |this, cx| {
                let note = match done {
                    Ok(()) => (Tone::Success, t!("credentials-repaired", path = repair.path.display().to_string())),
                    Err(e) => (Tone::Danger, t!("dialogs-sign-in-repair-failed", error = e)),
                };
                this.entries.entry(repair.repo.clone()).or_default().note = Some(note);
                this.refresh(repair.repo, cx);
            });
        }));
    }

    fn open_form(&mut self, repo: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let user = cx.new(|cx| InputState::new(window, cx).placeholder(ts!("credentials-user-placeholder")));
        let token =
            cx.new(|cx| InputState::new(window, cx).masked(true).placeholder(ts!("credentials-token-placeholder")));
        user.update(cx, |s, cx| s.focus(window, cx));
        self.form = Some(TokenForm { repo, user, token });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.form.take() else { return };
        let (user, token) = (form.user.read(cx).value().trim().to_owned(), form.token.read(cx).value().to_string());
        let Some(report) = self.entries.get(&form.repo).and_then(|e| e.report.clone()) else { return };
        if user.is_empty() || token.is_empty() {
            self.form = Some(form);
            return;
        }
        let host = crate::host::host(cx);
        let repo = form.repo.clone();
        self._tasks.push(cx.spawn(async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move { crate::credentials::save_token(&*host, &report, &user, &token) })
                .await;
            let _ = this.update(cx, |this, cx| {
                let note = match done {
                    Ok(()) => (Tone::Success, ts!("credentials-saved").to_owned()),
                    Err(e) => (Tone::Danger, e),
                };
                this.entries.entry(repo.clone()).or_default().note = Some(note);
                this.refresh(repo, cx);
            });
        }));
        cx.notify();
    }

    fn row(&self, i: usize, repo: &PathBuf, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let mono = look::mono_font(cx);
        let entry = self.entries.get(repo);
        let report = entry.and_then(|e| e.report.as_ref());
        let name = crate::model::Model::dir_name(repo);
        let status = match report {
            None => badge(ts!("credentials-looking"), Tone::Neutral, &p),
            Some(r) if r.ssh => badge(ts!("credentials-ssh"), Tone::Neutral, &p),
            Some(r) if r.remote.is_none() => badge(ts!("credentials-no-remote"), Tone::Neutral, &p),
            Some(r) if r.helpers.is_empty() => badge(ts!("credentials-no-helper"), Tone::Warning, &p),
            Some(r) if r.helpers.iter().all(|h| h.works()) => badge(ts!("credentials-ok"), Tone::Success, &p),
            Some(_) => badge(ts!("credentials-broken"), Tone::Danger, &p),
        };
        let repo_c = repo.clone();
        let mut card = div()
            .id(("credentials-repo", i))
            .debug_selector(move || format!("credentials-repo-{i}"))
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(10.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .when_some(report.and_then(Report::host_name), |d, h| {
                        d.child(div().flex_none().text_color(p.fg_dim).child(h))
                    })
                    .child(div().flex_1())
                    .child(status.flex_none()),
            )
            .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(repo.display().to_string()));
        let Some(report) = report else { return card.into_any_element() };
        for (j, h) in report.helpers.iter().enumerate() {
            let where_ = h.helper.file.as_ref().map_or_else(
                || ts!("credentials-command-line").to_owned(),
                |f| f.strip_prefix(repo).map_or_else(|_| f.display().to_string(), |r| r.display().to_string()),
            );
            card = card.child(
                div()
                    .id(("credentials-helper", i * 100 + j))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(crate::components::icon(
                        if h.works() { IconName::CircleCheck } else { IconName::CircleX },
                        12.,
                        if h.works() { p.ok } else { p.error },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .font_family(mono.clone())
                            .text_size(px(11.5))
                            .child(h.helper.value.clone()),
                    )
                    .child(div().flex_none().text_size(px(11.)).text_color(p.fg_dim).child(where_)),
            );
            for prog in h.programs.iter().filter(|p| p.on_git_path.is_none()) {
                let text = match &prog.installed_at {
                    Some(at) => t!(
                        "credentials-not-found-installed",
                        program = prog.name.clone(),
                        path = at.display().to_string()
                    ),
                    None => t!("credentials-not-found", program = prog.name.clone()),
                };
                card = card.child(div().pl(px(20.)).text_size(px(12.)).text_color(p.fg_dim).child(look::text(text)));
            }
        }
        if let Some(Some(user)) = entry.and_then(|e| e.account.clone()) {
            card = card.child(div().text_size(px(12.)).child(look::text(t!("credentials-signs-in-as", user = user))));
        } else if let Some(None) = entry.and_then(|e| e.account.clone()) {
            card = card
                .child(div().text_size(px(12.)).text_color(p.fg_dim).child(look::text(ts!("credentials-no-account"))));
        }
        if let Some((tone, text)) = entry.and_then(|e| e.note.clone()) {
            card = card.child(callout(tone, text, &p));
        }
        let mut buttons = div().flex().gap(px(6.));
        if let Some(repair) = report.repair() {
            buttons = buttons.child(
                look::small_button(("credentials-fix", i), IconName::Wrench, ts!("dialogs-fix-helper"), &p)
                    .debug_selector(move || format!("credentials-fix-{i}"))
                    .on_click(cx.listener(move |this, _, _, cx| this.repair(repair.clone(), cx))),
            );
        }
        if !report.ssh && report.remote.is_some() {
            let r = repo_c.clone();
            buttons = buttons.child(
                look::small_button(("credentials-check", i), IconName::UserRound, ts!("credentials-check"), &p)
                    .on_click(cx.listener(move |this, _, _, cx| this.check(r.clone(), cx))),
            );
            let r = repo_c.clone();
            buttons = buttons.child(
                look::small_button(("credentials-token", i), IconName::KeyRound, ts!("credentials-save-token"), &p)
                    .debug_selector(move || format!("credentials-token-{i}"))
                    .on_click(cx.listener(move |this, _, w, cx| this.open_form(r.clone(), w, cx))),
            );
        }
        card = card.child(buttons);
        if let Some(form) = self.form.as_ref().filter(|f| f.repo == *repo) {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(Input::new(&form.user))
                    .child(Input::new(&form.token))
                    .child(
                        div().text_size(px(11.5)).text_color(p.fg_dim).child(look::text(ts!("credentials-token-hint"))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(
                                look::small_button("credentials-save", IconName::Check, ts!("credentials-save"), &p)
                                    .debug_selector(|| "credentials-save".into())
                                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                            )
                            .child(
                                look::small_button("credentials-cancel", IconName::X, ts!("settings-cancel"), &p)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.form = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        card.into_any_element()
    }
}

impl Render for CredentialsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start(cx);
        let p = palette(cx);
        let repos = self.repos.clone();
        div()
            .id("credentials")
            .debug_selector(|| "credentials".into())
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().text_size(px(12.)).text_color(p.fg_dim).child(look::text(ts!("credentials-blurb"))))
            .when(repos.is_empty(), |d| {
                d.child(div().text_size(px(12.)).text_color(p.fg_dim).child(ts!("credentials-no-repositories")))
            })
            .children(repos.iter().enumerate().map(|(i, r)| self.row(i, r, cx)).collect::<Vec<_>>())
    }
}

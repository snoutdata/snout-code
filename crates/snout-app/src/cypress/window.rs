//! The Cypress tool window, on the right: the specs of the project's Cypress package, each
//! with how it went in the run in front (or, before any, in the last run the history keeps),
//! the last run's counts and time, and the buttons that run them: Run all, Rerun failed, a
//! spec on its own, Open Cypress. The browser and the environment runs go to are picked here
//! (the Tests window has the same switches). A run that went wrong in a known way shows what
//! happened, with its fix a click away; the results themselves are the Tests window's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, FontWeight, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Task, Window, div, px, uniform_list,
};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::tooltip::Tooltip;

use super::{Browser, Fix};
use crate::glyphs::{Icon, icon};
use crate::jstest::{Runner, Suite};
use crate::playwright::report::{Run, State, duration_text};
use crate::playwright::run::Scope;
use crate::playwright::tests::PwTests;
use crate::playwright::window::{button, mark};
use crate::run::{Runs, RunsEvent};
use crate::theme::{mix, palette};

/// What is known of the project's Cypress package, found off the UI thread: the Tests
/// window reads it too.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CypressInfo {
    pub suite: Option<Suite>,
    pub specs: Vec<PathBuf>,
    pub version: Option<String>,
    /// The browsers installed (Electron always).
    pub browsers: Vec<Browser>,
}

impl Global for CypressInfo {}

fn info(cx: &App) -> CypressInfo {
    cx.try_global::<CypressInfo>().cloned().unwrap_or_default()
}

/// How a spec went: the worst of its tests, how many ran of how many, and its time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpecState {
    pub state: Option<State>,
    pub done: usize,
    pub total: usize,
    pub millis: u64,
}

impl SpecState {
    fn add(&mut self, s: State, millis: u64) {
        self.total += 1;
        if !matches!(s, State::Queued | State::Running) {
            self.done += 1;
        }
        self.millis += millis;
        let rank = |s: Option<State>| match s {
            Some(State::Running) => 6,
            Some(State::Failed | State::Interrupted) => 5,
            Some(State::Flaky) => 4,
            Some(State::Passed) => 3,
            Some(State::Queued) => 2,
            Some(State::Skipped) => 1,
            None => 0,
        };
        if rank(Some(s)) > rank(self.state) {
            self.state = Some(s);
        }
    }
}

/// How each spec went in `run` (a Cypress run of the package at `dir`), by its path.
pub fn spec_states(run: &Run, dir: &Path) -> BTreeMap<PathBuf, SpecState> {
    let mut out: BTreeMap<PathBuf, SpecState> = BTreeMap::new();
    for t in run.tests.values() {
        let file = if t.file.is_relative() { dir.join(&t.file) } else { t.file.clone() };
        out.entry(file).or_default().add(t.state(run.over), t.millis().unwrap_or(0));
    }
    out
}

pub struct CypressWindow {
    tests: Entity<PwTests>,
    _runs: Entity<Runs>,
    scanning: bool,
    _scan: Option<Task<()>>,
    /// The suite and the session last looked at: a new one, or a run that ended, looks again.
    seen: (Option<Suite>, Option<u64>, bool),
    focus: FocusHandle,
}

crate::panels::panel!(CypressWindow, "Cypress", |_, _| "Cypress".into(), true);

impl CypressWindow {
    pub fn new(tests: Entity<PwTests>, runs: Entity<Runs>, cx: &mut Context<Self>) -> Self {
        cx.observe(&tests, |this, _, cx| this.tests_changed(cx)).detach();
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        let mut this = Self {
            tests,
            _runs: runs,
            scanning: false,
            _scan: None,
            seen: (None, None, false),
            focus: cx.focus_handle(),
        };
        this.tests_changed(cx);
        this
    }

    /// The package with Cypress, or the run in front, changed: look at the specs again.
    fn tests_changed(&mut self, cx: &mut Context<Self>) {
        let t = self.tests.read(cx);
        let suite = t.cypress_suite(cx);
        let now = (suite.clone(), t.live, t.running(cx));
        if now != self.seen {
            self.seen = now;
            self.refresh(cx);
        }
        cx.notify();
    }

    /// Find the specs, the Cypress version and the browsers, off the UI thread.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let suite = self.tests.read(cx).cypress_suite(cx);
        let host = crate::host::host(cx);
        self.scanning = true;
        let scan = cx.background_executor().spawn(async move {
            let Some(suite) = suite else { return CypressInfo::default() };
            CypressInfo {
                specs: super::specs(&*host, &suite.dir),
                version: super::version(&*host, &suite.dir),
                browsers: Browser::ALL.into_iter().filter(|b| b.installed(&*host)).collect(),
                suite: Some(suite),
            }
        });
        self._scan = Some(cx.spawn(async move |this, cx| {
            let found = scan.await;
            let _ = this.update(cx, |this, cx| {
                this.scanning = false;
                if cx.try_global::<CypressInfo>() != Some(&found) {
                    cx.set_global(found);
                }
                cx.notify();
            });
        }));
    }

    /// The run in front, when it is this package's Cypress run.
    fn run<'a>(&self, suite: &Suite, cx: &'a App) -> Option<&'a Run> {
        let run = self.tests.read(cx).run(cx)?;
        (run.runner == Some(Runner::Cypress) && run.dir.as_ref() == Some(&suite.dir)).then_some(run)
    }

    /// How each spec went: in the run in front, else in the last run of the history that ran it.
    fn states(&self, suite: &Suite, specs: &[PathBuf], cx: &App) -> BTreeMap<PathBuf, SpecState> {
        let mut out = self.run(suite, cx).map(|r| spec_states(r, &suite.dir)).unwrap_or_default();
        let history = &self.tests.read(cx).history;
        for spec in specs {
            if out.contains_key(spec) {
                continue;
            }
            let rel = spec.strip_prefix(&suite.dir).unwrap_or(spec).to_string_lossy().replace('\\', "/");
            let needle = format!(" › {rel} › ");
            for record in history.runs.iter().rev() {
                let mut st = SpecState::default();
                for (key, cell) in &record.results {
                    if key.contains(&needle) {
                        st.add(cell.state, cell.millis);
                    }
                }
                if st.total > 0 {
                    out.insert(spec.clone(), st);
                    break;
                }
            }
        }
        out
    }
}

/// Run these specs (those that failed, the run in front's or the history's).
fn start_specs(tests: &Entity<PwTests>, specs: &[PathBuf], cx: &mut App) {
    let Some(suite) = info(cx).suite else { return };
    let scope = crate::jstest::Scope::Tests(specs.iter().map(|f| (f.clone(), Vec::new())).collect());
    tests.update(cx, |t, cx| {
        t.tab = crate::playwright::tests::Tab::Live;
        t.start_unit(suite, scope, cx)
    });
    if let Some(runs) = crate::run::runs(cx) {
        runs.update(cx, |_, cx| cx.emit(RunsEvent::ShowTests));
    }
}

/// Run `scope` of the package with Cypress, and show the results.
fn start(tests: &Entity<PwTests>, scope: Scope, cx: &mut App) {
    let Some(suite) = info(cx).suite else { return };
    tests.update(cx, |t, cx| t.start_in(suite, scope, cx));
    if let Some(runs) = crate::run::runs(cx) {
        runs.update(cx, |_, cx| cx.emit(RunsEvent::ShowTests));
    }
}

/// The browser switch: the browser Cypress runs in, of those installed.
pub fn browser_switch(id: &'static str, cx: &App) -> AnyElement {
    let p = palette(cx);
    let runs = crate::run::runs(cx);
    let now = runs.as_ref().map(|r| r.read(cx).cypress_browser()).unwrap_or_default();
    let mut installed = info(cx).browsers;
    if installed.is_empty() {
        installed.push(Browser::Electron);
    }
    if !installed.contains(&now) {
        installed.push(now);
    }
    let trigger = button(id, now.name(), Some(Icon::Eye), p.accent, &p)
        .tooltip(|w, cx| Tooltip::new(ts!("cypress-the-browser-the-specs-run-in")).build(w, cx))
        .child(icon(Icon::ChevronDown, 11., p.fg_dim));
    crate::look::Trigger(trigger)
        .dropdown_menu(move |mut menu, _, _| {
            for b in &installed {
                let (b, runs) = (*b, runs.clone());
                menu = menu.item(PopupMenuItem::new(b.name()).checked(b == now).on_click(move |_, _, cx| {
                    if let Some(runs) = &runs {
                        runs.update(cx, |r, cx| r.set_cypress_browser(b, cx));
                    }
                }));
            }
            menu
        })
        .into_any_element()
}

/// What went wrong in the Cypress run in front, as one notice: the problem that matters most
/// in full with its fixes, then a line for each lesser one with its own. None when nothing did.
/// Red only when the results cannot be trusted; amber when they stand.
pub fn problem_cards(tests: &Entity<PwTests>, cx: &App) -> Option<AnyElement> {
    let p = palette(cx);
    let t = tests.read(cx);
    let run = t.run(cx).filter(|r| r.runner == Some(Runner::Cypress) && r.over)?;
    let c = run.counts();
    let passed = c.failed == 0 && c.passed + c.flaky > 0;
    let ranked = super::ranked(&run.cypress.problems, passed);
    let (lead, rest) = ranked.split_first()?;
    let info = info(cx);
    let version = info.version.clone();
    let mut installed = info.browsers.clone();
    if installed.is_empty() {
        installed.push(Browser::Electron);
    }
    let fix_buttons = |ix: usize, problem: &super::Problem, first_accent: bool| {
        let mut row = div().flex().flex_wrap().gap(px(6.));
        for (n, fix) in problem.fixes(run.browser, &installed).into_iter().enumerate() {
            let tests = tests.clone();
            let id = format!("cypress-fix-{ix}-{n}");
            let ink = if n == 0 && first_accent { p.accent } else { p.fg };
            let glyph = match fix {
                Fix::UpdateCypress | Fix::InstallBinary | Fix::InstallRosetta => Icon::Pull,
                Fix::UseBrowser(_) => Icon::Eye,
                Fix::EditEnvironments => Icon::Endpoint,
                Fix::RunAgain => Icon::Rerun,
            };
            row = row.child(button(id, fix.label(), Some(glyph), ink, &p).on_click(move |_, w, cx| {
                if fix == Fix::EditEnvironments {
                    w.dispatch_action(Box::new(crate::menus::EditEnvironments), cx);
                } else {
                    tests.update(cx, |t, cx| t.fix(fix, cx));
                }
            }));
        }
        row
    };
    let ink = if lead.minor(passed) { p.warn } else { p.error };
    let mut card = div()
        .debug_selector(|| "cypress-problem-0".to_owned())
        .p(px(10.))
        .rounded(px(6.))
        .border_1()
        .border_color(mix(ink, 0.45, p.border))
        .bg(p.wash(ink))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(icon(Icon::Warning, 14., ink))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::look::text(lead.title(passed)))),
        )
        .child(div().pt(px(4.)).text_color(p.fg_dim).child(crate::look::text(lead.detail(version.as_deref(), passed))))
        .child(div().pt(px(6.)).child(fix_buttons(0, lead, true)));
    for (i, problem) in rest.iter().enumerate() {
        let ix = i + 1;
        let small = if problem.minor(passed) { p.warn } else { p.error };
        card = card.child(
            div()
                .debug_selector(move || format!("cypress-problem-{ix}"))
                .mt(px(10.))
                .pt(px(8.))
                .border_t_1()
                .border_color(mix(ink, 0.3, p.border))
                .flex()
                .items_start()
                .gap(px(8.))
                .child(div().pt(px(2.)).child(icon(Icon::Warning, 12., small)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_size(px(12.)).text_color(p.fg_dim).child(crate::look::text(format!(
                            "{}. {}",
                            problem.title(passed),
                            problem.detail(version.as_deref(), passed)
                        ))))
                        .child(div().pt(px(6.)).child(fix_buttons(ix, problem, false))),
                ),
        );
    }
    Some(div().flex_none().px(px(12.)).py(px(8.)).border_b_1().border_color(p.border).child(card).into_any_element())
}

impl Render for CypressWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let base = div().size_full().flex().flex_col().bg(p.ground).text_size(px(crate::look::BODY)).text_color(p.fg);
        let found = info(cx);
        let Some(suite) = found.suite.clone() else {
            return base.child(
                crate::components::EmptyState::new(snout_core::art::spots::no_tests(), ts!("cypress-no-cypress"))
                    .hint(ts!("cypress-no-cypress-hint"))
                    .compact(),
            );
        };
        let t = self.tests.read(cx);
        let running = t.running(cx) && self.run(&suite, cx).is_some();
        let watch = t.watch;
        let run = self.run(&suite, cx);
        let counts = run.map(Run::counts);
        let last =
            t.history.runs.iter().rev().find(|r| r.results.keys().any(|k| suite_key(k, &found.specs, &suite.dir)));
        let states = Rc::new(self.states(&suite, &found.specs, cx));
        let failed_specs: Vec<PathBuf> = states
            .iter()
            .filter(|(_, s)| matches!(s.state, Some(State::Failed | State::Interrupted)))
            .map(|(f, _)| f.clone())
            .collect();

        // The buttons.
        let tests = self.tests.clone();
        // Two groups, the run's buttons and its switches: side by side when the window is wide
        // (docked at the bottom), the switches under the buttons when it is narrow.
        let mut bar = div().flex().flex_wrap().items_center().gap(px(6.));
        bar = bar.child(if running {
            let tests = tests.clone();
            button("cypress-stop", ts!("cmd-stop"), Some(Icon::Stop), p.error, &p)
                .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.stop(cx)))
                .into_any_element()
        } else {
            let tests = tests.clone();
            button("cypress-run-all", ts!("tests-run-all"), Some(Icon::Play), p.ok, &p)
                .on_click(move |_, _, cx| start(&tests, Scope::All, cx))
                .into_any_element()
        });
        if !running && !failed_specs.is_empty() {
            let tests = tests.clone();
            let specs = failed_specs.clone();
            bar = bar.child(
                button("cypress-rerun-failed", ts!("tests-rerun-failed"), Some(Icon::RerunFailed), p.error, &p)
                    .on_click(move |_, _, cx| start_specs(&tests, &specs, cx)),
            );
        }
        {
            let tests = tests.clone();
            bar = bar.child(
                button("cypress-open", ts!("cypress-open-cypress"), Some(Icon::Bug), p.fg, &p)
                    .tooltip(|w, cx| Tooltip::new(ts!("cypress-open-cypress-tip")).build(w, cx))
                    .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.open_cypress(cx))),
            );
        }
        let switches = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .child(browser_switch("cypress-browser", cx))
            .child(crate::playwright::window::TestsWindow::environment_switch("cypress-environment", true, cx))
            .child({
                let tests = tests.clone();
                button(
                    "cypress-watch",
                    ts!("tests-watch-label"),
                    Some(Icon::Eye),
                    if watch { p.accent } else { p.fg_dim },
                    &p,
                )
                .when(watch, |d| d.border_color(p.accent).bg(p.wash(p.accent)))
                .tooltip(|w, cx| Tooltip::new(ts!("cypress-watch-tip")).build(w, cx))
                .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.toggle_watch(cx)))
            });

        // The last run, in short.
        let total_of = |r: &crate::playwright::history::Record| (r.passed + r.failed + r.flaky + r.skipped) as u64;
        let (headline, ink) = match (counts, last) {
            _ if running => (ts!("console-running").to_owned(), p.accent),
            (Some(c), _) if c.failed > 0 => {
                (t!("cypress-failed-of", failed = (c.failed as u64), total = (c.total as u64)), p.error)
            }
            (Some(c), _) if c.total > 0 => {
                (t!("cypress-passed-of", passed = ((c.passed + c.flaky) as u64), total = (c.total as u64)), p.ok)
            }
            (None, Some(r)) if r.failed > 0 => {
                (t!("cypress-failed-of", failed = (r.failed as u64), total = total_of(r)), p.error)
            }
            (None, Some(r)) => {
                (t!("cypress-passed-of", passed = ((r.passed + r.flaky) as u64), total = total_of(r)), p.ok)
            }
            _ => (ts!("tests-not-run-yet").to_owned(), p.fg_dim),
        };
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let when = match (run, last) {
            (Some(_), _) => duration_text(t.elapsed_ms(cx)),
            (None, Some(r)) => format!("{} · {}", duration_text(r.millis), crate::welcome::ago(r.when, now)),
            _ => String::new(),
        };
        let progress = counts.filter(|c| c.total > 0).map(|c| c.done as f32 / c.total as f32);
        let detail = format!(
            "{} · {}{}",
            found.version.as_deref().map_or_else(|| "Cypress".to_owned(), |v| format!("Cypress {v}")),
            t!("cypress-specs", count = (found.specs.len() as u64)),
            if suite.dir == t.root.clone().unwrap_or_default() {
                String::new()
            } else {
                format!(" · {}", suite.label(&t.root.clone().unwrap_or_default()))
            }
        );
        let summary = div()
            .flex_none()
            .mx(px(8.))
            .mb(px(8.))
            .p(px(10.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border)
            .bg(p.panel)
            .debug_selector(|| "cypress-summary".into())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(ink).child(crate::look::text(headline)))
                    .child(div().flex_1())
                    .child(div().text_color(p.fg_dim).text_size(px(12.)).child(crate::look::text(when))),
            )
            .when_some(progress.filter(|_| running), |d, f| {
                d.child(div().mt(px(6.)).h(px(4.)).rounded(px(2.)).bg(mix(p.border, 0.5, p.ground)).child(
                    div().h_full().rounded(px(2.)).w(gpui::relative(f)).bg(if counts.is_some_and(|c| c.failed > 0) {
                        p.error
                    } else {
                        p.ok
                    }),
                ))
            })
            .child(div().pt(px(4.)).text_size(px(12.)).text_color(p.fg_dim).child(crate::look::text(detail)));

        let problems = problem_cards(&self.tests, cx);

        // The specs.
        let specs = Rc::new(found.specs.clone());
        // Named from the folder they all sit in (`cypress/e2e`), which the heading says.
        let dir = common_dir(&found.specs).unwrap_or_else(|| suite.dir.clone());
        let folder = dir.strip_prefix(&suite.dir).unwrap_or(&dir).to_string_lossy().replace('\\', "/");
        let count = specs.len();
        let list = if count == 0 {
            div()
                .flex_1()
                .p(px(12.))
                .text_color(p.fg_dim)
                .child(crate::look::text(if self.scanning {
                    ts!("cypress-looking-for-specs").to_owned()
                } else {
                    t!("cypress-no-specs", pattern = super::DEFAULT_SPECS)
                }))
                .into_any_element()
        } else {
            let tests = self.tests.clone();
            uniform_list("cypress-specs", count, move |range, _, cx| {
                let p = palette(cx);
                range
                    .map(|i| {
                        let file = specs[i].clone();
                        let rel = file.strip_prefix(&dir).unwrap_or(&file).to_string_lossy().replace('\\', "/");
                        let st = states.get(&file).cloned().unwrap_or_default();
                        let (glyph, ink) = st.state.map_or((Icon::More, mix(p.fg_dim, 0.6, p.ground)), |s| mark(s, &p));
                        let note = if st.total == 0 {
                            String::new()
                        } else if st.state == Some(State::Running) {
                            format!("{}/{}", st.done, st.total)
                        } else {
                            format!("{}/{} · {}", st.done, st.total, duration_text(st.millis))
                        };
                        let (open_file, run_file) = (file.clone(), file.clone());
                        let tests = tests.clone();
                        let group: gpui::SharedString = format!("cypress-spec-{i}").into();
                        crate::components::list_row_h(
                            ("cypress-spec", i),
                            crate::components::RowState::default(),
                            crate::components::TREE_ROW_H,
                            &p,
                        )
                        .group(group.clone())
                        .debug_selector(move || format!("cypress-spec-{i}"))
                        .px(px(10.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .child(icon(glyph, 13., ink))
                        .child(div().flex_1().min_w_0().truncate().child(rel))
                        .child(div().flex_none().text_size(px(11.5)).text_color(p.fg_dim).child(note))
                        .child(
                            div()
                                .id(("cypress-run-spec", i))
                                .debug_selector(move || format!("cypress-run-spec-{i}"))
                                .flex_none()
                                .opacity(0.)
                                .group_hover(group, |s| s.opacity(1.))
                                .tooltip(|w, cx| Tooltip::new(ts!("cypress-run-this-spec")).build(w, cx))
                                .child(icon(Icon::Play, 13., p.ok))
                                .on_click({
                                    let tests = tests.clone();
                                    let file = run_file.clone();
                                    move |_, _, cx| {
                                        cx.stop_propagation();
                                        let scope = Scope::At { file: file.clone(), line: None, project: None };
                                        start(&tests, scope, cx);
                                    }
                                }),
                        )
                        .on_click(move |e, _, cx| {
                            if e.click_count() >= 2 {
                                let scope = Scope::At { file: run_file.clone(), line: None, project: None };
                                start(&tests, scope, cx);
                            } else if let Some(runs) = crate::run::runs(cx) {
                                let file = open_file.clone();
                                runs.update(cx, |_, cx| cx.emit(RunsEvent::OpenAt(file, 0)));
                            }
                        })
                    })
                    .collect()
            })
            .flex_1()
            .w_full()
            .py(px(2.))
            .into_any_element()
        };
        let show = div()
            .id("cypress-show-results")
            .debug_selector(|| "cypress-show-results".into())
            .flex_none()
            .px(px(10.))
            .py(px(6.))
            .border_t_1()
            .border_color(p.border)
            .flex()
            .items_center()
            .gap(px(6.))
            .text_color(p.accent)
            .cursor_pointer()
            .child(icon(Icon::TestFolder, 13., p.accent))
            .child(crate::look::text(ts!("cypress-show-results")))
            .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::ShowTests), cx));
        base.child(
            div()
                .flex_none()
                .px(px(8.))
                .pt(px(8.))
                .pb(px(8.))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.))
                .child(bar)
                .child(switches),
        )
        .child(summary)
        .children(problems)
        .child(
            div()
                .px(px(10.))
                .pb(px(2.))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.fg_dim)
                .child(crate::look::text(if folder.is_empty() {
                    ts!("cypress-specs-heading").to_owned()
                } else {
                    format!("{} · {folder}", ts!("cypress-specs-heading"))
                })),
        )
        .child(list)
        .child(show)
    }
}

/// Whether a history key (`project › file › titles`) is of one of the package's specs.
fn suite_key(key: &str, specs: &[PathBuf], dir: &Path) -> bool {
    specs.iter().any(|s| {
        let rel = s.strip_prefix(dir).unwrap_or(s).to_string_lossy().replace('\\', "/");
        key.contains(&format!(" › {rel} › "))
    })
}

/// The folder all of `specs` are in (none for no specs).
fn common_dir(specs: &[PathBuf]) -> Option<PathBuf> {
    let mut dir = specs.first()?.parent()?.to_path_buf();
    for s in specs {
        while !s.starts_with(&dir) {
            dir = dir.parent()?.to_path_buf();
        }
    }
    Some(dir)
}

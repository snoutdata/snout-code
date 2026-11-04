//! The Tests tool window of a Playwright project: a summary bar (progress, counts, time and
//! what is left, the environment, Watch, Rerun Failed, Run or Stop), the workers and the
//! step each is at, the results grid (a row per test, a column per project, the last ten
//! results and the flaky rate), and the picked test's detail: its error at its line, its
//! screenshot comparisons, its steps, and from its trace the filmstrip, the failed requests
//! and the console's errors. The History tab lists the runs kept.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, FocusHandle, FontWeight, Hsla, Image,
    ImageFormat, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Task, Window, div, img, px,
};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::tooltip::Tooltip;

use super::history::cell_key;
use super::report::{Row, State, Test, duration_text};
use super::run::Scope;
use super::tests::{Filter, PwTests, Tab};
use super::trace::{self, Trace};
use crate::glyphs::{Icon, icon};
use crate::look;
use crate::run::{Launch, Place, RunsEvent};
use crate::theme::{Palette, mix, palette};

/// The trace of the test picked, read off the UI thread, and its filmstrip's frames.
struct Loaded {
    path: PathBuf,
    trace: Result<Trace, String>,
    frames: Vec<(f64, Arc<Image>)>,
}

pub struct TestsWindow {
    pub tests: Entity<PwTests>,
    focus: FocusHandle,
    filter: Entity<InputState>,
    trace: Option<Loaded>,
    loading: Option<(PathBuf, Task<()>)>,
}

crate::panels::panel!(TestsWindow, "Tests", |_, _| "Tests".into(), true);

/// The glyph and colour of a state.
pub fn mark(s: State, p: &Palette) -> (Icon, Hsla) {
    match s {
        State::Passed => (Icon::Check, p.ok),
        State::Failed => (Icon::Close, p.error),
        State::Interrupted => (Icon::Stop, p.error),
        State::Flaky => (Icon::Warning, p.warn),
        State::Skipped => (Icon::Minus, p.fg_dim),
        State::Running => (Icon::Refresh, p.accent),
        State::Queued => (Icon::More, mix(p.fg_dim, 0.6, p.ground)),
    }
}

fn button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    g: Option<Icon>,
    ink: Hsla,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let hover = p.button_hover;
    let sel = id.clone();
    div()
        .id(gpui::ElementId::Name(id))
        .debug_selector(move || sel.to_string())
        .flex_none()
        .h(px(26.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(p.border)
        .bg(p.button)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .children(g.map(|g| icon(g, 13., ink)))
        .child(div().whitespace_nowrap().child(label.into()))
}

fn chip(id: &'static str, label: String, on: bool, p: &Palette) -> gpui::Stateful<gpui::Div> {
    let hover = p.button_hover;
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex_none()
        .h(px(24.))
        .px(px(10.))
        .flex()
        .items_center()
        .rounded(px(12.))
        .border_1()
        .border_color(if on { p.accent } else { p.border })
        .when(on, |d| d.bg(p.wash(p.accent)))
        .text_color(if on { p.fg } else { p.fg_dim })
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label)
}

fn section(title: &'static str, p: &Palette) -> impl IntoElement {
    div().pt(px(12.)).pb(px(4.)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(p.fg_dim).child(title)
}

impl TestsWindow {
    pub fn new(tests: Entity<PwTests>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&tests, |this, _, cx| {
            this.load_trace(cx);
            cx.notify();
        })
        .detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter tests"));
        let t = tests.clone();
        cx.subscribe(&filter, move |_, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().to_string();
                t.update(cx, |t, cx| {
                    t.text = text;
                    cx.notify();
                });
            }
        })
        .detach();
        Self { tests, focus: cx.focus_handle(), filter, trace: None, loading: None }
    }

    /// Read the trace of the test picked, when it has one not read yet.
    fn load_trace(&mut self, cx: &mut Context<Self>) {
        let path = self
            .tests
            .read(cx)
            .selected_test(cx)
            .and_then(|t| t.last_try())
            .and_then(|t| t.trace())
            .map(Path::to_path_buf);
        let Some(path) = path else {
            self.trace = None;
            return;
        };
        if self.trace.as_ref().is_some_and(|l| l.path == path) || self.loading.as_ref().is_some_and(|(p, _)| *p == path)
        {
            return;
        }
        let read = cx.background_executor().spawn({
            let path = path.clone();
            async move {
                let trace = trace::read(&path).map_err(|e| e.to_string());
                let frames = trace
                    .as_ref()
                    .map(|t| {
                        t.filmstrip(8)
                            .into_iter()
                            .filter_map(|f| {
                                let bytes = trace::entry(&path, &f.entry).ok()?;
                                Some((f.at, Arc::new(Image::from_bytes(ImageFormat::Jpeg, bytes))))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Loaded { path, trace, frames }
            }
        });
        let task = cx.spawn(async move |this, cx| {
            let loaded = read.await;
            let _ = this.update(cx, |this, cx| {
                this.loading = None;
                this.trace = Some(loaded);
                cx.notify();
            });
        });
        self.loading = Some((path, task));
    }

    fn summary(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let t = self.tests.read(cx);
        let running = t.running(cx);
        let run = t.run(cx);
        let c = run.map(|r| r.counts()).unwrap_or_default();
        let total = run.map_or_else(|| t.listing.as_ref().map_or(0, |l| l.tests.len()), |r| r.tests.len());
        let (state_text, ink) = match run {
            None => ("Not run yet".to_owned(), p.fg_dim),
            Some(_) if running => ("Running".to_owned(), p.accent),
            Some(r) if !r.errors.is_empty() && r.tests.is_empty() => ("Did not start".to_owned(), p.error),
            Some(_) if c.failed > 0 => ("Failed".to_owned(), p.error),
            Some(r) if r.status.as_deref() == Some("interrupted") => ("Stopped".to_owned(), p.fg_dim),
            Some(_) => ("Passed".to_owned(), p.ok),
        };
        let projects = t.projects(cx);
        let workers = run.map_or(0, |r| r.workers);
        let detail = if projects.is_empty() {
            String::new()
        } else if workers > 0 {
            format!("{} · {workers} {}", projects.join(", "), if workers == 1 { "worker" } else { "workers" })
        } else {
            projects.join(", ")
        };
        let elapsed = t.elapsed_ms(cx);
        let eta = run.filter(|_| running).and_then(|r| t.history.eta(r));
        let time = match eta {
            Some(left) => format!("{} · about {} left", duration_text(elapsed), duration_text(left)),
            None if run.is_some() => duration_text(elapsed),
            None => String::new(),
        };
        let progress = if total == 0 { 0. } else { c.done as f32 / total as f32 };
        let env = Self::environment_switch("tests-environment", cx);
        let tests = self.tests.clone();
        let failed_any = c.failed > 0 && !running;
        let watch = t.watch;
        let count = |g: Icon, n: usize, ink: Hsla, what: &'static str| {
            div().flex().items_center().gap(px(4.)).child(icon(g, 12., ink)).child(format!("{n} {what}"))
        };
        let tab = t.tab;
        let tab_button = |id: &'static str, label: &'static str, which: Tab| {
            let tests = self.tests.clone();
            chip(id, label.into(), tab == which, &p)
                .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.set_tab(which, cx)))
        };
        div()
            .flex_none()
            .px(px(12.))
            .py(px(8.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .border_b_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(tab_button("tests-tab-live", "Live run", Tab::Live))
                    .child(tab_button("tests-tab-history", "History", Tab::History))
                    .child(div().w(px(8.)))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(ink).child(state_text))
                    .child(
                        div().text_color(p.fg_dim).whitespace_nowrap().overflow_hidden().text_ellipsis().child(detail),
                    )
                    .child(div().flex_1())
                    .child(env)
                    .child(
                        button("tests-watch", "Watch", Some(Icon::Eye), if watch { p.accent } else { p.fg_dim }, &p)
                            .when(watch, |d| d.border_color(p.accent).bg(p.wash(p.accent)))
                            .tooltip(|w, cx| {
                                Tooltip::new("Run with the browsers showing, one test at a time").build(w, cx)
                            })
                            .on_click({
                                let tests = tests.clone();
                                move |_, _, cx| tests.update(cx, |t, cx| t.toggle_watch(cx))
                            }),
                    )
                    .when(failed_any, |d| {
                        d.child(
                            button("tests-rerun-failed", "Rerun failed", Some(Icon::RerunFailed), p.error, &p)
                                .on_click({
                                    let tests = tests.clone();
                                    move |_, _, cx| tests.update(cx, |t, cx| t.start(Scope::LastFailed, cx))
                                }),
                        )
                    })
                    .child(if running {
                        button("tests-stop", "Stop", Some(Icon::Stop), p.error, &p)
                            .on_click({
                                let tests = tests.clone();
                                move |_, _, cx| tests.update(cx, |t, cx| t.stop(cx))
                            })
                            .into_any_element()
                    } else {
                        button("tests-run-all", "Run all", Some(Icon::Play), p.ok, &p)
                            .on_click({
                                let tests = tests.clone();
                                move |_, _, cx| tests.update(cx, |t, cx| t.start(Scope::All, cx))
                            })
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .text_size(px(12.))
                    .child(div().w(px(180.)).h(px(6.)).rounded(px(3.)).bg(mix(p.border, 0.5, p.ground)).child(
                        div().h_full().rounded(px(3.)).w(px(180. * progress)).bg(if c.failed > 0 {
                            p.error
                        } else {
                            p.ok
                        }),
                    ))
                    .child(div().child(format!("{} / {total}", c.done)))
                    .child(count(Icon::Check, c.passed, p.ok, "passed"))
                    .child(count(Icon::Close, c.failed, p.error, "failed"))
                    .child(count(Icon::Warning, c.flaky, p.warn, "flaky"))
                    .child(count(Icon::Minus, c.skipped, p.fg_dim, "skipped"))
                    .child(div().flex_1())
                    .child(div().text_color(p.fg_dim).child(time)),
            )
            .into_any_element()
    }

    /// The environment switcher: the one runs go against, and Edit Environments.
    pub fn environment_switch(id: &'static str, cx: &App) -> AnyElement {
        let p = palette(cx);
        let runs = crate::run::runs(cx);
        let (current, all) = runs
            .as_ref()
            .map(|r| {
                let r = r.read(cx);
                (r.environment().cloned(), r.project.environments.clone())
            })
            .unwrap_or_default();
        let label = current.as_ref().map_or_else(|| "No environment".to_owned(), |e| e.name.clone());
        let detail = current.as_ref().map(|e| e.detail()).filter(|d| !d.is_empty());
        let current_name = current.map(|e| e.name);
        let trigger = button(id, label, Some(Icon::Endpoint), p.accent, &p)
            .children(
                detail.map(|d| div().text_color(p.fg_dim).max_w(px(180.)).overflow_hidden().text_ellipsis().child(d)),
            )
            .child(icon(Icon::ChevronDown, 11., p.fg_dim));
        crate::look::Trigger(trigger)
            .dropdown_menu(move |mut menu, _, _| {
                let pick = |name: Option<String>| {
                    let runs = runs.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                        if let Some(r) = &runs {
                            r.update(cx, |r, cx| r.select_environment(name.clone(), cx));
                        }
                    }
                };
                menu = menu.item(
                    PopupMenuItem::new("No environment (the configuration's own)")
                        .checked(current_name.is_none())
                        .on_click(pick(None)),
                );
                for e in &all {
                    let on = current_name.as_deref() == Some(e.name.as_str());
                    menu =
                        menu.item(PopupMenuItem::new(e.name.clone()).checked(on).on_click(pick(Some(e.name.clone()))));
                }
                menu.separator().item(
                    PopupMenuItem::new("Edit Environments…")
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::EditEnvironments), cx)),
                )
            })
            .into_any_element()
    }

    fn workers(&self, cx: &App) -> Option<AnyElement> {
        let p = palette(cx);
        let t = self.tests.read(cx);
        let run = t.run(cx).filter(|_| t.running(cx))?;
        if run.busy.is_empty() {
            return None;
        }
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        let mono = look::mono_font(cx);
        Some(
            div()
                .id("tests-workers")
                .debug_selector(|| "tests-workers".into())
                .flex_none()
                .px(px(12.))
                .py(px(6.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .border_b_1()
                .border_color(p.border)
                .text_size(px(12.))
                .children(run.busy.iter().map(|(ix, w)| {
                    let test = run.tests.get(&w.test);
                    let title = test.map_or_else(String::new, |t| t.title().to_owned());
                    let project = test.map_or_else(String::new, |t| t.project.clone());
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(div().w(px(22.)).text_color(p.fg_dim).child(format!("#{}", ix + 1)))
                        .child(div().w(px(70.)).text_color(p.accent).overflow_hidden().child(project))
                        .child(div().w(px(220.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(title))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .text_color(p.fg_dim)
                                .child(w.step.clone().unwrap_or_default()),
                        )
                        .child(div().text_color(p.fg_dim).child(duration_text(now.saturating_sub(w.since))))
                }))
                .into_any_element(),
        )
    }

    fn filters(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let t = self.tests.read(cx);
        let c = t.run(cx).map(|r| r.counts()).unwrap_or_default();
        let total = t.rows(cx).len();
        let now = t.filter;
        let chip_for = |id: &'static str, label: String, f: Filter| {
            let tests = self.tests.clone();
            chip(id, label, now == f, &p).on_click(move |_, _, cx| tests.update(cx, |t, cx| t.set_filter(f, cx)))
        };
        div()
            .flex_none()
            .px(px(12.))
            .py(px(6.))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(chip_for("tests-filter-all", format!("All {total}"), Filter::All))
            .child(chip_for("tests-filter-failed", format!("Failed {}", c.failed), Filter::Failed))
            .child(chip_for("tests-filter-flaky", format!("Flaky {}", c.flaky), Filter::Flaky))
            .child(chip_for("tests-filter-slow", "Slow, over 4 s".into(), Filter::Slow))
            .child(div().flex_1())
            .child(div().w(px(220.)).child(Input::new(&self.filter).small()))
            .into_any_element()
    }

    fn grid(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let t = self.tests.read(cx);
        let projects = t.projects(cx);
        let rows: Vec<Row> = t.rows(cx).into_iter().filter(|r| t.shows(r, cx)).collect();
        let selected = t.selected.clone();
        let col = px(72.);
        let header = div()
            .flex_none()
            .px(px(12.))
            .h(px(26.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(11.))
            .text_color(p.fg_dim)
            .border_b_1()
            .border_color(mix(p.border, 0.6, p.ground))
            .child(div().flex_1().child("Test"))
            .children(projects.iter().map(|pr| div().w(col).flex_none().child(pr.clone())))
            .child(div().w(px(100.)).flex_none().child("Last 10 runs"))
            .child(div().w(px(48.)).flex_none().child("Flaky"));
        let mut list: Vec<AnyElement> = Vec::new();
        let mut file = String::new();
        for (i, row) in rows.iter().enumerate() {
            if row.file != file {
                file = row.file.clone();
                let n = rows.iter().filter(|r| r.file == file).count();
                list.push(
                    div()
                        .px(px(12.))
                        .pt(px(8.))
                        .pb(px(2.))
                        .flex()
                        .gap(px(8.))
                        .text_size(px(12.))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(file.clone()))
                        .child(
                            div().text_color(p.fg_dim).child(format!("{n} {}", if n == 1 { "test" } else { "tests" })),
                        )
                        .into_any_element(),
                );
            }
            let on = selected.as_ref().is_some_and(|(k, _)| *k == row.key);
            let name = row.path.join(" › ");
            let first = projects.iter().find(|pr| row.cells.contains_key(*pr)).cloned().unwrap_or_default();
            let mut cells = Vec::new();
            for pr in &projects {
                let cell = t.cell(row, pr, cx);
                let (g, ink) =
                    cell.as_ref().map_or((Icon::More, mix(p.fg_dim, 0.4, p.ground)), |(s, _, _)| mark(*s, &p));
                let time = cell.as_ref().and_then(|(_, ms, _)| *ms).map(duration_text).unwrap_or_default();
                let tests = self.tests.clone();
                let (key, project) = (row.key.clone(), pr.clone());
                let picked = selected.as_ref().is_some_and(|(k, sp)| *k == row.key && sp == pr);
                cells.push(
                    div()
                        .id(gpui::ElementId::Name(format!("tests-cell-{i}-{pr}").into()))
                        .w(col)
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .rounded(px(4.))
                        .when(picked, |d| d.bg(p.wash(p.accent)))
                        .when(row.cells.contains_key(pr), |d| {
                            d.cursor_pointer().on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                tests.update(cx, |t, cx| t.select(key.clone(), project.clone(), cx));
                            })
                        })
                        .child(icon(g, 12., ink))
                        .child(div().text_size(px(11.)).text_color(p.fg_dim).child(time)),
                );
            }
            // Each past run's worst result over the projects, the newest last.
            let mut last: Vec<State> = Vec::new();
            for pr in &projects {
                let mine = t.history.last(&cell_key(pr, &row.key), 10);
                let pad = mine.len().max(last.len());
                let (a, b) = (last.len(), mine.len());
                let mut merged = Vec::with_capacity(pad);
                for ix in 0..pad {
                    let x = (ix + a).checked_sub(pad).and_then(|j| last.get(j)).copied();
                    let y = (ix + b).checked_sub(pad).and_then(|j| mine.get(j)).copied();
                    merged.push(match (x, y) {
                        (Some(x), Some(y)) => worst(x, y),
                        (Some(x), None) | (None, Some(x)) => x,
                        (None, None) => State::Queued,
                    });
                }
                last = merged;
            }
            let flaky = projects.iter().filter_map(|pr| t.history.flaky_rate(&cell_key(pr, &row.key))).max();
            let tests = self.tests.clone();
            let (key, project) = (row.key.clone(), first.clone());
            let hover = p.button_hover;
            list.push(
                div()
                    .id(("tests-row", i))
                    .debug_selector(move || format!("tests-row-{i}"))
                    .px(px(12.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_l_2()
                    .border_color(if on { p.accent } else { gpui::transparent_black() })
                    .when(on, |d| d.bg(p.wash(p.accent)))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(move |_, _, cx| {
                        let (key, project) = (key.clone(), project.clone());
                        tests.update(cx, |t, cx| {
                            // The row picks its first failure, else its first cell.
                            let failed = t
                                .projects(cx)
                                .into_iter()
                                .find(|pr| {
                                    t.rows(cx)
                                        .iter()
                                        .find(|r| r.key == key)
                                        .and_then(|r| t.cell(r, pr, cx))
                                        .is_some_and(|(s, _, _)| {
                                            matches!(s, State::Failed | State::Interrupted | State::Flaky)
                                        })
                                })
                                .unwrap_or(project);
                            t.select(key, failed, cx)
                        });
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(150.))
                            .pl(px(10.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(name),
                    )
                    .children(cells)
                    .child(div().w(px(100.)).flex_none().flex().gap(px(2.)).children((0..10usize).map(|ix| {
                        let s = ix.checked_sub(10 - last.len()).and_then(|j| last.get(j)).copied();
                        let ink = match s {
                            Some(State::Passed) => p.ok,
                            Some(State::Failed | State::Interrupted) => p.error,
                            Some(State::Flaky) => p.warn,
                            _ => mix(p.border, 0.7, p.ground),
                        };
                        div().w(px(8.)).h(px(14.)).rounded(px(2.)).bg(ink)
                    })))
                    .child(
                        div()
                            .w(px(48.))
                            .flex_none()
                            .text_size(px(12.))
                            .text_color(p.warn)
                            .child(flaky.map(|f| format!("{f}%")).unwrap_or_default()),
                    )
                    .into_any_element(),
            );
        }
        if rows.is_empty() {
            let note = if t.listing_now {
                "Finding the tests…".to_owned()
            } else if let Some(e) = &t.listing_error {
                e.clone()
            } else if t.filter != Filter::All || !t.text.is_empty() {
                "No test matches the filter.".to_owned()
            } else {
                "No tests yet. Run them, or add a spec file.".to_owned()
            };
            list.push(div().p(px(16.)).text_color(p.fg_dim).child(note).into_any_element());
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id("tests-grid")
                    .debug_selector(|| "tests-grid".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .children(list),
            )
            .into_any_element()
    }

    fn detail(&self, test: &Test, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let mono = look::mono_font(cx);
        let t = self.tests.read(cx);
        let run = t.run(cx);
        let state = run.map_or(State::Queued, |r| test.state(r.over));
        let (g, ink) = mark(state, &p);
        let last = test.last_try();
        let badge = match state {
            State::Failed => format!("Failed in {}", test.project),
            State::Flaky => format!("Flaky in {}", test.project),
            other => format!("{} in {}", other.label(), test.project),
        };
        let retries = test.tries.len().saturating_sub(1);
        let location = run.and_then(|r| r.location(&test.id));
        let where_text = format!(
            "{}:{}{}",
            test.file.to_string_lossy().replace('\\', "/"),
            test.line,
            if retries > 0 {
                format!(" · {} {}", retries, if retries == 1 { "retry" } else { "retries" })
            } else {
                String::new()
            }
        );
        let mut body: Vec<AnyElement> = Vec::new();
        // The error, and the lines around it.
        let error = last.and_then(|l| l.errors.first());
        if let Some(e) = error {
            body.push(
                div()
                    .mt(px(8.))
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(p.wash(p.error))
                    .border_1()
                    .border_color(mix(p.error, 0.4, p.panel))
                    .font_family(mono.clone())
                    .text_size(px(12.))
                    .overflow_hidden()
                    .child(div().text_color(p.error).child(e.headline()))
                    .children((!e.snippet.is_empty()).then(|| {
                        let snippet = super::report::strip_ansi(&e.snippet);
                        div().mt(px(6.)).flex().flex_col().children(snippet.lines().map(|l| {
                            let hit = l.trim_start().starts_with('>');
                            div()
                                .whitespace_nowrap()
                                .text_color(if hit { p.fg } else { p.fg_dim })
                                .when(hit, |d| d.bg(p.wash(p.error)))
                                .child(l.to_owned())
                        }))
                    }))
                    .into_any_element(),
            );
        }
        // Screenshot comparisons.
        for cmp in last.map(|l| l.comparisons()).unwrap_or_default() {
            let pane = |label: &'static str, path: Option<&Path>| {
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().text_size(px(11.)).text_color(p.fg_dim).child(label))
                    .child(
                        div()
                            .h(px(120.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(p.border)
                            .bg(p.card)
                            .overflow_hidden()
                            .children(
                                path.map(|path| img(path.to_path_buf()).size_full().object_fit(ObjectFit::Contain)),
                            ),
                    )
            };
            let tests = self.tests.clone();
            body.push(section("Screenshot comparison", &p).into_any_element());
            body.push(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(pane("Expected", cmp.expected.as_deref()))
                    .child(pane("Actual", Some(&cmp.actual)))
                    .child(pane("Difference", cmp.diff.as_deref()))
                    .into_any_element(),
            );
            body.push(
                div()
                    .pt(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        button("tests-accept-screenshot", "Accept new screenshot", Some(Icon::Check), p.ok, &p)
                            .tooltip(|w, cx| {
                                Tooltip::new("Runs the test again, keeping its screenshots as they are now")
                                    .build(w, cx)
                            })
                            .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.accept_screenshots(cx))),
                    )
                    .child(div().text_size(px(12.)).text_color(p.fg_dim).child(cmp.name.clone()))
                    .into_any_element(),
            );
        }
        // The steps.
        if let Some(l) = last.filter(|l| !l.steps.is_empty()) {
            body.push(section("Steps", &p).into_any_element());
            body.push(
                div()
                    .flex()
                    .flex_col()
                    .text_size(px(12.))
                    .children(l.steps.iter().filter(|s| s.category != "hook" || s.error.is_some()).take(200).map(|s| {
                        let (g, ink) = if s.error.is_some() { (Icon::Close, p.error) } else { (Icon::Check, p.ok) };
                        div()
                            .pl(px(16. * s.depth as f32))
                            .h(px(20.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(icon(g, 11., ink))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(s.title.clone()),
                            )
                            .child(div().text_color(p.fg_dim).child(duration_text(s.duration.max(0.) as u64)))
                    }))
                    .into_any_element(),
            );
        }
        // From the trace.
        let trace_path = last.and_then(|l| l.trace()).map(Path::to_path_buf);
        if let Some(loaded) = self.trace.as_ref().filter(|l| Some(&l.path) == trace_path.as_ref()) {
            body.push(section("From the trace", &p).into_any_element());
            match &loaded.trace {
                Ok(tr) => {
                    if !loaded.frames.is_empty() {
                        body.push(
                            div()
                                .id("tests-filmstrip")
                                .debug_selector(|| "tests-filmstrip".into())
                                .flex()
                                .gap(px(6.))
                                .overflow_x_scrollbar()
                                .children(loaded.frames.iter().map(|(at, image)| {
                                    div()
                                        .flex_none()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.))
                                        .child(
                                            div()
                                                .w(px(112.))
                                                .h(px(70.))
                                                .rounded(px(3.))
                                                .border_1()
                                                .border_color(p.border)
                                                .overflow_hidden()
                                                .child(img(image.clone()).size_full().object_fit(ObjectFit::Cover)),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(p.fg_dim)
                                                .child(duration_text(at.max(0.) as u64)),
                                        )
                                }))
                                .into_any_element(),
                        );
                    }
                    for r in &tr.failed_requests {
                        let status = if r.status == 0 { "failed".to_owned() } else { r.status.to_string() };
                        body.push(
                            div()
                                .pt(px(4.))
                                .flex()
                                .gap(px(8.))
                                .font_family(mono.clone())
                                .text_size(px(12.))
                                .child(div().text_color(p.error).child(status))
                                .child(div().child(r.method.clone()))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .child(r.url.clone()),
                                )
                                .children(r.millis.map(|m| div().text_color(p.fg_dim).child(duration_text(m))))
                                .into_any_element(),
                        );
                    }
                    for e in tr.console_errors.iter().take(20) {
                        body.push(
                            div()
                                .pt(px(4.))
                                .flex()
                                .gap(px(8.))
                                .font_family(mono.clone())
                                .text_size(px(12.))
                                .child(div().text_color(p.error).child("console error"))
                                .child(div().flex_1().min_w_0().child(e.clone()))
                                .into_any_element(),
                        );
                    }
                    if tr.failed_requests.is_empty() && tr.console_errors.is_empty() {
                        body.push(
                            div()
                                .pt(px(4.))
                                .text_size(px(12.))
                                .text_color(p.fg_dim)
                                .child(format!("{} actions; no failed requests or console errors.", tr.actions.len()))
                                .into_any_element(),
                        );
                    }
                }
                Err(why) => body.push(
                    div()
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child(format!("The trace could not be read: {why}"))
                        .into_any_element(),
                ),
            }
        } else if trace_path.is_some() {
            body.push(section("From the trace", &p).into_any_element());
            body.push(div().text_size(px(12.)).text_color(p.fg_dim).child("Reading the trace…").into_any_element());
        }
        // What it printed.
        if let Some(out) = last.map(|l| l.output.trim()).filter(|o| !o.is_empty()) {
            body.push(section("Output", &p).into_any_element());
            body.push(
                div()
                    .font_family(mono.clone())
                    .text_size(px(12.))
                    .flex()
                    .flex_col()
                    .children(super::report::strip_ansi(out).lines().take(200).map(|l| div().child(l.to_owned())))
                    .into_any_element(),
            );
        }
        // What can be done with it.
        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some(path) = trace_path.clone() {
            let tests = self.tests.clone();
            actions.push(
                button("tests-open-trace", "Open full trace", Some(Icon::Search), p.accent, &p)
                    .on_click(move |_, _, cx| open_trace(&tests, &path, cx))
                    .into_any_element(),
            );
        }
        if let Some(video) = last.and_then(|l| l.video()).map(Path::to_path_buf) {
            actions.push(
                button("tests-video", "Video", Some(Icon::Play), p.fg, &p)
                    .on_click(move |_, _, cx| cx.open_with_system(&video))
                    .into_any_element(),
            );
        }
        {
            let tests = self.tests.clone();
            actions.push(
                button("tests-run-this", "Run this test", Some(Icon::Rerun), p.ok, &p)
                    .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.run_selected(cx)))
                    .into_any_element(),
            );
            let tests = self.tests.clone();
            actions.push(
                button("tests-debug-this", "Debug this test", Some(Icon::Bug), p.fg, &p)
                    .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.debug_selected(cx)))
                    .into_any_element(),
            );
            let tests = self.tests.clone();
            actions.push(
                button("tests-inspect-this", "Debug in Inspector", Some(Icon::Search), p.fg, &p)
                    .on_click(move |_, _, cx| tests.update(cx, |t, cx| t.inspect_selected(cx)))
                    .into_any_element(),
            );
        }
        if let Some((file, line)) = location {
            actions.push(
                button("tests-open-source", "Show in editor", Some(Icon::Code), p.fg, &p)
                    .on_click(move |_, _, cx| {
                        if let Some(runs) = crate::run::runs(cx) {
                            let (file, line) = (file.clone(), line.saturating_sub(1));
                            runs.update(cx, |_, cx| cx.emit(RunsEvent::OpenAt(file, line)));
                        }
                    })
                    .into_any_element(),
            );
        }
        if let Some(e) = error {
            let text = format!("{}\n\n{}", e.text(), super::report::strip_ansi(&e.stack));
            actions.push(
                button("tests-copy-error", "Copy error", Some(Icon::Copy), p.fg, &p)
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
                    .into_any_element(),
            );
        }
        div()
            .id("tests-detail")
            .debug_selector(|| "tests-detail".into())
            .size_full()
            .p(px(12.))
            .overflow_y_scrollbar()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon(g, 14., ink))
                    .child(div().text_size(px(11.)).text_color(ink).font_weight(FontWeight::SEMIBOLD).child(badge)),
            )
            .child(div().pt(px(4.)).font_weight(FontWeight::SEMIBOLD).child(test.path.join(" › ")))
            .child(div().text_size(px(12.)).text_color(p.fg_dim).child(where_text))
            .children(body)
            .child(div().pt(px(14.)).flex().flex_wrap().gap(px(6.)).children(actions))
            .into_any_element()
    }

    fn history(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let t = self.tests.read(cx);
        let runs = &t.history.runs;
        let cols: [(&str, f32); 9] = [
            ("When", 130.),
            ("What", 160.),
            ("Environment", 110.),
            ("Branch", 120.),
            ("Commit", 70.),
            ("Passed", 56.),
            ("Failed", 56.),
            ("Flaky", 50.),
            ("Time", 64.),
        ];
        let header = div()
            .flex_none()
            .px(px(12.))
            .h(px(26.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(11.))
            .text_color(p.fg_dim)
            .border_b_1()
            .border_color(p.border)
            .children(cols.iter().map(|(n, w)| div().w(px(*w)).flex_none().child(*n)))
            .child(div().flex_1().child("New since the run before"));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let rows = runs.iter().enumerate().rev().map(|(i, r)| {
            let before = i.checked_sub(1).and_then(|j| runs.get(j));
            let fresh = r.new_failures(before);
            let fresh_text = match fresh.len() {
                0 => String::new(),
                1 => fresh[0].clone(),
                n => format!("{} and {} more", fresh[0], n - 1),
            };
            let cell = |w: f32, text: String, ink: Hsla| {
                div()
                    .w(px(w))
                    .flex_none()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_color(ink)
                    .child(text)
            };
            div()
                .px(px(12.))
                .h(px(26.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(12.))
                .child(cell(130., ago(now.saturating_sub(r.when)), p.fg))
                .child(cell(160., r.scope.clone(), p.fg))
                .child(cell(110., r.environment.clone().unwrap_or_else(|| "none".into()), p.fg_dim))
                .child(cell(120., r.branch.clone().unwrap_or_default(), p.fg_dim))
                .child(cell(
                    70.,
                    r.commit.as_deref().map(|c| c.chars().take(7).collect()).unwrap_or_default(),
                    p.fg_dim,
                ))
                .child(cell(56., r.passed.to_string(), p.ok))
                .child(cell(56., r.failed.to_string(), if r.failed > 0 { p.error } else { p.fg_dim }))
                .child(cell(50., r.flaky.to_string(), if r.flaky > 0 { p.warn } else { p.fg_dim }))
                .child(cell(64., duration_text(r.millis), p.fg_dim))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(p.error)
                        .child(fresh_text),
                )
                .into_any_element()
        });
        let rows: Vec<AnyElement> = rows.collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(12.))
                    .pt(px(8.))
                    .flex()
                    .gap(px(8.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Runs of this project"))
                    .child(div().text_color(p.fg_dim).child("Kept on this machine, outside the repository")),
            )
            .child(header)
            .child(
                div()
                    .id("tests-history")
                    .debug_selector(|| "tests-history".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .when(rows.is_empty(), |d| {
                        d.child(div().p(px(16.)).text_color(p.fg_dim).child("No runs kept yet."))
                    })
                    .children(rows),
            )
            .into_any_element()
    }
}

/// The worse of two results, for a run's mark over its projects.
fn worst(a: State, b: State) -> State {
    let rank = |s: State| match s {
        State::Failed | State::Interrupted => 3,
        State::Flaky => 2,
        State::Passed => 1,
        _ => 0,
    };
    if rank(b) > rank(a) { b } else { a }
}

/// `12 min ago`, `yesterday`, …
fn ago(secs: u64) -> String {
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        86_400..172_800 => "yesterday".into(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// Open the trace in Playwright's own viewer (it has the DOM snapshots), from the Build window.
fn open_trace(tests: &Entity<PwTests>, path: &Path, cx: &mut App) {
    let Some(root) = tests.read(cx).root.clone() else { return };
    let Some(runs) = crate::run::runs(cx) else { return };
    let host = crate::host::host(cx);
    let path = path.to_owned();
    let find = cx.background_executor().spawn(async move {
        let cli = super::run::cli(&*host, &root).ok_or("The Playwright test runner is not installed.")?;
        let wanted = crate::lang::node::wanted(&*host, &root, Some(&root));
        let all = crate::lang::node::discover(&*host);
        let node = crate::lang::node::choose(&wanted, &all).cloned().ok_or("No Node fits this project.")?;
        Ok::<_, &str>(crate::run::process::ProcessLaunch {
            title: "Trace viewer".into(),
            program: node.bin.clone(),
            args: vec![cli.display().to_string(), "show-trace".into(), path.display().to_string()],
            cwd: root,
            env: vec![("PATH".into(), crate::lang::typescript::path_with(&*host, &node))],
            place: Place::Build,
            report: false,
            debug_node: None,
        })
    });
    cx.spawn(async move |cx| {
        if let Ok(launch) = find.await {
            let _ = runs.update(cx, |r, cx| r.start(Launch::Process(launch), cx));
        }
    })
    .detach();
}

impl Render for TestsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let t = self.tests.read(cx);
        let base = div().size_full().flex().flex_col().bg(p.ground).text_size(px(look::BODY)).text_color(p.fg);
        if t.root.is_none() {
            return base.child(
                crate::components::EmptyState::new(snout_core::art::spots::no_tests(), "No Playwright project")
                    .hint("Open a folder with a playwright.config.ts to run its tests here.")
                    .compact(),
            );
        }
        let tab = t.tab;
        let selected = t.selected_test(cx).cloned();
        let summary = self.summary(cx);
        if tab == Tab::History {
            return base.child(summary).child(div().flex_1().min_h_0().child(self.history(cx)));
        }
        let workers = self.workers(cx);
        let filters = self.filters(cx);
        let grid = self.grid(cx);
        let detail = selected.map(|s| self.detail(&s, cx));
        base.child(summary).children(workers).child(filters).child(
            div().flex_1().min_h_0().flex().child(div().flex_1().min_w_0().h_full().child(grid)).children(detail.map(
                |d| {
                    div()
                        .w(gpui::relative(0.4))
                        .max_w(px(460.))
                        .flex_none()
                        .h_full()
                        .border_l_1()
                        .border_color(p.border)
                        .bg(p.panel)
                        .child(d)
                },
            )),
        )
    }
}

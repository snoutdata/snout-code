//! Spring in the editor: a gutter lane of marks from the engine's `snout/springInjections`
//! (a leaf on a bean's declaration, an arrow into a socket on an injection point, a globe
//! on an endpoint's handler), their tooltips and menus, and the resolved values of
//! `@Value` placeholders as ghost text at the end of their lines.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point,
    StatefulInteractiveElement as _, Styled as _, canvas, div, point, px, size,
};
use gpui_component::input::RopeExt as _;
use serde_json::json;

use super::run_gutter::{GutterMenu, Lanes, visible_lines};
use super::{EditorPanel, font_size};
use crate::glyphs::{Icon, icon};
use crate::spring::{self, FileSpring, MarkKind, SpringLine, Target};
use crate::theme::{mix, palette};

/// An entry of a Spring mark's menu.
#[derive(Clone, Debug, PartialEq)]
pub enum SpringItem {
    /// A line of text that heads what follows.
    Heading(String),
    /// Go somewhere: a bean, or a place a bean is injected into.
    Go { label: String, detail: String, target: Target },
    /// Put an endpoint's path on the clipboard.
    CopyPath(String),
    /// Show the endpoint in the Spring tool window.
    ShowEndpoint(String),
    /// Something to say, with nowhere to go.
    Note(String),
}

/// How long typing pauses before the marks and hints are asked for again.
const SETTLE: Duration = Duration::from_millis(900);

impl EditorPanel {
    /// Ask the engine for the file's Spring marks.
    pub(super) fn refresh_spring(&mut self, cx: &mut Context<Self>) {
        if !spring::has_marks(&self.path) || self.class_file.is_some() {
            self.spring = None;
            return;
        }
        let Some(model) = spring::spring(cx) else { return };
        self.spring_seen = model.read(cx).revision;
        let task = model.read(cx).file_marks(&self.path, cx);
        let path = self.path.clone();
        self._spring = Some(cx.spawn(async move |this, cx| {
            let found = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.path == path
                    && let Some(found) = found
                {
                    this.spring = Some(Rc::new(found));
                    cx.notify();
                }
            });
        }));
        self.refresh_hints(cx);
    }

    /// Ask the server for the `@Value` hints of the file.
    pub(super) fn refresh_hints(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.lang.clone().filter(|_| self.class_file.is_none()) else { return };
        let rope = self.state.read(cx).text().clone();
        if !rope.to_string().contains("@Value") {
            if !self.hints.is_empty() {
                self.hints = Rc::new(Vec::new());
                cx.notify();
            }
            return;
        }
        let params = json!({
            "textDocument": { "uri": link.uri.as_str() },
            "range": spring::whole(rope.lines_len() as u32)
        });
        let call = link.ask(&rope, "textDocument/inlayHint", params, cx);
        let revision = self.revision;
        self._hints = Some(cx.spawn(async move |this, cx| {
            let Ok(v) = call.await else { return };
            let hints = spring::parse_hints(&v);
            let _ = this.update(cx, |this, cx| {
                if this.revision == revision {
                    this.hints = Rc::new(hints);
                    cx.notify();
                }
            });
        }));
    }

    /// The text changed: ask again once typing pauses.
    pub(super) fn spring_changed(&mut self, cx: &mut Context<Self>) {
        if !spring::has_marks(&self.path) {
            return;
        }
        self._spring_settle = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, cx| this.refresh_spring(cx));
        }));
    }

    /// The Spring model was read again: so may the file's marks be.
    pub(super) fn spring_model_changed(&mut self, cx: &mut Context<Self>) {
        let Some(model) = spring::spring(cx) else { return };
        if model.read(cx).revision != self.spring_seen {
            self.refresh_spring(cx);
        }
    }

    /// The marks, and the `@Value` hints, drawn over the editor from its own layout.
    pub(super) fn spring_gutter(&self, cx: &App) -> impl IntoElement {
        let p = palette(cx);
        let green = spring::green(&p);
        let marks = self.spring.clone();
        let hints = self.hints.clone();
        let (state, pad) = (self.state.clone(), self.gutter_pad);
        let mono = crate::look::mono_font(cx);
        let size_pt = font_size(cx);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let s = state.read(cx);
                let Some(line_height) = s.line_height() else { return };
                let visible = visible_lines(s);
                let top_of = |line: u32| visible.iter().find(|(l, _)| *l == line as usize).map(|(_, t)| *t);
                if let Some(marks) = &marks {
                    let l = Lanes::new(bounds, px(0.), pad);
                    let (sl, sr) = l.spring();
                    for m in &marks.lines {
                        let (Some(kind), Some(top)) = (m.mark(), top_of(m.line)) else { continue };
                        let b = gpui::Bounds::new(point(sl, top + px(1.)), size(sr - sl, line_height - px(2.)));
                        let (g, ink) = match kind {
                            MarkKind::Bean => (Icon::Leaf, green),
                            MarkKind::Endpoint => (Icon::Endpoint, green),
                            MarkKind::Injection { problem: false } => (Icon::Inject, green),
                            MarkKind::Injection { problem: true } => (Icon::Inject, p.warn),
                        };
                        crate::glyphs::paint(g, b, ink, window);
                    }
                }
                // `@Value` hints: after the end of their line, quiet and in italics.
                let rope = s.text();
                let places: Vec<(gpui::Bounds<Pixels>, &String)> = hints
                    .iter()
                    .filter_map(|(line, text)| {
                        let line = *line as usize;
                        if top_of(line as u32).is_none() || line >= rope.lines_len() {
                            return None;
                        }
                        let end = rope.line_end_offset(line);
                        s.range_to_bounds(&(end..end)).map(|b| (b, text))
                    })
                    .collect();
                for (b, text) in places {
                    let run = gpui::TextRun {
                        len: text.len(),
                        font: gpui::Font { style: gpui::FontStyle::Italic, ..gpui::font(mono.clone()) },
                        color: mix(green, 0.75, p.panel),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped = window.text_system().shape_line(text.clone().into(), px(size_pt * 0.92), &[run], None);
                    let at = point(b.right() + px(16.), b.top());
                    let _ = shaped.paint(at, line_height, gpui::TextAlign::Left, None, window, cx);
                }
            },
        )
        .absolute()
        .inset_0()
    }

    /// The line whose Spring mark is under `at`, if any.
    pub(super) fn spring_mark_at(&self, at: Point<Pixels>, cx: &App) -> Option<SpringLine> {
        let marks = self.spring.as_ref()?;
        let l = self.lanes.get();
        let (sl, sr) = l.spring();
        if l.bounds.size.width <= px(0.) || at.x < sl || at.x >= sr {
            return None;
        }
        let s = self.state.read(cx);
        let line_height = s.line_height()?;
        let (line, _) = visible_lines(s).into_iter().find(|(_, top)| at.y >= *top && at.y < *top + line_height)?;
        marks.line(line as u32).cloned()
    }

    /// The pointer moved over the editor: show a Spring mark's tooltip while it is on one.
    pub(super) fn gutter_hover(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let tip = self.spring_mark_at(at, cx).map(|m| (m.line, m.tooltip()));
        let now = tip.map(|(line, text)| (line, at, text));
        let same = match (&self.gutter_tip, &now) {
            (Some((a, _, _)), Some((b, _, _))) => a == b,
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.gutter_tip = now;
            cx.notify();
        }
    }

    /// A click on a Spring mark: go to the one bean an injection resolves to, else a menu.
    pub(super) fn spring_press(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let Some(line) = self.spring_mark_at(at, cx) else { return false };
        self.gutter_tip = None;
        let items = spring_items(&line);
        let gos: Vec<&Target> = items
            .iter()
            .filter_map(|i| match i {
                SpringItem::Go { target, .. } => Some(target),
                _ => None,
            })
            .collect();
        let only_injections = line.beans.is_empty() && line.endpoints.is_empty();
        if only_injections && let [target] = gos.as_slice() {
            let target = (*target).clone();
            if let Some(model) = spring::spring(cx) {
                model.update(cx, |s, cx| s.open(target, cx));
            }
            cx.notify();
            return true;
        }
        self.gutter_menu = Some(GutterMenu::Spring { at, items });
        cx.notify();
        true
    }

    /// The tooltip of the Spring mark under the pointer.
    pub(super) fn spring_tooltip(&self, cx: &App) -> Option<AnyElement> {
        let (_, at, text) = self.gutter_tip.as_ref()?;
        if self.gutter_menu.is_some() {
            return None;
        }
        let p = palette(cx);
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        Some(
            gpui::deferred(
                gpui::anchored().position(*at + point(px(12.), px(14.))).snap_to_window_with_margin(px(8.)).child(
                    div()
                        .id("spring-tooltip")
                        .debug_selector(|| "spring-tooltip".into())
                        .px(px(8.))
                        .py(px(5.))
                        .rounded(px(5.))
                        .bg(p.card)
                        .border_1()
                        .border_color(mix(p.border, 0.8, p.card))
                        .shadow_md()
                        .text_size(px(12.))
                        .text_color(p.fg)
                        .font_family(crate::look::ui_font(cx))
                        .children(lines.into_iter().map(|l| div().whitespace_nowrap().child(l))),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    /// The body of a Spring mark's menu.
    pub(super) fn spring_menu_body(&self, items: &[SpringItem], cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let hover = p.button_hover;
        let green = spring::green(&p);
        // A bean injected in many places makes a long list: it scrolls.
        let mut col = div()
            .id("spring-menu-list")
            .flex()
            .flex_col()
            .min_w(px(240.))
            .max_w(px(640.))
            .max_h(px(440.))
            .overflow_y_scroll();
        for (ix, item) in items.iter().enumerate() {
            let row = |id: &'static str| {
                div()
                    .id((id, ix))
                    .debug_selector(move || format!("{id}-{ix}"))
                    .h(px(26.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
            };
            col = match item.clone() {
                SpringItem::Heading(text) => col.child(
                    div()
                        .px(px(10.))
                        .pt(px(if ix == 0 { 2. } else { 8. }))
                        .pb(px(3.))
                        .text_size(px(11.5))
                        .text_color(p.fg_dim)
                        .whitespace_nowrap()
                        .child(text),
                ),
                SpringItem::Note(text) => {
                    col.child(div().px(px(10.)).py(px(4.)).max_w(px(520.)).text_color(p.fg_dim).child(text))
                }
                SpringItem::Go { label, detail, target } => col.child(
                    row("spring-go")
                        .child(icon(if target.uri.is_some() { Icon::Leaf } else { Icon::Jar }, 13., green))
                        .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(label))
                        .child(div().flex_1().min_w(px(12.)))
                        .child(
                            div()
                                .flex_none()
                                .text_color(p.fg_dim)
                                .text_size(px(11.5))
                                .whitespace_nowrap()
                                .child(detail),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.gutter_menu = None;
                            if let Some(model) = spring::spring(cx) {
                                let target = target.clone();
                                model.update(cx, |s, cx| s.open(target, cx));
                            }
                            cx.notify();
                        })),
                ),
                SpringItem::CopyPath(path) => {
                    col.child(row("spring-copy").child(icon(Icon::Copy, 13., p.fg_dim)).child("Copy Path").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.gutter_menu = None;
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.clone()));
                            cx.notify();
                        }),
                    ))
                }
                SpringItem::ShowEndpoint(path) => col.child(
                    row("spring-show").child(icon(Icon::Endpoint, 13., green)).child("Show in Endpoints").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.gutter_menu = None;
                            if let Some(model) = spring::spring(cx) {
                                let path = path.clone();
                                model.update(cx, |s, cx| s.show_endpoint(path, cx));
                            }
                            cx.notify();
                        }),
                    ),
                ),
            };
        }
        col.into_any_element()
    }
}

/// The file and line of a place, "GreetingController.java:33".
fn place(target: &Target) -> String {
    match target.file() {
        Some((path, line)) => {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            format!("{name}:{}", line + 1)
        }
        None => "library".to_owned(),
    }
}

/// What a Spring mark's menu lists for a line.
pub fn spring_items(line: &SpringLine) -> Vec<SpringItem> {
    let mut out = Vec::new();
    for e in &line.endpoints {
        let paths = if e.paths.is_empty() { vec![e.path.clone()] } else { e.paths.clone() };
        for path in &paths {
            out.push(SpringItem::Heading(format!("{} {path}", e.methods_label())));
        }
        out.push(SpringItem::CopyPath(e.path.clone()));
        out.push(SpringItem::ShowEndpoint(e.path.clone()));
    }
    for b in &line.beans {
        let sites = b.sites();
        if sites.is_empty() {
            out.push(SpringItem::Heading(format!("Bean '{}'", b.name)));
            out.push(SpringItem::Note("Not injected anywhere".into()));
            continue;
        }
        out.push(SpringItem::Heading(format!("Bean '{}' is injected into", b.name)));
        for s in sites {
            let target = s.target();
            out.push(SpringItem::Go { label: s.label(), detail: place(&target), target });
        }
    }
    for i in &line.injections {
        let name = i.name.clone().or(i.member.clone()).unwrap_or_default();
        match (i.status.as_str(), i.beans.as_slice()) {
            ("value", _) => {
                out.push(SpringItem::Note(format!("'{name}' takes {}", i.value.clone().unwrap_or_default())));
            }
            (_, []) => {
                let why = i.message.clone().unwrap_or_else(|| format!("No bean for '{name}'"));
                out.push(SpringItem::Note(format!("{why}. Press ⌘. for quick fixes.")));
            }
            (status, beans) => {
                let heading = if status == "ambiguous" {
                    format!("'{name}' could take any of")
                } else if beans.len() > 1 {
                    format!("'{name}' takes")
                } else {
                    format!("'{name}' takes the bean")
                };
                out.push(SpringItem::Heading(heading));
                for b in beans {
                    let target = b.target();
                    if target.can_open() {
                        out.push(SpringItem::Go { label: b.label(), detail: place(&target), target });
                    }
                }
            }
        }
    }
    out
}

/// The Spring marks of a file, for tests.
pub fn spring_of(e: &EditorPanel) -> Option<Rc<FileSpring>> {
    e.spring.clone()
}

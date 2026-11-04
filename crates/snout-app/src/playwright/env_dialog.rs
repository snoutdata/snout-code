//! Edit Environments: where a Playwright project's tests point. Each environment has a name,
//! the application's URL (`BASE_URL`), other variables, and a `.env` file of the project
//! to read first. Kept with the project's settings, outside the repository, so each tester
//! keeps their own; secrets belong in the `.env` file, which the repository ignores.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement as _;

use crate::glyphs::{Icon, icon};
use crate::look;
use crate::run::Runs;
use crate::run::config::{self, Environment};
use crate::theme::palette;

pub enum EnvEvent {
    Close,
}

struct Fields {
    name: Entity<InputState>,
    base_url: Entity<InputState>,
    vars: Entity<InputState>,
    env_file: Entity<InputState>,
}

pub struct EnvDialog {
    runs: Entity<Runs>,
    pub list: Vec<Environment>,
    pub selected: Option<usize>,
    /// The one runs go against, by name.
    pub current: Option<String>,
    fields: Fields,
    focus: FocusHandle,
}

impl EventEmitter<EnvEvent> for EnvDialog {}

impl Focusable for EnvDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EnvDialog {
    pub fn new(runs: Entity<Runs>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let r = runs.read(cx);
        let list = r.project.environments.clone();
        let current = r.project.environment.clone();
        let selected = current.as_ref().and_then(|n| list.iter().position(|e| &e.name == n)).or(if list.is_empty() {
            None
        } else {
            Some(0)
        });
        let field = |hint: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        let fields = Fields {
            name: field("Staging", window, cx),
            base_url: field("https://staging.example.com", window, cx),
            vars: field("USER_EMAIL=qa@example.com; LOCALE=en", window, cx),
            env_file: field(".env.staging", window, cx),
        };
        let mut this = Self { runs, list, selected, current, fields, focus: cx.focus_handle() };
        this.load_fields(window, cx);
        this
    }

    fn load_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let e = self.selected.and_then(|i| self.list.get(i)).cloned().unwrap_or_default();
        let set = |state: &Entity<InputState>, text: String, window: &mut Window, cx: &mut Context<Self>| {
            state.update(cx, |s, cx| s.set_value(text, window, cx));
        };
        set(&self.fields.name, e.name.clone(), window, cx);
        set(&self.fields.base_url, e.base_url.clone(), window, cx);
        set(&self.fields.vars, config::env_text(&e.vars), window, cx);
        set(&self.fields.env_file, e.env_file.clone().unwrap_or_default(), window, cx);
    }

    fn store_fields(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.selected else { return };
        let read = |s: &Entity<InputState>| s.read(cx).value().to_string();
        let (name, url, vars, file) = (
            read(&self.fields.name),
            read(&self.fields.base_url),
            read(&self.fields.vars),
            read(&self.fields.env_file),
        );
        let Some(e) = self.list.get_mut(i) else { return };
        let old = e.name.clone();
        e.name = name.trim().to_owned();
        e.base_url = url.trim().to_owned();
        e.vars = config::parse_env(&vars);
        e.env_file = Some(file.trim().to_owned()).filter(|f| !f.is_empty());
        if self.current.as_deref() == Some(old.as_str()) {
            self.current = Some(e.name.clone());
        }
    }

    fn pick(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.store_fields(cx);
        self.selected = Some(ix);
        self.load_fields(window, cx);
        cx.notify();
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.store_fields(cx);
        let names = ["Local", "Staging", "Production"];
        let name = names
            .iter()
            .find(|n| !self.list.iter().any(|e| e.name == **n))
            .map_or_else(|| format!("Environment {}", self.list.len() + 1), |n| (*n).to_owned());
        let base_url = if name == "Local" { "http://localhost:8080".to_owned() } else { String::new() };
        self.list.push(Environment { name, base_url, ..Default::default() });
        self.selected = Some(self.list.len() - 1);
        self.load_fields(window, cx);
        cx.notify();
    }

    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.selected else { return };
        let gone = self.list.remove(i);
        if self.current.as_deref() == Some(gone.name.as_str()) {
            self.current = None;
        }
        self.selected = if self.list.is_empty() { None } else { Some(i.min(self.list.len() - 1)) };
        self.load_fields(window, cx);
        cx.notify();
    }

    /// Keep the environments; the one picked in the list becomes the one runs go against.
    pub fn apply(&mut self, cx: &mut Context<Self>) {
        self.store_fields(cx);
        self.list.retain(|e| !e.name.is_empty());
        let picked = self.selected.and_then(|i| self.list.get(i)).map(|e| e.name.clone()).or(self.current.clone());
        let list = self.list.clone();
        self.runs.update(cx, |r, cx| r.set_environments(list, picked, cx));
    }

    fn form(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        if self.selected.is_none() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(p.fg_dim)
                .child("No environment. Add one with +.")
                .into_any_element();
        }
        let label = |text: &'static str| div().flex_none().w(px(150.)).text_color(p.fg_dim).child(text);
        let row = |text: &'static str, field: AnyElement| {
            div().flex().items_center().gap(px(10.)).child(label(text)).child(div().flex_1().min_w_0().child(field))
        };
        let input = |s: &Entity<InputState>| look::field(s, false, None, &p).into_any_element();
        div()
            .id("env-form")
            .size_full()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .overflow_y_scrollbar()
            .child(row("Name", input(&self.fields.name)))
            .child(row("Application URL", input(&self.fields.base_url)))
            .child(row("Variables", input(&self.fields.vars)))
            .child(row(".env file", input(&self.fields.env_file)))
            .child(div().pt(px(6.)).text_size(px(12.)).text_color(p.fg_dim).child(
                "Runs get the URL as BASE_URL, which the configuration's baseURL reads. The file's variables come \
                 first, then those above. Keep passwords in the .env file, which the repository should ignore.",
            ))
            .into_any_element()
    }
}

impl Render for EnvDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let items: Vec<AnyElement> = self
            .list
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let on = Some(i) == self.selected;
                let current = self.current.as_deref() == Some(e.name.as_str());
                crate::components::list_row_h(
                    ("env-item", i),
                    crate::components::RowState::current(on),
                    crate::components::TREE_ROW_H,
                    &p,
                )
                .debug_selector(move || format!("env-item-{i}"))
                .mx(px(4.))
                .w_auto()
                .px(px(8.))
                .gap(px(6.))
                .rounded(px(4.))
                .cursor_pointer()
                .child(icon(Icon::Endpoint, 12., p.accent))
                .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(e.name.clone()))
                .when(current, |d| d.child(div().text_size(px(11.)).text_color(p.fg_dim).child("in use")))
                .on_click(cx.listener(move |this, _, w, cx| this.pick(i, w, cx)))
                .into_any_element()
            })
            .collect();
        let button = |id: &'static str, ico: crate::components::IconName, text: &'static str, primary: bool| {
            crate::components::footer_button(id, ico, text, primary, true, &p)
        };
        let small = |id: &'static str, g: Icon, tip: &'static str| crate::tabs::tool_button(id, g, tip, &p);
        div()
            .id("env-dialog")
            .debug_selector(|| "env-dialog".into())
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(EnvEvent::Close)))
            .occlude()
            .w(px(760.))
            .h(px(420.))
            .flex()
            .flex_col()
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(12.))
            .shadow_lg()
            .overflow_hidden()
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(
                crate::components::dialog_header_icon(
                    crate::components::Tone::Success,
                    crate::components::IconName::Globe,
                    "Environments",
                    Some("Where the tests point: the application's URL and the variables they read.".into()),
                    &p,
                )
                .pb(px(12.))
                .border_b_1()
                .border_color(p.border),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .flex_none()
                            .w(px(220.))
                            .h_full()
                            .flex()
                            .flex_col()
                            .bg(p.ground)
                            .border_r_1()
                            .border_color(p.border)
                            .child(
                                div()
                                    .flex_none()
                                    .h(px(32.))
                                    .px(px(6.))
                                    .flex()
                                    .items_center()
                                    .gap(px(2.))
                                    .child(
                                        small("env-add", Icon::Plus, "Add Environment")
                                            .on_click(cx.listener(|this, _, w, cx| this.add(w, cx))),
                                    )
                                    .child(
                                        small("env-remove", Icon::Minus, "Remove Environment")
                                            .on_click(cx.listener(|this, _, w, cx| this.remove(w, cx))),
                                    ),
                            )
                            .child(
                                div()
                                    .id("env-list")
                                    .flex_1()
                                    .min_h_0()
                                    .pb(px(6.))
                                    .overflow_y_scrollbar()
                                    .children(items),
                            ),
                    )
                    .child(div().flex_1().min_w_0().h_full().child(self.form(cx))),
            )
            .child(
                div()
                    .flex_none()
                    .min_h(px(56.))
                    .px(px(20.))
                    .py(px(12.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(p.border)
                    .bg(p.dialog_footer())
                    .child(
                        button("env-cancel", crate::components::CANCEL_ICON, "Cancel", false)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(EnvEvent::Close))),
                    )
                    .child(button("env-ok", crate::components::OK_ICON, "OK", true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.apply(cx);
                            cx.emit(EnvEvent::Close);
                        },
                    ))),
            )
    }
}

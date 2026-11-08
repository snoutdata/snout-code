//! Edit Configurations: the project's run configurations on the left, the one picked on the
//! right: its name, kind, module, main class or task, program arguments, VM options,
//! environment variables and active profiles. A configuration the build provides keeps its
//! id when changed, so the change stands in for it; the user's own can be added and
//! removed. Saved with the project's settings, outside the repository.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::input::InputState;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;

use super::Runs;
use super::config::{self, Kind, RunConfig};
use crate::glyphs::{Icon, icon};
use crate::look;
use crate::theme::palette;

pub enum ConfigEvent {
    Close,
}

/// The profiles of an Active profiles field: `dev, local`.
pub fn split_profiles(text: &str) -> Vec<String> {
    text.split(',').map(|p| p.trim().to_owned()).filter(|p| !p.is_empty()).collect()
}

/// The field's text with `profile` added, or taken out when it is there.
pub fn toggled_profiles(text: &str, profile: &str) -> String {
    let mut list = split_profiles(text);
    match list.iter().position(|p| p == profile) {
        Some(i) => {
            list.remove(i);
        }
        None => list.push(profile.to_owned()),
    }
    list.join(", ")
}

struct Fields {
    name: Entity<InputState>,
    main_class: Entity<InputState>,
    task: Entity<InputState>,
    args: Entity<InputState>,
    jvm_args: Entity<InputState>,
    env: Entity<InputState>,
    profiles: Entity<InputState>,
    port: Entity<InputState>,
}

pub struct ConfigDialog {
    runs: Entity<Runs>,
    /// The configurations being edited, and the build's as they came.
    pub list: Vec<RunConfig>,
    engine: Vec<RunConfig>,
    pub selected: Option<usize>,
    fields: Fields,
    modules: Vec<String>,
    next: usize,
    focus: FocusHandle,
}

impl EventEmitter<ConfigEvent> for ConfigDialog {}

impl Focusable for ConfigDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ConfigDialog {
    pub fn new(runs: Entity<Runs>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let r = runs.read(cx);
        let list = r.configurations();
        let engine = r.found_configs();
        let selected = r
            .project
            .selected
            .as_ref()
            .and_then(|id| list.iter().position(|c| &c.id == id))
            .or(if list.is_empty() { None } else { Some(0) });
        let mut modules: Vec<String> = crate::project_model::store(cx)
            .and_then(|s| s.read(cx).current.clone())
            .map(|m| m.modules.iter().map(|m| m.id.clone()).collect())
            .unwrap_or_default();
        for c in &list {
            if !c.module.is_empty() && !modules.contains(&c.module) {
                modules.push(c.module.clone());
            }
        }
        let field = |hint: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        let fields = Fields {
            name: field(ts!("dialogs-name"), window, cx),
            main_class: field("com.example.App", window, cx),
            task: field(":app:bootRun", window, cx),
            args: field(ts!("config-arguments-the-program-gets"), window, cx),
            jvm_args: field("-Xmx1g -Dkey=value", window, cx),
            env: field("NAME=value; OTHER=value", window, cx),
            profiles: field("dev, local", window, cx),
            port: field(ts!("config-9229-empty-run-the-file"), window, cx),
        };
        let next = list.len() + 1;
        let mut this = Self { runs, list, engine, selected, fields, modules, next, focus: cx.focus_handle() };
        this.load_fields(window, cx);
        this
    }

    /// Fill the fields from the selected configuration.
    fn load_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let c = self.selected.and_then(|i| self.list.get(i)).cloned().unwrap_or_default();
        let set = |state: &Entity<InputState>, text: String, window: &mut Window, cx: &mut Context<Self>| {
            state.update(cx, |s, cx| s.set_value(text, window, cx));
        };
        set(&self.fields.name, c.name.clone(), window, cx);
        set(&self.fields.main_class, c.main_class.clone().unwrap_or_default(), window, cx);
        set(&self.fields.task, c.task.clone().unwrap_or_default(), window, cx);
        set(&self.fields.args, config::join_args(&c.args), window, cx);
        set(&self.fields.jvm_args, config::join_args(&c.jvm_args), window, cx);
        set(&self.fields.env, config::env_text(&c.env), window, cx);
        set(&self.fields.profiles, c.profiles.clone(), window, cx);
        set(&self.fields.port, c.port.map(|p| p.to_string()).unwrap_or_default(), window, cx);
    }

    /// Take what the fields say into the selected configuration.
    fn store_fields(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.selected else { return };
        let read = |s: &Entity<InputState>| s.read(cx).value().to_string();
        let (name, main, task) = (read(&self.fields.name), read(&self.fields.main_class), read(&self.fields.task));
        let (args, jvm, env, profiles) =
            (read(&self.fields.args), read(&self.fields.jvm_args), read(&self.fields.env), read(&self.fields.profiles));
        let Some(c) = self.list.get_mut(i) else { return };
        let mut next = c.clone();
        next.name = name.trim().to_owned();
        next.main_class = Some(main.trim().to_owned()).filter(|s| !s.is_empty());
        next.task = Some(task.trim().to_owned()).filter(|s| !s.is_empty());
        next.args = config::split_args(&args);
        next.jvm_args = config::split_args(&jvm);
        next.env = config::parse_env(&env);
        next.profiles = profiles.trim().to_owned();
        next.port = read(&self.fields.port).trim().parse().ok();
        if next != *c {
            next.user = true;
            *c = next;
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
        let module = self.modules.first().cloned().unwrap_or_default();
        self.list.push(RunConfig {
            id: format!("user:{}:{}", std::process::id(), self.next),
            name: t!("config-unnamed", next = (self.next).to_string()),
            kind: Kind::Main,
            module,
            user: true,
            ..Default::default()
        });
        self.next += 1;
        self.selected = Some(self.list.len() - 1);
        self.load_fields(window, cx);
        cx.notify();
    }

    /// Remove the user's configuration, or put the build's back as the build has it.
    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.selected else { return };
        let id = self.list[i].id.clone();
        match self.engine.iter().find(|c| c.id == id) {
            Some(original) => self.list[i] = original.clone(),
            None => {
                self.list.remove(i);
                self.selected = if self.list.is_empty() { None } else { Some(i.min(self.list.len() - 1)) };
            }
        }
        self.load_fields(window, cx);
        cx.notify();
    }

    fn set_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        self.store_fields(cx);
        if let Some(c) = self.selected.and_then(|i| self.list.get_mut(i))
            && c.kind != kind
        {
            c.kind = kind;
            c.user = true;
        }
        cx.notify();
    }

    fn set_module(&mut self, module: String, cx: &mut Context<Self>) {
        self.store_fields(cx);
        if let Some(c) = self.selected.and_then(|i| self.list.get_mut(i))
            && c.module != module
        {
            c.module = module;
            c.user = true;
        }
        cx.notify();
    }

    /// Save the configurations; the one selected becomes the title bar's.
    pub fn apply(&mut self, cx: &mut Context<Self>) {
        self.store_fields(cx);
        let user: Vec<RunConfig> = self.list.iter().filter(|c| c.user).cloned().collect();
        let selected = self.selected.and_then(|i| self.list.get(i)).map(|c| c.id.clone());
        self.runs.update(cx, |r, cx| r.set_user_configurations(user, selected, cx));
    }

    /// Toggle `profile` in the Active profiles field.
    fn toggle_profile(&mut self, profile: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.fields.profiles.read(cx).value().to_string();
        let next = toggled_profiles(&text, profile);
        self.fields.profiles.update(cx, |s, cx| s.set_value(next, window, cx));
        self.store_fields(cx);
        cx.notify();
    }

    /// The Active profiles field, and a menu of the profiles the Spring model knows.
    fn profiles_field(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let known: Vec<(String, String)> = crate::spring::spring(cx)
            .map(|s| s.read(cx).profile_info.known().into_iter().map(|k| (k.name, k.sources.join("; "))).collect())
            .unwrap_or_default();
        let current = self.fields.profiles.read(cx).value().to_string();
        let this = cx.entity();
        let field = look::field(&self.fields.profiles, false, None, &p);
        if known.is_empty() {
            return field.into_any_element();
        }
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().flex_1().min_w_0().child(field))
            .child(
                Button::new("config-profiles")
                    .debug_selector(|| "config-profiles".into())
                    .outline()
                    .icon(crate::components::menu_icon(crate::components::IconName::Leaf))
                    .label(ts!("settings-profiles"))
                    .dropdown_menu(move |mut menu, _, _| {
                        for (name, from) in &known {
                            let this = this.clone();
                            let n = name.clone();
                            let on = split_profiles(&current).iter().any(|c| c == name);
                            menu = menu.item(
                                PopupMenuItem::new(format!("{name}    {from}"))
                                    .checked(on)
                                    .on_click(move |_, w, cx| this.update(cx, |d, cx| d.toggle_profile(&n, w, cx))),
                            );
                        }
                        menu
                    }),
            )
            .into_any_element()
    }

    fn form(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let Some(c) = self.selected.and_then(|i| self.list.get(i)).cloned() else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(p.fg_dim)
                .child(ts!("config-no-configuration-add-one-with"))
                .into_any_element();
        };
        let label = |text: &'static str| div().flex_none().w(px(150.)).text_color(p.fg_dim).child(text);
        let row = |text: &'static str, field: AnyElement| {
            div().flex().items_center().gap(px(10.)).child(label(text)).child(div().flex_1().min_w_0().child(field))
        };
        let input = |s: &Entity<InputState>| look::field(s, false, None, &p).into_any_element();
        let this = cx.entity();
        let kind_menu = {
            let this = this.clone();
            let current = c.kind;
            Button::new("config-kind")
                .debug_selector(|| "config-kind".into())
                .outline()
                .icon(crate::components::menu_icon(crate::components::IconName::Shapes))
                .label(current.label())
                .dropdown_menu(move |mut menu, _, _| {
                    for k in Kind::ALL {
                        let this = this.clone();
                        menu = menu.item(
                            PopupMenuItem::new(k.label())
                                .checked(k == current)
                                .on_click(move |_, _, cx| this.update(cx, |d, cx| d.set_kind(k, cx))),
                        );
                    }
                    menu
                })
                .into_any_element()
        };
        let module_menu = {
            let this = this.clone();
            let modules = self.modules.clone();
            let current = c.module.clone();
            Button::new("config-module")
                .debug_selector(|| "config-module".into())
                .outline()
                .icon(crate::components::menu_icon(crate::components::IconName::Package))
                .label(if current.is_empty() { ts!("config-choose-a-module").to_owned() } else { current.clone() })
                .dropdown_menu(move |mut menu, _, _| {
                    for m in &modules {
                        let this = this.clone();
                        let m2 = m.clone();
                        menu = menu.item(
                            PopupMenuItem::new(m.clone())
                                .checked(*m == current)
                                .on_click(move |_, _, cx| this.update(cx, |d, cx| d.set_module(m2.clone(), cx))),
                        );
                    }
                    menu
                })
                .into_any_element()
        };
        let from_build = self.engine.iter().any(|e| e.id == c.id);
        let pw = c.kind == Kind::Playwright;
        // Node's kinds have no module, VM or profiles.
        let node = c.kind.is_node();
        div()
            .id("config-form")
            .size_full()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .overflow_y_scrollbar()
            .child(row(ts!("dialogs-name"), input(&self.fields.name)))
            .child(row(ts!("config-kind"), div().flex().child(kind_menu).into_any_element()))
            .when(!node, |d| d.child(row(ts!("find-module"), div().flex().child(module_menu).into_any_element())))
            .when(c.kind == Kind::Main, |d| d.child(row(ts!("config-main-class"), input(&self.fields.main_class))))
            .when(c.kind != Kind::Main && !pw, |d| {
                d.child(row(
                    match c.kind {
                        Kind::Maven => ts!("config-goals"),
                        Kind::Npm => ts!("config-script"),
                        Kind::Node => ts!("config-file"),
                        _ => ts!("config-task"),
                    },
                    input(&self.fields.task),
                ))
            })
            .child(row(
                match c.kind {
                    Kind::Gradle => ts!("config-gradle-arguments"),
                    Kind::Maven => ts!("config-maven-arguments"),
                    Kind::Playwright => ts!("config-playwright-arguments"),
                    Kind::Npm => ts!("config-script-arguments"),
                    _ => ts!("config-program-arguments"),
                },
                input(&self.fields.args),
            ))
            .when(!node || c.kind == Kind::Node, |d| {
                d.child(row(
                    if c.kind == Kind::Node { ts!("config-node-options") } else { ts!("config-vm-options") },
                    input(&self.fields.jvm_args),
                ))
            })
            .child(row(ts!("config-environment-variables"), input(&self.fields.env)))
            .when(c.kind == Kind::Node, |d| d.child(row(ts!("config-attach-to-port"), input(&self.fields.port))))
            .when(!node, |d| d.child(row(ts!("settings-active-profiles"), self.profiles_field(cx))))
            .child(div().pt(px(6.)).text_size(px(12.)).text_color(p.fg_dim).child(if from_build {
                if c.user {
                    ts!("config-from-the-build-with-your-changes-remove-puts")
                } else {
                    ts!("config-from-the-build-a-change-you-make-stands")
                }
            } else {
                ts!("config-your-own-configuration")
            }))
            .into_any_element()
    }
}

impl Render for ConfigDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let mut items: Vec<AnyElement> = Vec::new();
        let mut last_group = "";
        for (i, c) in self.list.iter().enumerate() {
            let group =
                if c.user && !self.engine.iter().any(|e| e.id == c.id) { ts!("title-yours") } else { c.group() };
            if group != last_group {
                last_group = group;
                items.push(
                    div()
                        .px(px(10.))
                        .pt(px(8.))
                        .pb(px(2.))
                        .text_size(px(11.))
                        .text_color(p.fg_dim)
                        .child(group)
                        .into_any_element(),
                );
            }
            let on = Some(i) == self.selected;
            items.push(
                crate::components::list_row_h(
                    ("config-item", i),
                    crate::components::RowState::current(on),
                    crate::components::TREE_ROW_H,
                    &p,
                )
                .debug_selector(move || format!("config-item-{i}"))
                .mx(px(4.))
                .w_auto()
                .px(px(8.))
                .gap(px(6.))
                .rounded(px(4.))
                .cursor_pointer()
                .child(icon(
                    match c.kind {
                        Kind::Main => Icon::Play,
                        Kind::Gradle => Icon::Build,
                        Kind::Maven => Icon::Feather,
                        Kind::Tests => Icon::Check,
                        Kind::Playwright => Icon::TestFolder,
                        Kind::Npm => Icon::Package,
                        Kind::Node => Icon::Code,
                    },
                    12.,
                    p.ok,
                ))
                .child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(c.name.clone()))
                .when(c.user, |d| d.child(div().text_color(p.accent).child("•")))
                .on_click(cx.listener(move |this, _, w, cx| this.pick(i, w, cx)))
                .into_any_element(),
            );
        }
        let button = |id: &'static str, ico: crate::components::IconName, text: &'static str, primary: bool| {
            crate::components::footer_button(id, ico, text, primary, true, &p)
        };
        let small = |id: &'static str, g: Icon, tip: &'static str| crate::tabs::tool_button(id, g, tip, &p);
        div()
            .id("config-dialog")
            .debug_selector(|| "config-dialog".into())
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(ConfigEvent::Close)))
            .occlude()
            .w(px(860.))
            .h(px(540.))
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
                    crate::components::IconName::Play,
                    ts!("config-run-configurations"),
                    Some(ts!("config-what-run-and-debug-start-an-application-a").into()),
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
                            .w(px(260.))
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
                                        small("config-add", Icon::Plus, ts!("config-add-configuration"))
                                            .on_click(cx.listener(|this, _, w, cx| this.add(w, cx))),
                                    )
                                    .child(
                                        small(
                                            "config-remove",
                                            Icon::Minus,
                                            ts!("config-remove-or-put-back-as-the-build-has"),
                                        )
                                        .on_click(cx.listener(|this, _, w, cx| this.remove(w, cx))),
                                    ),
                            )
                            .child(
                                div()
                                    .id("config-list")
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
                        button("config-cancel", crate::components::CANCEL_ICON, ts!("settings-cancel"), false)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(ConfigEvent::Close))),
                    )
                    .child(
                        button("config-apply", crate::components::APPLY_ICON, ts!("settings-apply"), false).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.apply(cx);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(button("config-ok", crate::components::OK_ICON, "OK", true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.apply(cx);
                            cx.emit(ConfigEvent::Close);
                        },
                    ))),
            )
    }
}

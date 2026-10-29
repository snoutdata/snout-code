//! The Spring tool window, on the right stripe, with two tabs. Beans: each module's beans,
//! grouped by package or by stereotype, with a search and filters for the stereotype, the
//! profile and the source language. Endpoints: every request mapping as a table of
//! method, path, handler and module, with a search, an HTTP method filter and sorting by
//! any column. A click goes to the bean or the handler.

use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px, relative, uniform_list,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::tooltip::Tooltip;

use super::{Bean, Endpoint, Load, ModuleBeans, Spring, green, simple};
use crate::components::RowState;
use crate::glyphs::{Icon, icon};
use crate::theme::{mix, palette};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Beans,
    Endpoints,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    Package,
    Stereotype,
}

/// Which beans the profile filter keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileFilter {
    Any,
    /// Beans with no `@Profile`.
    Unprofiled,
    Named(String),
}

/// Which beans the source filter keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    All,
    Java,
    Scala,
    Xml,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::All => "All sources",
            Source::Java => "Java",
            Source::Scala => "Scala",
            Source::Xml => "XML",
        }
    }

    fn keeps(self, language: &str) -> bool {
        match self {
            Source::All => true,
            Source::Java => language == "java",
            Source::Scala => language == "scala",
            Source::Xml => language == "xml",
        }
    }
}

/// The columns of the Endpoints table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Method,
    Path,
    Handler,
    Module,
}

/// What the Beans tab keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct BeanFilter {
    pub query: String,
    pub stereotype: Option<String>,
    pub profile: ProfileFilter,
    pub source: Source,
}

impl Default for BeanFilter {
    fn default() -> Self {
        Self { query: String::new(), stereotype: None, profile: ProfileFilter::Any, source: Source::All }
    }
}

impl BeanFilter {
    pub fn keeps(&self, b: &Bean) -> bool {
        if self.stereotype.as_ref().is_some_and(|s| *s != b.stereotype_label()) {
            return false;
        }
        match &self.profile {
            ProfileFilter::Any => {}
            ProfileFilter::Unprofiled if !b.profiles.is_empty() => return false,
            ProfileFilter::Named(p) if !b.profiles.iter().any(|x| x.trim_start_matches('!') == p) => return false,
            _ => {}
        }
        if !self.source.keeps(&b.language) {
            return false;
        }
        matches_query(&self.query, &[&b.name, &b.ty, b.declaring_type.as_deref().unwrap_or_default()])
    }
}

/// Every word of `query` is in one of `fields`, ignoring case.
fn matches_query(query: &str, fields: &[&str]) -> bool {
    let fields: Vec<String> = fields.iter().map(|f| f.to_lowercase()).collect();
    query.split_whitespace().all(|w| {
        let w = w.to_lowercase();
        fields.iter().any(|f| f.contains(&w))
    })
}

/// What the Endpoints tab keeps, and its order.
#[derive(Clone, Debug, PartialEq)]
pub struct EndpointFilter {
    pub query: String,
    pub method: Option<String>,
    pub sort: Column,
    pub ascending: bool,
}

impl Default for EndpointFilter {
    fn default() -> Self {
        Self { query: String::new(), method: None, sort: Column::Path, ascending: true }
    }
}

/// The endpoints the filter keeps, in its order.
pub fn endpoint_rows(all: &[Endpoint], f: &EndpointFilter) -> Vec<Endpoint> {
    let mut out: Vec<Endpoint> = all
        .iter()
        .filter(|e| f.method.as_ref().is_none_or(|m| e.methods.is_empty() || e.methods.contains(m)))
        .filter(|e| {
            let handler = e.handler_label();
            let paths = e.paths.join(" ");
            matches_query(&f.query, &[&e.path, &paths, &handler, &e.handler.class, &e.module, &e.methods_label()])
        })
        .cloned()
        .collect();
    let key = |e: &Endpoint| -> String {
        match f.sort {
            Column::Method => e.methods_label(),
            Column::Path => e.path.clone(),
            Column::Handler => e.handler_label(),
            Column::Module => e.module.clone(),
        }
    };
    out.sort_by(|a, b| {
        let o = key(a).cmp(&key(b)).then_with(|| a.path.cmp(&b.path));
        if f.ascending { o } else { o.reverse() }
    });
    out
}

/// A row of the Beans tab.
#[derive(Clone, Debug, PartialEq)]
pub enum BeanRow {
    Module { key: String, name: String, count: usize, library: usize, open: bool },
    Group { key: String, name: String, count: usize, open: bool },
    Bean(Box<Bean>),
}

/// The Beans tab's rows: modules, their groups, the beans of the open ones. While a search
/// or a filter is on, every group is open.
pub fn bean_rows(
    modules: &[ModuleBeans],
    f: &BeanFilter,
    grouping: Grouping,
    open: &HashSet<String>,
    closed: &HashSet<String>,
) -> Vec<BeanRow> {
    let filtering = *f != BeanFilter::default();
    let mut out = Vec::new();
    for m in modules {
        let kept: Vec<&Bean> = m.beans.iter().filter(|b| f.keeps(b)).collect();
        if kept.is_empty() && filtering {
            continue;
        }
        let mkey = format!("m:{}", m.module);
        let mopen = !closed.contains(&mkey);
        out.push(BeanRow::Module {
            key: mkey.clone(),
            name: m.module.clone(),
            count: kept.len(),
            library: m.library_beans,
            open: mopen,
        });
        if !mopen {
            continue;
        }
        let mut groups: BTreeMap<String, Vec<&Bean>> = BTreeMap::new();
        for b in kept {
            let g = match grouping {
                Grouping::Package => b.package(),
                Grouping::Stereotype => b.stereotype_label(),
            };
            groups.entry(g).or_default().push(b);
        }
        let single = groups.len() == 1;
        for (name, mut beans) in groups {
            beans.sort_by_key(|b| b.name.to_lowercase());
            let gkey = format!("{mkey}:{name}");
            let gopen = filtering || single || open.contains(&gkey);
            let label = if name.is_empty() { "(default package)".to_owned() } else { name };
            out.push(BeanRow::Group { key: gkey, name: label, count: beans.len(), open: gopen });
            if gopen {
                out.extend(beans.into_iter().map(|b| BeanRow::Bean(Box::new(b.clone()))));
            }
        }
    }
    out
}

/// "1,242".
pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A filter's menu, while it shows.
#[derive(Clone, Debug, PartialEq)]
enum Menu {
    Stereotype(Point<Pixels>),
    Profile(Point<Pixels>),
    Source(Point<Pixels>),
    /// The active profiles the beans are evaluated under.
    Active(Point<Pixels>),
}

pub struct SpringWindow {
    spring: Entity<Spring>,
    pub tab: Tab,
    pub bean_search: Entity<InputState>,
    pub endpoint_search: Entity<InputState>,
    pub beans: BeanFilter,
    pub endpoints: EndpointFilter,
    pub grouping: Grouping,
    open: HashSet<String>,
    closed: HashSet<String>,
    menu: Option<Menu>,
    focus: FocusHandle,
}

impl SpringWindow {
    pub fn new(spring: Entity<Spring>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&spring, |_, _, cx| cx.notify()).detach();
        let bean_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search beans by name or type"));
        let endpoint_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search paths and handlers"));
        cx.subscribe(&bean_search, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                this.beans.query = input.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&endpoint_search, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                this.endpoints.query = input.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();
        Self {
            spring,
            tab: Tab::Beans,
            bean_search,
            endpoint_search,
            beans: BeanFilter::default(),
            endpoints: EndpointFilter::default(),
            grouping: Grouping::Package,
            open: HashSet::new(),
            closed: HashSet::new(),
            menu: None,
            focus: cx.focus_handle(),
        }
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.menu = None;
        cx.notify();
    }

    /// The Endpoints tab, searching for `query`.
    pub fn show_endpoints(&mut self, query: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = Tab::Endpoints;
        if let Some(q) = query {
            self.endpoint_search.update(cx, |s, cx| s.set_value(q.clone(), window, cx));
            self.endpoints.query = q;
        }
        cx.notify();
    }

    /// Fold every module and group shut.
    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.open.clear();
        let modules: Vec<String> = self.spring.read(cx).modules.iter().map(|m| format!("m:{}", m.module)).collect();
        self.closed.extend(modules);
        cx.notify();
    }

    /// The Beans tab's rows now.
    pub fn bean_rows(&self, cx: &App) -> Vec<BeanRow> {
        bean_rows(&self.spring.read(cx).modules, &self.beans, self.grouping, &self.open, &self.closed)
    }

    /// The Endpoints tab's rows now.
    pub fn endpoint_rows(&self, cx: &App) -> Vec<Endpoint> {
        endpoint_rows(&self.spring.read(cx).endpoints, &self.endpoints)
    }

    pub fn toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if key.starts_with("m:") && !key[2..].contains(':') {
            if !self.closed.remove(key) {
                self.closed.insert(key.to_owned());
            }
        } else if !self.open.remove(key) {
            self.open.insert(key.to_owned());
        }
        cx.notify();
    }

    /// Sort the endpoints by `column`, or the other way round when they are already.
    pub fn sort_by(&mut self, column: Column, cx: &mut Context<Self>) {
        if self.endpoints.sort == column {
            self.endpoints.ascending = !self.endpoints.ascending;
        } else {
            self.endpoints.sort = column;
            self.endpoints.ascending = true;
        }
        cx.notify();
    }
}

crate::panels::panel!(SpringWindow, "Spring", |_, _| "Spring".into(), true);

impl SpringWindow {
    /// A tab of the window's own header.
    fn tab_button(&self, tab: Tab, label: String, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let on = self.tab == tab;
        let hover = p.button_hover;
        let id = match tab {
            Tab::Beans => "spring-tab-beans",
            Tab::Endpoints => "spring-tab-endpoints",
        };
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .h(px(24.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(4.))
            .cursor_pointer()
            .whitespace_nowrap()
            .when(on, |d| d.bg(p.button).font_weight(FontWeight::MEDIUM))
            .when(!on, |d| d.text_color(p.fg_dim).hover(move |s| s.bg(hover)))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.set_tab(tab, cx)))
    }

    /// A filter's chip: its value, and a chevron for its menu.
    fn chip(
        &self,
        id: &'static str,
        label: String,
        on: bool,
        menu: fn(Point<Pixels>) -> Menu,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let hover = p.button_hover;
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .flex_none()
            .h(px(22.))
            .pl(px(8.))
            .pr(px(4.))
            .flex()
            .items_center()
            .gap(px(2.))
            .rounded(px(4.))
            .border_1()
            .border_color(if on { mix(p.accent, 0.6, p.ground) } else { mix(p.border, 0.8, p.ground) })
            .when(on, |d| d.bg(p.wash(p.accent)))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .whitespace_nowrap()
            .text_size(px(12.))
            .child(label)
            .child(icon(Icon::ChevronDown, 12., p.fg_dim))
            .on_click(cx.listener(move |this, e: &gpui::ClickEvent, _, cx| {
                this.menu = Some(menu(e.position()));
                cx.notify();
            }))
    }

    fn beans_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let spring = self.spring.read(cx);
        let profiles_exist = !spring.profiles().is_empty();
        let stereo_label = self.beans.stereotype.clone().unwrap_or_else(|| "All stereotypes".into());
        let profile_label = match &self.beans.profile {
            ProfileFilter::Any => "Any profile".to_owned(),
            ProfileFilter::Unprofiled => "No profile".to_owned(),
            ProfileFilter::Named(p) => format!("Profile {p}"),
        };
        let mode = spring.profile_mode.clone();
        let info = spring.profile_info.clone();
        let inactive = info.inactive_beans();
        let switch = div()
            .flex_none()
            .px(px(8.))
            .pb(px(6.))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(icon(Icon::Leaf, 13., green(&p)))
            .child(div().text_size(px(12.)).text_color(p.fg_dim).child("Active profiles"))
            .child(self.chip(
                "spring-active-profiles",
                crate::spring::mode_label(&mode, &info),
                mode != crate::spring::ProfileMode::Any,
                Menu::Active,
                cx,
            ))
            .when(inactive > 0, |d| {
                d.child(
                    div()
                        .debug_selector(|| "spring-inactive-count".into())
                        .text_size(px(11.5))
                        .text_color(p.fg_dim)
                        .child(format!("{} inactive", thousands(inactive))),
                )
            });
        let rows: Rc<Vec<BeanRow>> = Rc::new(self.bean_rows(cx));
        let shown: usize = rows
            .iter()
            .map(|r| match r {
                BeanRow::Module { count, .. } => *count,
                _ => 0,
            })
            .sum();
        let grouping = self.grouping;
        let filters = div()
            .flex_none()
            .px(px(8.))
            .pb(px(6.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .child(self.chip(
                "spring-filter-stereotype",
                stereo_label,
                self.beans.stereotype.is_some(),
                Menu::Stereotype,
                cx,
            ))
            .when(profiles_exist || self.beans.profile != ProfileFilter::Any, |d| {
                d.child(self.chip(
                    "spring-filter-profile",
                    profile_label,
                    self.beans.profile != ProfileFilter::Any,
                    Menu::Profile,
                    cx,
                ))
            })
            .child(self.chip(
                "spring-filter-source",
                self.beans.source.label().to_owned(),
                self.beans.source != Source::All,
                Menu::Source,
                cx,
            ))
            .child(div().flex_1())
            .child(
                div()
                    .id("spring-grouping")
                    .debug_selector(|| "spring-grouping".into())
                    .flex_none()
                    .h(px(22.))
                    .px(px(6.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(p.fg_dim)
                    .hover({
                        let h = p.button_hover;
                        move |s| s.bg(h)
                    })
                    .child(match grouping {
                        Grouping::Package => "By package",
                        Grouping::Stereotype => "By stereotype",
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.grouping = match this.grouping {
                            Grouping::Package => Grouping::Stereotype,
                            Grouping::Stereotype => Grouping::Package,
                        };
                        this.open.clear();
                        cx.notify();
                    })),
            );
        let body: AnyElement = if rows.is_empty() {
            note(if self.spring.read(cx).bean_count() == 0 { "No beans" } else { "No bean matches" }, "", cx)
        } else {
            let this = cx.entity();
            let count = rows.len();
            let leaf = green(&p);
            uniform_list("spring-beans", count, move |range, _, _| {
                range
                    .map(|i| {
                        let row = rows[i].clone();
                        let this = this.clone();
                        let base = crate::components::tree_row(("spring-bean-row", i), RowState::default(), &p)
                            .debug_selector(move || format!("spring-bean-row-{i}"))
                            .overflow_hidden()
                            .gap(px(5.))
                            .cursor_pointer();
                        match row {
                            BeanRow::Module { key, name, count, library, open } => base
                                .pl(px(8.))
                                .child(crate::look::chevron(open, p.fg_dim))
                                .child(icon(Icon::Module, 14., p.accent))
                                .child(div().font_weight(FontWeight::MEDIUM).child(name))
                                .child(div().text_color(p.fg_dim).child(format!(
                                    "{} beans, {} from libraries",
                                    thousands(count),
                                    thousands(library)
                                )))
                                .on_click(move |_, _, cx| this.update(cx, |w, cx| w.toggle(&key, cx))),
                            BeanRow::Group { key, name, count, open } => base
                                .pl(px(24.))
                                .child(crate::look::chevron(open, p.fg_dim))
                                .child(icon(
                                    if grouping == Grouping::Package { Icon::Package } else { Icon::Leaf },
                                    14.,
                                    p.fg_dim,
                                ))
                                .child(div().child(name))
                                .child(div().text_color(p.fg_dim).child(thousands(count)))
                                .on_click(move |_, _, cx| this.update(cx, |w, cx| w.toggle(&key, cx))),
                            BeanRow::Bean(b) => {
                                let target = b.target();
                                let tip: SharedString = format!(
                                    "{} ({})\n{}{}",
                                    b.ty,
                                    b.stereotype_label(),
                                    match b.injected_count() {
                                        0 => "Not injected anywhere".to_owned(),
                                        1 => "Injected in 1 place".to_owned(),
                                        n => format!("Injected in {n} places"),
                                    },
                                    if b.profiles.is_empty() {
                                        String::new()
                                    } else {
                                        format!("\nProfiles: {}", b.profiles.join(", "))
                                    } + if b.active { "" } else { "\nNot active: the active profiles leave it out" }
                                )
                                .into();
                                let off = !b.active;
                                base.pl(px(56.))
                                    .when(off, |d| {
                                        d.opacity(0.5).debug_selector(move || format!("spring-bean-inactive-{i}"))
                                    })
                                    .child(icon(Icon::Leaf, 13., if off { p.fg_dim } else { leaf }))
                                    .child(div().when(off, |d| d.text_color(p.fg_dim)).child(b.name.clone()))
                                    .child(div().text_color(p.fg_dim).child(simple(&b.ty).to_owned()))
                                    .when(b.primary, |d| d.child(crate::look::chip("Primary", p.accent, &p)))
                                    .when(b.language == "scala", |d| d.child(crate::look::chip("Scala", p.warn, &p)))
                                    .when(b.language == "xml", |d| d.child(crate::look::chip("XML", p.fg_dim, &p)))
                                    .children(b.profiles.iter().map(|pr| crate::look::chip(pr.clone(), leaf, &p)))
                                    .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                                    .on_click(move |_, _, cx| {
                                        let spring = this.read(cx).spring.clone();
                                        spring.update(cx, |s, cx| s.open(target.clone(), cx));
                                    })
                            }
                        }
                    })
                    .collect()
            })
            .flex_1()
            .min_h_0()
            .px(px(4.))
            .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_none().px(px(8.)).pb(px(6.)).child(crate::look::field(&self.bean_search, true, None, &p)))
            .child(switch)
            .child(filters)
            .child(
                div()
                    .flex_none()
                    .px(px(10.))
                    .pb(px(4.))
                    .text_size(px(11.5))
                    .text_color(p.fg_dim)
                    .debug_selector(|| "spring-bean-count".into())
                    .child(format!("{} of {} beans", thousands(shown), thousands(self.spring.read(cx).bean_count()))),
            )
            .child(body)
            .into_any_element()
    }

    fn endpoints_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let rows: Rc<Vec<Endpoint>> = Rc::new(self.endpoint_rows(cx));
        let shown = rows.len();
        let total = self.spring.read(cx).endpoints.len();
        let hover = p.button_hover;
        let methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
        let mut method_row = div().flex_none().px(px(8.)).pb(px(6.)).flex().flex_wrap().gap(px(4.));
        for m in std::iter::once("All").chain(methods) {
            let on = match &self.endpoints.method {
                None => m == "All",
                Some(x) => x == m,
            };
            method_row = method_row.child(
                div()
                    .id(SharedString::from(format!("spring-method-{m}")))
                    .debug_selector(move || format!("spring-method-{m}"))
                    .h(px(22.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .when(on, |d| d.bg(p.button).font_weight(FontWeight::MEDIUM))
                    .when(!on, |d| d.text_color(p.fg_dim).hover(move |s| s.bg(hover)))
                    .child(m)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.endpoints.method = (m != "All").then(|| m.to_owned());
                        cx.notify();
                    })),
            );
        }
        let sort = (self.endpoints.sort, self.endpoints.ascending);
        let header_cell = |id: &'static str, col: Column, label: &'static str, cx: &mut Context<Self>| {
            let sorted = (sort.0 == col).then_some(sort.1);
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .flex()
                .items_center()
                .cursor_pointer()
                .child(crate::components::head_title(label, sorted, &p))
                .on_click(cx.listener(move |this, _, _, cx| this.sort_by(col, cx)))
        };
        // The module column only when the endpoints come from more than one.
        let modules = {
            let all = &self.spring.read(cx).endpoints;
            all.first().is_some_and(|f| all.iter().any(|e| e.module != f.module))
        };
        let method_w = px(58.);
        let header = crate::components::header_bar(&p)
            .mx(px(4.))
            .child(header_cell("spring-sort-method", Column::Method, "Method", cx).w(method_w).flex_none())
            .child(header_cell("spring-sort-path", Column::Path, "Path", cx).flex_1().min_w_0())
            .child(header_cell("spring-sort-handler", Column::Handler, "Handler", cx).w(relative(0.3)).flex_none())
            .when(modules, |d| {
                d.child(header_cell("spring-sort-module", Column::Module, "Module", cx).w(px(80.)).flex_none())
            })
            .child(div().w(px(18.)).flex_none());
        let body: AnyElement = if rows.is_empty() {
            note(if total == 0 { "No endpoints" } else { "No endpoint matches" }, "", cx)
        } else {
            let this = cx.entity();
            let count = rows.len();
            let leaf = green(&p);
            uniform_list("spring-endpoints", count, move |range, _, _| {
                range
                    .map(|i| {
                        let e = rows[i].clone();
                        let this = this.clone();
                        let target = e.target();
                        let path = e.path.clone();
                        let tip: SharedString = e.summary().into();
                        let method_color = match e.methods.first().map(String::as_str) {
                            Some("GET") => leaf,
                            Some("POST") => p.accent,
                            Some("DELETE") => p.error,
                            Some("PUT" | "PATCH") => p.warn,
                            _ => p.fg_dim,
                        };
                        crate::components::list_row(("spring-endpoint-row", i), RowState::default(), &p)
                            .debug_selector(move || format!("spring-endpoint-row-{i}"))
                            .overflow_hidden()
                            .gap(px(crate::components::CELL_GAP))
                            .cursor_pointer()
                            .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                            .child(
                                div()
                                    .w(method_w)
                                    .flex_none()
                                    .overflow_hidden()
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(method_color)
                                    .child(match e.methods.as_slice() {
                                        [] => "ANY".to_owned(),
                                        [one] => one.clone(),
                                        [first, rest @ ..] => format!("{first} +{}", rest.len()),
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .font_family(crate::look::MONO)
                                    .text_size(px(12.))
                                    .child(e.path.clone()),
                            )
                            .child(
                                div()
                                    .w(relative(0.3))
                                    .flex_none()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_color(p.fg_dim)
                                    .child(e.handler_label()),
                            )
                            .when(modules, |d| {
                                d.child(
                                    div()
                                        .w(px(80.))
                                        .flex_none()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .text_color(p.fg_dim)
                                        .child(e.module.clone()),
                                )
                            })
                            .child(
                                div()
                                    .id(("spring-copy-path", i))
                                    .debug_selector(move || format!("spring-copy-path-{i}"))
                                    .flex_none()
                                    .size(px(18.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(3.))
                                    .hover(move |s| s.bg(hover))
                                    .tooltip(|w, cx| Tooltip::new("Copy Path").build(w, cx))
                                    .child(icon(Icon::Copy, 12., p.fg_dim))
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.clone()));
                                    }),
                            )
                            .on_click(move |_, _, cx| {
                                let spring = this.read(cx).spring.clone();
                                spring.update(cx, |s, cx| s.open(target.clone(), cx));
                            })
                    })
                    .collect()
            })
            .flex_1()
            .min_h_0()
            .px(px(4.))
            .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_none().px(px(8.)).pb(px(6.)).child(crate::look::field(
                &self.endpoint_search,
                true,
                None,
                &p,
            )))
            .child(method_row)
            .child(
                div()
                    .flex_none()
                    .px(px(10.))
                    .pb(px(4.))
                    .text_size(px(11.5))
                    .text_color(p.fg_dim)
                    .debug_selector(|| "spring-endpoint-count".into())
                    .child(format!("{} of {} endpoints", thousands(shown), thousands(total))),
            )
            .child(header)
            .child(body)
            .into_any_element()
    }

    /// A filter's menu, where its chip was clicked.
    fn menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.clone()?;
        let p = palette(cx);
        let hover = p.button_hover;
        let spring = self.spring.read(cx);
        type Pick = Rc<dyn Fn(&mut SpringWindow, &mut Context<SpringWindow>)>;
        let (at, items): (Point<Pixels>, Vec<(String, bool, Pick)>) = match menu {
            Menu::Stereotype(at) => {
                let mut items: Vec<(String, bool, Pick)> = vec![(
                    "All stereotypes".into(),
                    self.beans.stereotype.is_none(),
                    Rc::new(|w: &mut SpringWindow, _: &mut Context<SpringWindow>| w.beans.stereotype = None),
                )];
                for s in spring.stereotypes() {
                    let on = self.beans.stereotype.as_ref() == Some(&s);
                    let v = s.clone();
                    items.push((
                        s,
                        on,
                        Rc::new(move |w: &mut SpringWindow, _: &mut Context<SpringWindow>| {
                            w.beans.stereotype = Some(v.clone())
                        }),
                    ));
                }
                (at, items)
            }
            Menu::Profile(at) => {
                let mut items: Vec<(String, bool, Pick)> = vec![
                    (
                        "Any profile".into(),
                        self.beans.profile == ProfileFilter::Any,
                        Rc::new(|w: &mut SpringWindow, _: &mut Context<SpringWindow>| {
                            w.beans.profile = ProfileFilter::Any
                        }),
                    ),
                    (
                        "No profile".into(),
                        self.beans.profile == ProfileFilter::Unprofiled,
                        Rc::new(|w: &mut SpringWindow, _: &mut Context<SpringWindow>| {
                            w.beans.profile = ProfileFilter::Unprofiled
                        }),
                    ),
                ];
                for s in spring.profiles() {
                    let on = self.beans.profile == ProfileFilter::Named(s.clone());
                    let v = s.clone();
                    items.push((
                        s,
                        on,
                        Rc::new(move |w: &mut SpringWindow, _: &mut Context<SpringWindow>| {
                            w.beans.profile = ProfileFilter::Named(v.clone())
                        }),
                    ));
                }
                (at, items)
            }
            Menu::Active(at) => {
                use crate::spring::ProfileMode;
                let mode = spring.profile_mode.clone();
                let info = spring.profile_info.clone();
                let set = |m: ProfileMode| -> Pick {
                    Rc::new(move |w: &mut SpringWindow, cx: &mut Context<SpringWindow>| {
                        let m = m.clone();
                        w.spring.update(cx, |s, cx| s.set_profile_mode(m, cx));
                    })
                };
                let files = info.config_active();
                let mut items: Vec<(String, bool, Pick)> = vec![
                    ("Any profile: every bean may be there".into(), mode == ProfileMode::Any, set(ProfileMode::Any)),
                    (
                        format!(
                            "As the application files say ({})",
                            if files.is_empty() { "default".to_owned() } else { files.join(", ") }
                        ),
                        mode == ProfileMode::Config,
                        set(ProfileMode::Config),
                    ),
                ];
                // The selected run configuration's profiles.
                if let Some(c) = crate::run::runs(cx).and_then(|r| r.read(cx).selected_config())
                    && !c.profiles.trim().is_empty()
                {
                    let list: Vec<String> =
                        c.profiles.split(',').map(|p| p.trim().to_owned()).filter(|p| !p.is_empty()).collect();
                    let on = mode == ProfileMode::Chosen(list.clone());
                    items.push((
                        format!("As '{}' runs ({})", c.name, list.join(", ")),
                        on,
                        set(ProfileMode::Chosen(list)),
                    ));
                }
                let chosen = match &mode {
                    ProfileMode::Chosen(p) => p.clone(),
                    ProfileMode::Config => info.active().unwrap_or_default(),
                    ProfileMode::Any => Vec::new(),
                };
                for k in info.known() {
                    let on = chosen.contains(&k.name);
                    let next = mode.toggled(&k.name, &info);
                    items.push((k.name.clone(), on, set(next)));
                }
                (at, items)
            }
            Menu::Source(at) => {
                let items = [Source::All, Source::Java, Source::Scala, Source::Xml]
                    .into_iter()
                    .map(|s| {
                        let pick: Pick =
                            Rc::new(move |w: &mut SpringWindow, _: &mut Context<SpringWindow>| w.beans.source = s);
                        (s.label().to_owned(), self.beans.source == s, pick)
                    })
                    .collect();
                (at, items)
            }
        };
        let list = items.into_iter().enumerate().map(|(i, (label, on, pick))| {
            div()
                .id(("spring-menu-item", i))
                .debug_selector(move || format!("spring-menu-item-{i}"))
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(4.))
                .cursor_pointer()
                .whitespace_nowrap()
                .hover(move |s| s.bg(hover))
                .child(div().w(px(13.)).flex_none().when(on, |d| d.child(icon(Icon::Check, 13., p.accent))))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    pick(this, cx);
                    this.menu = None;
                    cx.notify();
                }))
        });
        Some(
            gpui::deferred(
                gpui::anchored().position(at).snap_to_window_with_margin(px(8.)).child(
                    div()
                        .id("spring-menu")
                        .debug_selector(|| "spring-menu".into())
                        .occlude()
                        .p(px(4.))
                        .max_h(px(420.))
                        .overflow_y_scroll()
                        .rounded(px(6.))
                        .bg(p.card)
                        .border_1()
                        .border_color(mix(p.border, 0.8, p.card))
                        .shadow_lg()
                        .text_size(px(crate::look::BODY))
                        .text_color(p.fg)
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.menu = None;
                            cx.notify();
                        }))
                        .children(list),
                ),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }
}

/// A note in the middle of an empty tab.
fn note(title: &'static str, text: &'static str, cx: &App) -> AnyElement {
    use snout_core::art::spots;
    let art = if title.contains("matches") {
        spots::no_results()
    } else if title.starts_with("Reading") {
        snout_core::art::building()
    } else {
        spots::no_beans()
    };
    let _ = cx;
    div()
        .flex_1()
        .min_h_0()
        .child(crate::components::EmptyState::new(art, title).hint(text).compact())
        .into_any_element()
}

impl Render for SpringWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let spring = self.spring.read(cx);
        let base = div().size_full().flex().flex_col().bg(p.ground).text_size(px(crate::look::BODY)).text_color(p.fg);
        if !spring.loaded {
            let (title, text) = match &spring.load {
                Load::Loading => ("Reading the Spring model", "The beans and endpoints are being read."),
                Load::Failed(_) | Load::Idle => {
                    ("No Spring model yet", "Beans and endpoints appear once the Java engine has imported the project.")
                }
            };
            return base.child(note(title, text, cx));
        }
        let beans = format!("Beans {}", thousands(spring.bean_count()));
        let endpoints = format!("Endpoints {}", thousands(spring.endpoints.len()));
        let loading = spring.load == Load::Loading;
        let tabs = div()
            .flex_none()
            .h(px(32.))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .child(self.tab_button(Tab::Beans, beans, cx))
            .child(self.tab_button(Tab::Endpoints, endpoints, cx))
            .child(div().flex_1())
            .when(loading, |d| d.child(div().text_size(px(11.5)).text_color(p.fg_dim).child("Reading…")));
        let body = match self.tab {
            Tab::Beans => self.beans_tab(cx),
            Tab::Endpoints => self.endpoints_tab(cx),
        };
        base.child(tabs).child(div().flex_1().min_h_0().child(body)).children(self.menu(cx))
    }
}

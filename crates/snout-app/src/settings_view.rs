//! The Settings sheet (the gear, or ⌘,): a searchable tree of pages on the left, the page
//! on the right, and Cancel, Apply and OK at the foot. It edits a copy of the settings;
//! Apply and OK put the copy in place, save it to `settings.json`, and the window follows.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::scroll::ScrollableElement as _;

use crate::glyphs::{Icon, icon};
use crate::lang::java::Jdk;
use crate::look::{self, Trigger};
use crate::model::Model;
use crate::settings::{Settings, ThemeChoice};
use crate::theme::{Palette, mix, palette};

/// A page of the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Appearance,
    Keymap,
    Editor,
    VersionControl,
    Gradle,
    ClaudeCode,
}

impl Page {
    pub const ALL: [Page; 6] =
        [Page::Appearance, Page::Keymap, Page::Editor, Page::VersionControl, Page::Gradle, Page::ClaudeCode];

    pub fn title(self) -> &'static str {
        match self {
            Page::Appearance => "Appearance",
            Page::Keymap => "Keymap",
            Page::Editor => "Editor",
            Page::VersionControl => "Version Control",
            Page::Gradle => "Gradle",
            Page::ClaudeCode => "Claude Code",
        }
    }

    /// The group the page sits under in the tree, if any.
    fn group(self) -> Option<&'static str> {
        match self {
            Page::Gradle => Some("Build Tools"),
            Page::ClaudeCode => Some("Tools"),
            _ => None,
        }
    }

    /// Words the search finds the page by, besides its title.
    fn keywords(self) -> &'static str {
        match self {
            Page::Appearance => "theme dark light system font size interface",
            Page::Keymap => "shortcuts keys bindings",
            Page::Editor => "tab size indent soft wrap whitespace font",
            Page::VersionControl => "git fetch interval background",
            Page::Gradle => "build tools jdk java",
            Page::ClaudeCode => "tools agent console program path",
        }
    }

    /// Whether the search `query` finds the page.
    pub fn matches(self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.title().to_lowercase().contains(&q)
            || self.group().is_some_and(|g| g.to_lowercase().contains(&q))
            || self.keywords().contains(&q)
    }
}

/// The typefaces offered: the bundled ones first, then the system's.
const UI_FONTS: [(&str, &str); 2] = [("Inter", "Inter"), (".SystemUIFont", "System")];
const EDITOR_FONTS: [(&str, &str); 2] = [("Hack", "Hack"), ("Menlo", "Menlo")];

pub enum SettingsEvent {
    /// Put the copy in place (and close when the flag says so).
    Apply(Box<Settings>, bool),
    Cancel,
}

pub struct SettingsView {
    model: Entity<Model>,
    /// The settings as the sheet shows them.
    pub draft: Settings,
    pub page: Page,
    pub search: Entity<InputState>,
    claude: Entity<InputState>,
    /// The JDKs on this Mac, for the Gradle page.
    jdks: Vec<Jdk>,
    /// The Gradle builds the Gradle page lists.
    builds: Vec<PathBuf>,
    /// The keymap: each command and its keys.
    keys: Vec<(SharedString, String)>,
    focus: FocusHandle,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsView {
    pub fn new(
        model: Entity<Model>,
        builds: Vec<PathBuf>,
        jdks: Vec<Jdk>,
        keys: Vec<(SharedString, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = model.read(cx).settings.clone();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
        cx.subscribe(&search, |this, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let query = this.search.read(cx).value().to_string();
                if !this.page.matches(&query)
                    && let Some(first) = Page::ALL.into_iter().find(|p| p.matches(&query))
                {
                    this.page = first;
                }
                cx.notify();
            }
        })
        .detach();
        let path = draft.claude_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let claude = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Found on PATH and the usual places").default_value(path)
        });
        cx.subscribe(&claude, |this, input, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                let text = input.read(cx).value().trim().to_owned();
                this.draft.claude_path = (!text.is_empty()).then(|| PathBuf::from(text));
                cx.notify();
            }
        })
        .detach();
        Self { model, draft, page: Page::Appearance, search, claude, jdks, builds, keys, focus: cx.focus_handle() }
    }

    /// The settings differ from those in use.
    pub fn changed(&self, cx: &App) -> bool {
        self.draft != self.model.read(cx).settings
    }

    pub fn apply(&mut self, close: bool, cx: &mut Context<Self>) {
        // Only what the sheet edits: the rest may have moved on while it was open.
        let mut next = self.model.read(cx).settings.clone();
        let d = &self.draft;
        next.theme = d.theme;
        next.ui_font = d.ui_font.clone();
        next.ui_font_size = d.ui_font_size;
        next.editor_font = d.editor_font.clone();
        next.editor_font_size = d.editor_font_size;
        next.tab_size = d.tab_size;
        next.soft_wrap = d.soft_wrap;
        next.show_whitespace = d.show_whitespace;
        next.fetch_minutes = d.fetch_minutes;
        next.claude_path = d.claude_path.clone();
        next.project_jdks = d.project_jdks.clone();
        self.draft = next.clone();
        cx.emit(SettingsEvent::Apply(Box::new(next), close));
    }

    fn set(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Settings)) {
        f(&mut self.draft);
        cx.notify();
    }
}

/// A labelled row of a page: the label on the left, the control after it.
fn row(label: &'static str, control: impl IntoElement, p: &Palette) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .min_h(px(34.))
        .child(div().w(px(150.)).flex_none().text_color(p.fg).child(label))
        .child(control)
}

fn section(text: &'static str, p: &Palette) -> gpui::Div {
    div().pt(px(14.)).pb(px(6.)).text_color(p.fg_dim).text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(text)
}

/// A − value + stepper.
fn stepper(
    id: &'static str,
    value: String,
    on_step: impl Fn(i32, &mut Window, &mut App) + 'static,
    p: &Palette,
) -> AnyElement {
    let on_step = std::rc::Rc::new(on_step);
    let hover = p.button_hover;
    let button = |suffix: &'static str, sign: i32, text: &'static str| {
        let on_step = on_step.clone();
        div()
            .id(SharedString::from(format!("{id}-{suffix}")))
            .debug_selector(move || format!("{id}-{suffix}"))
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(text)
            .on_click(move |_, w, cx| on_step(sign, w, cx))
    };
    div()
        .flex()
        .items_center()
        .h(px(28.))
        .px(px(2.))
        .rounded(px(5.))
        .border_1()
        .border_color(p.border)
        .bg(p.input)
        .child(button("down", -1, "−"))
        .child(div().min_w(px(44.)).text_center().child(value))
        .child(button("up", 1, "+"))
        .into_any_element()
}

/// A drop-down of `choices` (value, label), the current one checked.
fn choice(
    id: &'static str,
    current: &str,
    choices: Vec<(String, String)>,
    on_pick: impl Fn(String, &mut App) + 'static,
    p: &Palette,
) -> AnyElement {
    let label = choices.iter().find(|(v, _)| v == current).map(|(_, l)| l.clone()).unwrap_or(current.to_owned());
    let on_pick = std::rc::Rc::new(on_pick);
    let current = current.to_owned();
    let hover = p.button_hover;
    Trigger(
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .h(px(28.))
            .min_w(px(180.))
            .px(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .rounded(px(5.))
            .border_1()
            .border_color(p.border)
            .bg(p.input)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(label)
            .child(icon(Icon::ChevronDown, 12., p.fg_dim)),
    )
    .dropdown_menu(move |mut menu, _, _| {
        for (value, label) in &choices {
            let (value, on_pick) = (value.clone(), on_pick.clone());
            menu = menu.item(
                PopupMenuItem::new(label.clone())
                    .checked(value == current)
                    .on_click(move |_, _, cx| on_pick(value.clone(), cx)),
            );
        }
        menu
    })
    .into_any_element()
}

impl SettingsView {
    fn page_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let d = self.draft.clone();
        let view = cx.entity();
        let set = move |f: Box<dyn Fn(&mut Settings)>| {
            let view = view.clone();
            move |cx: &mut App| view.update(cx, |v, cx| v.set(cx, |s| f(s)))
        };
        let body = div().flex().flex_col();
        match self.page {
            Page::Appearance => {
                let theme = |t: ThemeChoice, label: &'static str, id: &'static str, cx: &mut Context<Self>| {
                    look::toggle(id, label, d.theme == t, &p)
                        .debug_selector(move || id.to_owned())
                        .on_click(cx.listener(move |this, _, _, cx| this.set(cx, |s| s.theme = t)))
                };
                let ui_fonts = UI_FONTS.iter().map(|(v, l)| ((*v).to_owned(), (*l).to_owned())).collect();
                let editor_fonts = EDITOR_FONTS.iter().map(|(v, l)| ((*v).to_owned(), (*l).to_owned())).collect();
                let pick_ui = set.clone();
                let pick_editor = set.clone();
                let view = cx.entity();
                let view2 = cx.entity();
                body.child(section("Theme", &p))
                    .child(row(
                        "Theme",
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(theme(ThemeChoice::Light, "Light", "settings-theme-light", cx))
                            .child(theme(ThemeChoice::Dark, "Dark", "settings-theme-dark", cx))
                            .child(theme(ThemeChoice::System, "Match the system", "settings-theme-system", cx)),
                        &p,
                    ))
                    .child(section("Interface", &p))
                    .child(row(
                        "Font",
                        choice(
                            "settings-ui-font",
                            &d.ui_font,
                            ui_fonts,
                            move |v, cx| pick_ui(Box::new(move |s: &mut Settings| s.ui_font = v.clone()))(cx),
                            &p,
                        ),
                        &p,
                    ))
                    .child(row(
                        "Size",
                        stepper(
                            "settings-ui-size",
                            format!("{}", d.ui_font_size),
                            move |by, _, cx| {
                                view.update(cx, |v, cx| {
                                    v.set(cx, |s| s.ui_font_size = (s.ui_font_size + by as f32).clamp(10., 20.))
                                })
                            },
                            &p,
                        ),
                        &p,
                    ))
                    .child(section("Editor", &p))
                    .child(row(
                        "Font",
                        choice(
                            "settings-editor-font",
                            &d.editor_font,
                            editor_fonts,
                            move |v, cx| pick_editor(Box::new(move |s: &mut Settings| s.editor_font = v.clone()))(cx),
                            &p,
                        ),
                        &p,
                    ))
                    .child(row(
                        "Size",
                        stepper(
                            "settings-editor-size",
                            format!("{}", d.editor_font_size),
                            move |by, _, cx| {
                                view2.update(cx, |v, cx| {
                                    v.set(cx, |s| {
                                        s.editor_font_size = (s.editor_font_size + by as f32)
                                            .clamp(crate::editor::FONT_MIN, crate::editor::FONT_MAX)
                                    })
                                })
                            },
                            &p,
                        ),
                        &p,
                    ))
                    .into_any_element()
            }
            Page::Keymap => body
                .child(section("Shortcuts (read only for now)", &p))
                .children(self.keys.iter().map(|(name, keys)| {
                    div()
                        .flex()
                        .items_center()
                        .h(px(26.))
                        .border_b_1()
                        .border_color(mix(p.border, 0.5, p.card))
                        .child(div().flex_1().child(name.clone()))
                        .child(div().text_color(p.fg_dim).font_family(look::mono_font(cx)).child(keys.clone()))
                }))
                .into_any_element(),
            Page::Editor => {
                let view = cx.entity();
                body.child(section("Indentation", &p))
                    .child(row(
                        "Tab size",
                        stepper(
                            "settings-tab-size",
                            d.tab_size.to_string(),
                            move |by, _, cx| {
                                view.update(cx, |v, cx| {
                                    v.set(cx, |s| s.tab_size = (s.tab_size as i32 + by).clamp(1, 8) as usize)
                                })
                            },
                            &p,
                        ),
                        &p,
                    ))
                    .child(
                        div()
                            .pl(px(162.))
                            .text_size(px(12.))
                            .text_color(p.fg_dim)
                            .child("For a file whose own indentation cannot be told."),
                    )
                    .child(section("Display", &p))
                    .child(
                        div().py(px(6.)).child(
                            look::checkbox("settings-soft-wrap", "Soft wrap long lines", d.soft_wrap, true, &p)
                                .debug_selector(|| "settings-soft-wrap".into())
                                .on_click(cx.listener(|this, _, _, cx| this.set(cx, |s| s.soft_wrap = !s.soft_wrap))),
                        ),
                    )
                    .child(
                        div().py(px(6.)).child(
                            look::checkbox("settings-whitespace", "Show whitespace", d.show_whitespace, true, &p)
                                .debug_selector(|| "settings-whitespace".into())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set(cx, |s| s.show_whitespace = !s.show_whitespace)
                                })),
                        ),
                    )
                    .into_any_element()
            }
            Page::VersionControl => {
                let view = cx.entity();
                body.child(section("Git", &p))
                    .child(row(
                        "Fetch every",
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(stepper(
                                "settings-fetch",
                                d.fetch_minutes.to_string(),
                                move |by, _, cx| {
                                    view.update(cx, |v, cx| {
                                        v.set(cx, |s| {
                                            s.fetch_minutes =
                                                (s.fetch_minutes as i64 + by as i64 * 5).clamp(5, 120) as u64
                                        })
                                    })
                                },
                                &p,
                            ))
                            .child(div().text_color(p.fg_dim).child("minutes")),
                        &p,
                    ))
                    .child(
                        div()
                            .pl(px(162.))
                            .text_size(px(12.))
                            .text_color(p.fg_dim)
                            .child("The projects are fetched in the background, to count what there is to pull."),
                    )
                    .into_any_element()
            }
            Page::Gradle => {
                let mut body = body.child(section("JDK per project", &p));
                if self.builds.is_empty() {
                    body = body.child(div().text_color(p.fg_dim).child("No Gradle build is open."));
                }
                for (i, root) in self.builds.clone().into_iter().enumerate() {
                    let ids = ["settings-jdk-0", "settings-jdk-1", "settings-jdk-2", "settings-jdk-3"];
                    let Some(id) = ids.get(i).copied() else { break };
                    let current = d.project_jdks.get(&root).map(|h| h.display().to_string()).unwrap_or_default();
                    let mut choices = vec![(String::new(), "Automatic (the build's Java level)".to_owned())];
                    choices.extend(
                        self.jdks
                            .iter()
                            .map(|j| (j.home.display().to_string(), format!("JDK {} ({})", j.major, j.version))),
                    );
                    let pick = set.clone();
                    let key = root.clone();
                    body = body.child(row(
                        "Project",
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().text_color(p.fg_dim).child(Model::dir_name(&root)))
                            .child(choice(
                                id,
                                &current,
                                choices,
                                move |v, cx| {
                                    let key = key.clone();
                                    pick(Box::new(move |s: &mut Settings| {
                                        if v.is_empty() {
                                            s.project_jdks.remove(&key);
                                        } else {
                                            s.project_jdks.insert(key.clone(), PathBuf::from(&v));
                                        }
                                    }))(cx)
                                },
                                &p,
                            )),
                        &p,
                    ));
                }
                if self.jdks.is_empty() {
                    body = body.child(div().pt(px(8.)).text_color(p.fg_dim).child("No JDK was found on this Mac."));
                }
                body.child(
                    div()
                        .pt(px(10.))
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child("The Java server restarts on the chosen JDK the next time it starts."),
                )
                .into_any_element()
            }
            Page::ClaudeCode => body
                .child(section("Program", &p))
                .child(row("Claude Code", div().w(px(360.)).child(look::field(&self.claude, false, None, &p)), &p))
                .child(
                    div()
                        .pl(px(162.))
                        .text_size(px(12.))
                        .text_color(p.fg_dim)
                        .child("Leave empty to find claude on PATH, in ~/.local/bin and the usual places."),
                )
                .into_any_element(),
        }
    }

    fn tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let query = self.search.read(cx).value().to_string();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut group = None;
        for page in Page::ALL.into_iter().filter(|pg| pg.matches(&query)) {
            if page.group() != group {
                group = page.group();
                if let Some(g) = group {
                    rows.push(
                        div()
                            .h(px(26.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(icon(Icon::ChevronDown, 11., p.fg_dim))
                            .child(g)
                            .into_any_element(),
                    );
                }
            }
            let on = page == self.page;
            let hover = p.button_hover;
            let id = format!("settings-page-{}", page.title().to_lowercase().replace(' ', "-"));
            let sel = id.clone();
            rows.push(
                div()
                    .id(SharedString::from(id))
                    .debug_selector(move || sel.clone())
                    .h(px(26.))
                    .pl(px(if page.group().is_some() { 28. } else { 12. }))
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .font_weight(if page.group().is_some() { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                    .when(on, |d| d.bg(p.selection()))
                    .when(!on, |d| d.hover(move |s| s.bg(hover)))
                    .child(page.title())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = page;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        if rows.is_empty() {
            rows.push(div().px(px(12.)).text_color(p.fg_dim).child("Nothing matches").into_any_element());
        }
        div()
            .w(px(220.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(8.))
            .bg(p.ground)
            .border_r_1()
            .border_color(p.border)
            .child(look::field(&self.search, true, None, &p))
            .child(div().id("settings-tree").flex_1().min_h_0().overflow_y_scrollbar().children(rows))
            .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let changed = self.changed(cx);
        let page = self.page_body(cx);
        let tree = self.tree(cx);
        let button = |id: &'static str, text: &'static str, primary: bool, enabled: bool| {
            let hover = if primary { mix(p.accent, 0.88, p.fg) } else { p.button_hover };
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .h(px(28.))
                .min_w(px(76.))
                .px(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .when(primary, |d| d.bg(p.accent).text_color(p.on_accent))
                .when(!primary, |d| d.bg(p.button).border_1().border_color(p.border))
                .when(!enabled, |d| d.opacity(0.5))
                .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
                .child(text)
        };
        div()
            .id("settings")
            .debug_selector(|| "settings".into())
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::search::SearchClose, _, cx| cx.emit(SettingsEvent::Cancel)))
            .occlude()
            .w(px(860.))
            .h(px(580.))
            .flex()
            .flex_col()
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(10.))
            .shadow_lg()
            .overflow_hidden()
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(
                div()
                    .h(px(34.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_b_1()
                    .border_color(p.border)
                    .bg(p.ground)
                    .text_color(p.fg_dim)
                    .child("Settings"),
            )
            .child(
                div().flex_1().min_h_0().flex().child(tree).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .px(px(20.))
                                .pt(px(14.))
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.page.title()),
                        )
                        .child(
                            div()
                                .id("settings-page")
                                .flex_1()
                                .min_h_0()
                                .px(px(20.))
                                .pb(px(12.))
                                .overflow_y_scrollbar()
                                .child(page),
                        ),
                ),
            )
            .child(
                div()
                    .h(px(52.))
                    .flex_none()
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(p.border)
                    .child(
                        button("settings-cancel", "Cancel", false, true)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Cancel))),
                    )
                    .child(
                        button("settings-apply", "Apply", false, changed)
                            .when(changed, |d| d.on_click(cx.listener(|this, _, _, cx| this.apply(false, cx)))),
                    )
                    .child(
                        button("settings-ok", "OK", true, true)
                            .on_click(cx.listener(|this, _, _, cx| this.apply(true, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::Page;

    #[test]
    fn the_search_finds_pages_by_title_group_and_keyword() {
        assert!(Page::Gradle.matches("build"));
        assert!(Page::Gradle.matches("JDK"));
        assert!(Page::Editor.matches("wrap"));
        assert!(Page::ClaudeCode.matches("tools"));
        assert!(!Page::Keymap.matches("theme"));
        assert!(Page::ALL.iter().all(|p| p.matches("")));
    }
}

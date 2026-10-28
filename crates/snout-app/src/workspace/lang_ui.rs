//! The workspace's side of the language servers: opening the places they point to (files
//! and library class files), applying their edits, the symbol boxes, the Problems and
//! References tabs, and the bar offering to download a JDK.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Task, Window, div, px,
};
use gpui_component::dock::{DockPlacement, PanelId, panel_handle};
use lsp_types::{Position, Uri, WorkspaceEdit};
use serde_json::{Value, json};

use super::Workspace;
use crate::editor::EditorPanel;
use crate::lang::edits::{self, Step};
use crate::lang::{HubEvent, LangHub};
use crate::look;
use crate::search::{Item, Mode, SearchEvent, SearchView};
use crate::theme::{mix, palette};

impl Workspace {
    pub(super) fn on_hub_event(
        &mut self,
        _: &Entity<LangHub>,
        e: &HubEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match e {
            HubEvent::Changed => cx.notify(),
            HubEvent::Open { root, uri, at } => self.open_location(root.clone(), uri.clone(), *at, window, cx),
            HubEvent::ApplyEdit { edit, reply } => {
                let applied = self.apply_edit(edit.clone(), window, cx);
                if let Some((root, id)) = reply {
                    self.hub.read(cx).reply_apply(root, id.clone(), applied);
                }
            }
            HubEvent::References { title, locations } => {
                let (title, locations) = (title.clone(), locations.clone());
                self.references.update(cx, |r, cx| r.show(title, locations, cx));
                self.show_bottom(PanelId::from(self.references.entity_id()), window, cx);
            }
        }
    }

    /// Open the bottom dock on the tab `id`.
    pub fn show_bottom(&mut self, id: PanelId, window: &mut Window, cx: &mut Context<Self>) {
        self.dock.update(cx, |d, cx| {
            if !d.is_dock_open(DockPlacement::Bottom) {
                d.toggle_dock(DockPlacement::Bottom, window, cx);
            }
            d.select_panel(id, window, cx);
        });
    }

    /// The Problems tab, in front.
    pub fn show_problems(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_bottom(PanelId::from(self.problems.entity_id()), window, cx);
    }

    /// Start the Java server when the project shown holds Gradle builds.
    pub(super) fn ensure_project_server(&mut self, cx: &mut Context<Self>) {
        if self.side.read(cx).activity != crate::panels::Activity::Project {
            return;
        }
        let Some(root) = self.model.read(cx).project_root() else { return };
        if self.hub.read(cx).servers.contains_key(&root) || !self.hub.read(cx).enabled {
            return;
        }
        if crate::project_model::has_gradle(&root) {
            self.hub.update(cx, |h, cx| h.ensure_server(&root, cx));
        }
    }

    /// Show a place a server pointed to: a file in its tab, or a library class file in a
    /// read only tab.
    pub fn open_location(
        &mut self,
        root: PathBuf,
        uri: Uri,
        at: Position,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = snout_lang::uri::to_path(&uri) {
            self.open_file(path.clone(), None, false, window, cx);
            if let Some(editor) = self.editors(cx).into_iter().find(|e| e.read(cx).path == path) {
                editor.update(cx, |e, cx| e.go_to_lsp(at, window, cx));
            }
            return;
        }
        let open =
            self.editors(cx).into_iter().find(|e| e.read(cx).class_file.as_ref().is_some_and(|(u, _)| *u == uri));
        let editor = match open {
            Some(editor) => {
                self.dock.update(cx, |d, cx| d.select_panel(PanelId::from(editor.entity_id()), window, cx));
                editor.update(cx, |e, cx| e.go_to_lsp(at, window, cx));
                return;
            }
            None => {
                let editor = cx.new(|cx| EditorPanel::open(PathBuf::new(), window, cx));
                let loading = "// Loading the class from the Java server…\n".to_owned();
                editor.update(cx, |e, cx| e.show_class_file(uri.clone(), root.clone(), loading, window, cx));
                self.track_editor(&editor, window, cx);
                let view = panel_handle(editor.clone());
                self.dock.update(cx, |d, cx| d.add_panel_view(view, DockPlacement::Center, None, window, cx));
                self.dock.update(cx, |d, cx| d.select_panel(PanelId::from(editor.entity_id()), window, cx));
                self.set_active_editor(Some(&editor), cx);
                editor
            }
        };
        self.load_class_file(editor, root, uri, Some(at), window, cx);
    }

    /// Fill a class file tab from the server (waiting for it to be ready).
    pub(super) fn load_class_file(
        &mut self,
        editor: Entity<EditorPanel>,
        root: PathBuf,
        uri: Uri,
        at: Option<Position>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hub = self.hub.clone();
        hub.update(cx, |h, cx| h.ensure_server(&root, cx));
        cx.spawn_in(window, async move |_, cx| {
            // A restored tab may wait for the server to start.
            for _ in 0..1200 {
                let ready = cx.update(|_, cx| hub.read(cx).client(&root).is_some()).unwrap_or(false);
                if ready {
                    break;
                }
                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            }
            let Ok(call) = cx.update(|_, cx| hub.read(cx).class_file(&root, &uri)) else { return };
            let text = match call.await {
                Ok(t) if !t.is_empty() => t,
                Ok(_) => "// The Java server has no source or class for this.\n".to_owned(),
                Err(e) => format!("// The Java server could not show this class: {e}\n"),
            };
            let _ = cx.update(|window, cx| {
                editor.update(cx, |e, cx| {
                    e.show_class_file(uri, root, text, window, cx);
                    if let Some(at) = at {
                        e.go_to_lsp(at, window, cx);
                    }
                })
            });
        })
        .detach();
    }

    /// Apply a server's edit; false when a part of it could not be applied.
    pub fn apply_edit(&mut self, edit: WorkspaceEdit, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut ok = true;
        let mut on_disk: Vec<PathBuf> = Vec::new();
        for step in edits::steps(edit) {
            match step {
                Step::Edit(path, list) => match self.editors(cx).into_iter().find(|e| e.read(cx).path == path) {
                    Some(editor) => editor.update(cx, |e, cx| e.apply_lsp_edits(&list, window, cx)),
                    None => match edits::edit_file(&path, &list) {
                        Ok(()) => on_disk.push(path),
                        Err(e) => {
                            ok = false;
                            eprintln!("[java] could not edit {}: {e}", path.display());
                        }
                    },
                },
                Step::Create(path) => {
                    if !path.exists() {
                        if let Some(dir) = path.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        ok &= std::fs::write(&path, "").is_ok();
                        on_disk.push(path);
                    }
                }
                Step::Rename(from, to) => {
                    if std::fs::rename(&from, &to).is_ok() {
                        let moved: Vec<Entity<EditorPanel>> =
                            self.editors(cx).into_iter().filter(|e| e.read(cx).path == from).collect();
                        for editor in moved {
                            editor.update(cx, |e, cx| {
                                // The text stays as it is (unsaved edits included); the tab
                                // now saves to the new name.
                                e.path = to.clone();
                                cx.notify();
                            });
                        }
                        on_disk.extend([from, to]);
                    } else {
                        ok = false;
                    }
                }
                Step::Delete(path) => {
                    ok &= std::fs::remove_file(&path).is_ok();
                    on_disk.push(path);
                }
            }
        }
        if !on_disk.is_empty() {
            self.hub.read(cx).files_changed(&on_disk);
        }
        ok
    }

    /// ⌘T, or ⇧⌘O in an editor (`file`): the symbol box.
    pub fn open_symbols(&mut self, file: bool, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.active_editor(window, cx);
        let link = editor.as_ref().and_then(|e| e.read(cx).lang.clone());
        let root = link.as_ref().map(|l| l.root.clone()).or_else(|| {
            let repo = self.model.read(cx).project_root()?;
            self.hub.read(cx).servers.keys().find(|r| r.starts_with(&repo) || repo.starts_with(r)).cloned()
        });
        let Some(root) = root else {
            self.model.update(cx, |m, cx| {
                m.message = "Symbols come from the Java server, which starts with a Java file".into();
                cx.notify();
            });
            return;
        };
        let hub = self.hub.clone();
        let source: crate::search::SymbolSource = if file {
            let Some(link) = link else { return };
            let text = editor.map(|e| e.read(cx).state.read(cx).text().clone());
            Rc::new(move |query: String, cx: &mut App| {
                let Some(text) = &text else { return Task::ready(Vec::new()) };
                let params = json!({ "textDocument": { "uri": link.uri.as_str() } });
                let call = link.ask(text, "textDocument/documentSymbol", params, cx);
                let (root, uri) = (link.root.clone(), link.uri.clone());
                cx.background_executor().spawn(async move {
                    let v = call.await.unwrap_or_default();
                    filter_symbols(file_symbols(&v, &root, &uri), &query)
                })
            })
        } else {
            symbols_of(hub, root)
        };
        let model = self.model.clone();
        let view = cx.new(|cx| SearchView::new(model, PathBuf::new(), Mode::Symbols, window, cx));
        view.update(cx, |v, cx| v.set_symbols(source, cx));
        cx.subscribe_in(&view, window, move |this, _, e: &SearchEvent, window, cx| {
            this.search = None;
            match e {
                SearchEvent::OpenLocation(root, uri, at) => {
                    this.open_location(root.clone(), uri.clone(), *at, window, cx);
                }
                _ => window.focus(&this.focus, cx),
            }
            cx.notify();
        })
        .detach();
        let v = view.clone();
        cx.defer_in(window, move |_, window, cx| v.update(cx, |v, cx| v.focus_input(window, cx)));
        self.search = Some(view);
        cx.notify();
    }

    /// The workspace's symbols, from the Java server of the open project, if one runs.
    pub fn workspace_symbols(&self, cx: &App) -> Option<crate::search::SymbolSource> {
        let repo = self.model.read(cx).project_root()?;
        let root = self.hub.read(cx).servers.keys().find(|r| r.starts_with(&repo) || repo.starts_with(r)).cloned()?;
        Some(symbols_of(self.hub.clone(), root))
    }

    /// The bar offering a JDK when the build needs one this Mac lacks.
    pub(super) fn jdk_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prompt = self.hub.read(cx).jdk_prompt.clone()?;
        let p = palette(cx);
        let text = match (&prompt.progress, &prompt.error) {
            (Some(progress), _) => match progress.percent() {
                Some(pct) => format!("Downloading JDK {}… {pct}%", prompt.major),
                None => format!("Downloading JDK {}…", prompt.major),
            },
            (None, Some(error)) => format!("JDK {} could not be downloaded: {error}", prompt.major),
            (None, None) => format!("This project needs JDK {}. Download it?", prompt.major),
        };
        let busy = prompt.progress.is_some();
        let hub = self.hub.clone();
        let hub2 = self.hub.clone();
        Some(
            div()
                .id("jdk-bar")
                .debug_selector(|| "jdk-bar".into())
                .flex_none()
                .mx(px(6.))
                .mt(px(6.))
                .px(px(10.))
                .py(px(5.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(8.))
                .bg(p.wash(p.accent))
                .border_1()
                .border_color(mix(p.accent, 0.4, p.panel))
                .text_size(px(12.5))
                .child(div().flex_1().min_w_0().child(text))
                .when_some(prompt.progress.as_ref().and_then(|pr| pr.percent()), |d, pct| {
                    d.child(
                        div()
                            .flex_none()
                            .w(px(160.))
                            .h(px(4.))
                            .rounded(px(2.))
                            .bg(p.border)
                            .child(div().h_full().rounded(px(2.)).bg(p.accent).w(px(160. * pct as f32 / 100.))),
                    )
                })
                .when(!busy, |d| {
                    let label = if prompt.error.is_some() { "Try again" } else { "Download" };
                    d.child(
                        look::small_button("jdk-download", label, &p)
                            .debug_selector(|| "jdk-download".into())
                            .on_click(move |_, _, cx| hub.update(cx, |h, cx| h.download_jdk(cx))),
                    )
                    .child(
                        look::small_button("jdk-dismiss", "Not now", &p)
                            .debug_selector(|| "jdk-dismiss".into())
                            .on_click(move |_, _, cx| hub2.update(cx, |h, cx| h.dismiss_jdk(cx))),
                    )
                })
                .into_any_element(),
        )
    }

    /// The bar saying the Gradle scripts changed since the model was read, offering to sync.
    pub(super) fn gradle_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let store = crate::project_model::store(cx)?;
        let s = store.read(cx);
        let text = match (&s.sync_error, s.scripts_changed) {
            (_, true) => "Gradle files changed. Sync?".to_owned(),
            (Some(e), false) if s.current.is_some() => format!("The Gradle sync failed: {e}"),
            _ => return None,
        };
        let changed = s.scripts_changed;
        let p = palette(cx);
        let (sync, dismiss) = (store.clone(), store.clone());
        Some(
            div()
                .id("gradle-bar")
                .debug_selector(|| "gradle-bar".into())
                .flex_none()
                .mx(px(6.))
                .mt(px(6.))
                .px(px(10.))
                .py(px(5.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(8.))
                .bg(p.wash(p.accent))
                .border_1()
                .border_color(mix(p.accent, 0.4, p.panel))
                .text_size(px(12.5))
                .child(crate::glyphs::icon(crate::glyphs::Icon::Build, 14., p.accent))
                .child(div().flex_1().min_w_0().child(text))
                .child(
                    look::small_button("gradle-sync", if changed { "Sync" } else { "Try again" }, &p)
                        .debug_selector(|| "gradle-sync".into())
                        .on_click(move |_, _, cx| sync.update(cx, |s, cx| s.sync(cx))),
                )
                .child(
                    look::small_button("gradle-dismiss", "Not now", &p)
                        .debug_selector(|| "gradle-dismiss".into())
                        .on_click(move |_, _, cx| {
                            dismiss.update(cx, |s, cx| {
                                s.sync_error = None;
                                s.dismiss_changes(cx);
                            })
                        }),
                )
                .into_any_element(),
        )
    }

    /// Open a library's class read only, as going to its definition does: the Java engine
    /// finds it by name and hands back its class file.
    pub fn open_library_class(&mut self, package: String, class: String, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.model.read(cx).project_root();
        let hub = self.hub.read(cx);
        let root = project
            .as_ref()
            .and_then(|project| hub.servers.keys().find(|r| r.starts_with(project) || project.starts_with(r)).cloned());
        let Some(root) = root.filter(|r| hub.client(r).is_some()) else {
            self.model.update(cx, |m, cx| {
                m.message = "Library classes open once the Java engine is ready".into();
                cx.notify();
            });
            return;
        };
        let call = hub.request(&root, "workspace/symbol", json!({ "query": class }));
        let qualified = if package.is_empty() { class.clone() } else { format!("{package}.{class}") };
        cx.spawn_in(window, async move |this, cx| {
            let answer = call.await.unwrap_or_default();
            let list: Vec<lsp_types::SymbolInformation> = serde_json::from_value(answer).unwrap_or_default();
            let hit = list
                .into_iter()
                .find(|s| s.name == class && s.container_name.as_deref().unwrap_or_default() == package);
            let _ = this.update_in(cx, |this, window, cx| match hit {
                Some(s) => this.open_location(root, s.location.uri, s.location.range.start, window, cx),
                None => this.model.update(cx, |m, cx| {
                    m.message = format!("The Java engine does not know {qualified}");
                    cx.notify();
                }),
            });
        })
        .detach();
    }
}

/// The rows for `workspace/symbol`'s answer.
fn workspace_symbols(v: &Value, root: &Path) -> Vec<Item> {
    let list: Vec<lsp_types::SymbolInformation> = serde_json::from_value(v.clone()).unwrap_or_default();
    list.into_iter()
        .take(crate::search::LIMIT)
        .map(|s| {
            let file = match snout_lang::uri::to_path(&s.location.uri) {
                Some(p) => {
                    p.strip_prefix(root).map(|r| r.display().to_string()).unwrap_or_else(|_| p.display().to_string())
                }
                None => s.container_name.clone().unwrap_or_default(),
            };
            let container = s.container_name.clone().unwrap_or_default();
            Item {
                path: s.name.clone(),
                detail: Some(if container.is_empty() { file } else { format!("{container}  {file}") }),
                target: Some((root.to_owned(), s.location.uri, s.location.range.start)),
                ..Default::default()
            }
        })
        .collect()
}

/// The rows for `textDocument/documentSymbol`'s answer, nested names flattened.
fn file_symbols(v: &Value, root: &Path, uri: &Uri) -> Vec<Item> {
    fn walk(list: &[lsp_types::DocumentSymbol], parent: &str, root: &Path, uri: &Uri, out: &mut Vec<Item>) {
        for s in list {
            out.push(Item {
                path: s.name.clone(),
                detail: Some(if parent.is_empty() {
                    format!("{}", s.selection_range.start.line + 1)
                } else {
                    format!("{parent}  {}", s.selection_range.start.line + 1)
                }),
                target: Some((root.to_owned(), uri.clone(), s.selection_range.start)),
                ..Default::default()
            });
            if let Some(children) = &s.children {
                let name = if parent.is_empty() { s.name.clone() } else { format!("{parent}.{}", s.name) };
                walk(children, &name, root, uri, out);
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(nested) = serde_json::from_value::<Vec<lsp_types::DocumentSymbol>>(v.clone()) {
        walk(&nested, "", root, uri, &mut out);
    } else if let Ok(flat) = serde_json::from_value::<Vec<lsp_types::SymbolInformation>>(v.clone()) {
        out = flat
            .into_iter()
            .map(|s| Item {
                path: s.name,
                detail: s.container_name,
                target: Some((root.to_owned(), s.location.uri, s.location.range.start)),
                ..Default::default()
            })
            .collect();
    }
    out
}

/// The symbols whose name matches `query` fuzzily, best first (all, in order, when empty).
fn filter_symbols(items: Vec<Item>, query: &str) -> Vec<Item> {
    use fuzzy_matcher::FuzzyMatcher as _;
    if query.trim().is_empty() {
        return items;
    }
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().ignore_case();
    let mut hits: Vec<(i64, usize, Item)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, it)| matcher.fuzzy_match(&it.path, query.trim()).map(|s| (s, i, it)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, _, it)| it).collect()
}

/// The symbols of the build at `root` that match a query, from its server.
fn symbols_of(hub: Entity<crate::lang::LangHub>, root: PathBuf) -> crate::search::SymbolSource {
    Rc::new(move |query: String, cx: &mut App| {
        if query.trim().is_empty() {
            return Task::ready(Vec::new());
        }
        let call = hub.read(cx).request(&root, "workspace/symbol", json!({ "query": query }));
        let root = root.clone();
        cx.background_executor().spawn(async move {
            let v = call.await.unwrap_or_default();
            workspace_symbols(&v, &root)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_document_symbols_with_their_parents() {
        let v = json!([{ "name": "App", "kind": 5,
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 9, "character": 1 } },
            "selectionRange": { "start": { "line": 0, "character": 13 }, "end": { "line": 0, "character": 16 } },
            "children": [{ "name": "main(String[])", "kind": 6,
                "range": { "start": { "line": 2, "character": 4 }, "end": { "line": 4, "character": 5 } },
                "selectionRange": { "start": { "line": 2, "character": 23 }, "end": { "line": 2, "character": 27 } } }] }]);
        let uri = snout_lang::uri::from_path(Path::new("/w/App.java"));
        let items = file_symbols(&v, Path::new("/w"), &uri);
        let names: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(names, ["App", "main(String[])"]);
        assert_eq!(items[1].detail.as_deref(), Some("App  3"));
        assert_eq!(filter_symbols(items, "main")[0].path, "main(String[])");
    }
}

//! The keymap: the app's default shortcuts over GPUI Kit's, with the user's changes from
//! `settings.json` on top. A change replaces every shortcut of its action (an empty list
//! takes them all away); the rest keep their defaults. Applying it rebuilds the bindings
//! at once, so a change in Settings works without a restart.

use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::{Action, App, Global, KeyBinding, KeyBindingContextPredicate, Keystroke, SharedString};

use crate::menus::*;

/// The user's shortcuts by action name (`snout::GoToFile`), each as GPUI writes keys
/// (`cmd-shift-p`, or `cmd-k cmd-s` for two in a row).
pub type Overrides = BTreeMap<String, Vec<String>>;

/// GPUI Kit's own bindings, as it made them before the app's.
struct KitBindings(Vec<KeyBinding>);

impl Global for KitBindings {}

/// The app's own bindings: keys, action, and the context they apply in.
fn ours() -> Vec<(&'static str, Box<dyn Action>, Option<&'static str>)> {
    use crate::claude_ide as c;
    use crate::editor as e;
    use crate::search as s;
    use gpui_component::input as i;
    vec![
        ("cmd-q", Box::new(Quit), None),
        ("cmd-o", Box::new(Open), None),
        ("cmd-j", Box::new(ToggleConsole), None),
        ("cmd-r", Box::new(Refresh), None),
        ("f5", Box::new(Refresh), None),
        ("cmd-shift-o", Box::new(Checkout), None),
        ("cmd-1", Box::new(ShowProject), None),
        ("cmd-2", Box::new(ShowGit), None),
        // Outside text fields, which take these keys first: ⌘⌫ moves the selected files to
        // the Trash, as in Finder, Esc leaves the commit view, and ↑ ↓ step through the
        // Journal while it shows.
        ("cmd-backspace", Box::new(Trash), Some("Workspace")),
        ("escape", Box::new(Back), Some("Workspace")),
        ("up", Box::new(PrevCommit), Some("Workspace")),
        ("down", Box::new(NextCommit), Some("Workspace")),
        ("cmd-a", Box::new(i::SelectAll), Some("Workspace")),
        ("cmd-s", Box::new(e::Save), Some("EditorPanel")),
        ("cmd-p", Box::new(GoToFile), None),
        ("cmd-shift-a", Box::new(SearchEverywhere), None),
        ("cmd-,", Box::new(OpenSettings), None),
        ("escape", Box::new(s::SearchClose), Some("BranchPopup")),
        ("escape", Box::new(s::SearchClose), Some("Settings")),
        ("escape", Box::new(s::SearchClose), Some("NewProject")),
        ("cmd-shift-f", Box::new(FindInFiles), None),
        // GPUI Kit's editor takes ⇧⌘F for replace; here it stays find in files, and
        // replace is ⌥⌘F. ⌘F finds in the file.
        ("cmd-shift-f", Box::new(FindInFiles), Some("EditorPanel > Input")),
        ("cmd-alt-f", Box::new(i::Replace), Some("EditorPanel > Input")),
        ("cmd-l", Box::new(GoToLine), None),
        ("cmd-shift-p", Box::new(CommandPalette), None),
        // Language features, in a Java editor.
        ("f12", Box::new(i::GoToDefinition), Some("EditorPanel > Input")),
        ("shift-f12", Box::new(e::FindReferences), Some("EditorPanel")),
        ("f2", Box::new(e::RenameSymbol), Some("EditorPanel")),
        ("shift-alt-f", Box::new(e::FormatDocument), Some("EditorPanel")),
        ("shift-alt-o", Box::new(e::OrganizeImports), Some("EditorPanel")),
        ("cmd-shift-space", Box::new(e::ShowSignature), Some("EditorPanel")),
        ("ctrl-space", Box::new(i::ShowCompletions), Some("EditorPanel > Input")),
        ("escape", Box::new(e::CancelRename), Some("RenameBox > Input")),
        ("cmd-t", Box::new(GoToSymbol), None),
        // In an editor ⇧⌘O lists the file's symbols; elsewhere it stays Check Out.
        ("cmd-shift-o", Box::new(GoToSymbolInFile), Some("EditorPanel > Input")),
        ("cmd-shift-m", Box::new(ShowProblems), None),
        // Running and debugging.
        ("ctrl-r", Box::new(RunSelected), None),
        ("ctrl-d", Box::new(DebugSelected), Some("Workspace")),
        ("ctrl-shift-r", Box::new(RunContext), None),
        ("ctrl-shift-d", Box::new(DebugContext), None),
        ("cmd-f2", Box::new(StopRuns), None),
        ("f8", Box::new(StepOver), None),
        ("f7", Box::new(StepInto), None),
        ("shift-f8", Box::new(StepOut), None),
        ("f9", Box::new(Resume), None),
        ("cmd-\\", Box::new(SplitRight), None),
        ("cmd-w", Box::new(CloseTab), Some("Workspace")),
        ("cmd-=", Box::new(ZoomIn), None),
        ("cmd-+", Box::new(ZoomIn), None),
        ("cmd--", Box::new(ZoomOut), None),
        ("cmd-0", Box::new(ZoomReset), None),
        ("cmd-/", Box::new(e::ToggleComment), Some("EditorPanel")),
        ("cmd-d", Box::new(e::DuplicateLine), Some("EditorPanel")),
        ("alt-up", Box::new(e::MoveLineUp), Some("EditorPanel > Input")),
        ("alt-down", Box::new(e::MoveLineDown), Some("EditorPanel > Input")),
        ("alt-z", Box::new(e::ToggleSoftWrap), Some("EditorPanel")),
        // In the go-to box the arrows pick and Esc closes, ahead of the field's own keys.
        ("up", Box::new(s::SearchUp), Some("SearchBox > Input")),
        ("down", Box::new(s::SearchDown), Some("SearchBox > Input")),
        ("escape", Box::new(s::SearchClose), Some("SearchBox > Input")),
        // A change Claude Code proposes: ⌘↩ accepts it, Esc (once the editor has nothing
        // of its own to dismiss) rejects it.
        ("cmd-enter", Box::new(e::AcceptChanges), Some("ClaudeReview")),
        ("cmd-enter", Box::new(e::AcceptChanges), Some("ClaudeReview > Input")),
        ("escape", Box::new(e::RejectChanges), Some("ClaudeReview")),
        ("cmd-alt-k", Box::new(e::SendToClaude), Some("EditorPanel")),
        // Claude's changes waiting for review: step through them, keep or revert one.
        ("ctrl-alt-shift-down", Box::new(c::NextClaudeChange), Some("EditorPanel")),
        ("ctrl-alt-shift-up", Box::new(c::PrevClaudeChange), Some("EditorPanel")),
        ("cmd-alt-y", Box::new(c::AcceptClaudeHunk), Some("EditorPanel")),
        ("cmd-alt-z", Box::new(c::RevertClaudeHunk), Some("EditorPanel")),
    ]
}

/// Take note of GPUI Kit's bindings (made by `gpui_component::init`), then bind the app's
/// with the user's changes.
pub fn init(overrides: &Overrides, cx: &mut App) {
    let kit: Vec<KeyBinding> = cx.key_bindings().borrow().bindings().cloned().collect();
    cx.set_global(KitBindings(kit));
    apply(overrides, cx);
}

/// Every default binding, GPUI Kit's then the app's, in the order they apply.
fn defaults(cx: &App) -> Vec<KeyBinding> {
    let mut out = cx.try_global::<KitBindings>().map(|k| k.0.clone()).unwrap_or_default();
    for (keys, action, context) in ours() {
        let predicate = context.and_then(|c| KeyBindingContextPredicate::parse(c).ok()).map(Rc::new);
        match KeyBinding::load(keys, action, predicate, false, None, cx) {
            Ok(b) => out.push(b),
            Err(e) => eprintln!("[keymap] {keys}: {e}"),
        }
    }
    out
}

/// The bindings in force with `overrides`.
fn effective(overrides: &Overrides, cx: &App) -> Vec<KeyBinding> {
    let defaults = defaults(cx);
    let mut out: Vec<KeyBinding> =
        defaults.iter().filter(|b| !overrides.contains_key(b.action().name())).cloned().collect();
    for (name, keys) in overrides {
        let template = defaults.iter().find(|b| b.action().name() == name);
        let action = match template {
            Some(b) => b.action().boxed_clone(),
            None => match action_named(name, cx) {
                Some(a) => a,
                None => continue,
            },
        };
        let predicate = template.and_then(KeyBinding::predicate);
        for keys in keys {
            match KeyBinding::load(keys, action.boxed_clone(), predicate.clone(), false, None, cx) {
                Ok(b) => out.push(b),
                Err(e) => eprintln!("[keymap] {name}: {keys}: {e}"),
            }
        }
    }
    out
}

/// The action called `name`, if the keymap lists it.
fn action_named(name: &str, cx: &App) -> Option<Box<dyn Action>> {
    cx.build_action(name, None).ok().or_else(|| commands().into_iter().find(|(_, a)| a.name() == name).map(|(_, a)| a))
}

/// Put the bindings with `overrides` in force.
pub fn apply(overrides: &Overrides, cx: &mut App) {
    let bindings = effective(overrides, cx);
    cx.clear_key_bindings();
    cx.bind_keys(bindings);
}

/// The commands the keymap lists, by name.
fn commands() -> Vec<(SharedString, Box<dyn Action>)> {
    crate::menus::commands(&[])
}

/// A binding's keys as GPUI writes them.
fn keys_of(b: &KeyBinding, cx: &App) -> String {
    let os = cx.platform_info().os;
    b.keystrokes().iter().map(|k| k.unparse(os)).collect::<Vec<_>>().join(" ")
}

/// Keys as GPUI writes them (`cmd-shift-p`), as the menus show them (⇧⌘P).
pub fn display(keys: &str, cx: &App) -> String {
    let os = cx.platform_info().os;
    keys.split_whitespace()
        .map(|k| Keystroke::parse_with_os(k, os).map(|k| k.display(os).to_string()).unwrap_or_else(|_| k.to_owned()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One command of the keymap.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: SharedString,
    /// The action's name, which the settings key changes by.
    pub action: String,
    /// The keys it has by default, and those it has now.
    pub defaults: Vec<String>,
    pub keys: Vec<String>,
    /// Where its keys apply (`EditorPanel`), none for anywhere.
    pub context: Option<String>,
}

impl Entry {
    pub fn changed(&self) -> bool {
        self.keys != self.defaults
    }

    /// Whether the search `query` finds it, by name or by keys.
    pub fn matches(&self, query: &str, cx: &App) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.name.to_lowercase().contains(&q)
            || self.keys.iter().any(|k| k.contains(&q) || display(k, cx).to_lowercase().contains(&q))
    }
}

/// Every command the keymap lists, with its keys under `overrides`.
pub fn entries(overrides: &Overrides, cx: &App) -> Vec<Entry> {
    let defaults = defaults(cx);
    let mut out: Vec<Entry> = Vec::new();
    for (name, action) in commands() {
        let id = action.name().to_owned();
        if out.iter().any(|e| e.action == id) {
            continue;
        }
        let mine: Vec<&KeyBinding> = defaults.iter().filter(|b| b.action().name() == id).collect();
        let mut keys_default: Vec<String> = Vec::new();
        for b in &mine {
            let k = keys_of(b, cx);
            if !keys_default.contains(&k) {
                keys_default.push(k);
            }
        }
        let context = mine.first().and_then(|b| b.predicate()).map(|p| p.to_string());
        let keys = overrides.get(&id).cloned().unwrap_or_else(|| keys_default.clone());
        out.push(Entry { name, action: id, defaults: keys_default, keys, context });
    }
    out
}

/// Which commands each key runs where, under some overrides: to tell the user when keys
/// they give a command already belong to another.
pub struct Conflicts {
    /// (keys, context) to the actions bound there, by action name, in binding order.
    bound: std::collections::HashMap<(String, Option<String>), Vec<String>>,
    names: Vec<(String, SharedString)>,
}

impl Conflicts {
    pub fn new(overrides: &Overrides, cx: &App) -> Self {
        let mut bound: std::collections::HashMap<(String, Option<String>), Vec<String>> = Default::default();
        for b in effective(overrides, cx) {
            let key = (keys_of(&b, cx), b.predicate().map(|p| p.to_string()));
            let actions = bound.entry(key).or_default();
            let name = b.action().name().to_owned();
            if !actions.contains(&name) {
                actions.push(name);
            }
        }
        let names = commands().into_iter().map(|(n, a)| (a.name().to_owned(), n)).collect();
        Self { bound, names }
    }

    /// The other actions `keys` is bound to in `context`, by action name.
    pub fn others(&self, keys: &str, context: Option<&str>, action: &str) -> Vec<String> {
        self.bound
            .get(&(keys.to_owned(), context.map(str::to_owned)))
            .map(|v| v.iter().filter(|a| *a != action).cloned().collect())
            .unwrap_or_default()
    }

    /// What the user calls an action.
    pub fn name(&self, action: &str) -> String {
        self.names
            .iter()
            .find(|(a, _)| a == action)
            .map(|(_, n)| n.trim_end_matches('…').to_string())
            .unwrap_or_else(|| action.rsplit("::").next().unwrap_or(action).to_owned())
    }
}

/// `overrides` with `action` given `keys` (the defaults again drop the change).
pub fn set(overrides: &mut Overrides, entry: &Entry, keys: Vec<String>) {
    if keys == entry.defaults {
        overrides.remove(&entry.action);
    } else {
        overrides.insert(entry.action.clone(), keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn changes_replace_an_actions_keys_and_conflicts_are_found(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            init(&Overrides::new(), cx);
            let all = entries(&Overrides::new(), cx);
            let go = all.iter().find(|e| e.action == "snout::GoToFile").expect("Go to File is listed").clone();
            assert_eq!(go.keys, ["cmd-p"]);
            assert!(!go.changed());
            // No two listed commands share keys where they apply, as shipped.
            let none = Conflicts::new(&Overrides::new(), cx);
            for e in &all {
                for k in &e.keys {
                    assert!(none.others(k, e.context.as_deref(), &e.action).is_empty(), "{} {k}", e.name);
                }
            }
            // Go to File on ⌘K: ⌘P is free, ⌘K runs it, and the menus' shortcut follows.
            let mut mine = Overrides::new();
            set(&mut mine, &go, vec!["cmd-k".into()]);
            apply(&mine, cx);
            let map = cx.key_bindings();
            let found: Vec<String> = map
                .borrow()
                .bindings()
                .filter(|b| b.action().name() == "snout::GoToFile")
                .map(|b| keys_of(b, cx))
                .collect();
            assert_eq!(found, ["cmd-k"]);
            // ⌘, on Go to File too: Settings had it.
            set(&mut mine, &go, vec!["cmd-k".into(), "cmd-,".into()]);
            let c = Conflicts::new(&mine, cx);
            let others: Vec<String> = c.others("cmd-,", None, "snout::GoToFile").iter().map(|a| c.name(a)).collect();
            assert_eq!(others, ["Settings"]);
            // The defaults again: no change is kept.
            set(&mut mine, &go, vec!["cmd-p".into()]);
            assert!(mine.is_empty());
            assert_eq!(display("cmd-shift-p", cx), "⌘⇧P");
        });
    }
}

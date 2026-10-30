//! The keymap: the app's default shortcuts over GPUI Kit's, with the user's changes from
//! `settings.json` on top. A change replaces every shortcut of its action (an empty list
//! takes them all away); the rest keep their defaults. Applying it rebuilds the bindings
//! at once, so a change in Settings works without a restart.
//!
//! The defaults are written for the Mac. On Linux (see [`linux_keys`]) Ctrl takes ⌘'s place,
//! except where that would clash with a key Ctrl already has: there IntelliJ IDEA's keymap
//! for Linux decides (Shift+F10 runs, Shift+F9 debugs, Ctrl+F5 reruns, Ctrl+R replaces, Alt+1
//! shows the project), and no default uses Ctrl+Alt with a function key, which switches the
//! virtual console. Shortcuts show as "Ctrl+Shift+F" there, and the terminal keeps the
//! Ctrl keys a shell needs (Ctrl+Shift+C and Ctrl+Shift+V copy and paste).

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

/// A binding of the app's own: keys, action, and the context it applies in.
type Binding = (&'static str, Box<dyn Action>, Option<&'static str>);

/// The app's own bindings on this system: keys, action, and the context they apply in.
fn ours() -> Vec<(String, Box<dyn Action>, Option<&'static str>)> {
    if cfg!(target_os = "macos") {
        return mac_defaults().into_iter().map(|(k, a, c)| (k.to_owned(), a, c)).collect();
    }
    let mut out: Vec<(String, Box<dyn Action>, Option<&'static str>)> = mac_defaults()
        .into_iter()
        .filter_map(|(k, a, c)| {
            let keys = linux_keys(k, a.name())?;
            Some((keys, a, c))
        })
        .collect();
    out.extend(linux_extras().into_iter().map(|(k, a, c)| (k.to_owned(), a, c)));
    out
}

/// Where Linux's keys differ from ⌘ turned into Ctrl: by action (its last name) and Mac keys,
/// the keys on Linux, or none for no default there.
const LINUX: &[(&str, &str, Option<&str>)] = &[
    // Ctrl+R and Ctrl+D are Rerun and Duplicate Line there: run and debug as IntelliJ does.
    ("RunSelected", "ctrl-r", Some("shift-f10")),
    ("DebugSelected", "ctrl-d", Some("shift-f9")),
    ("RunContext", "ctrl-shift-r", Some("ctrl-shift-f10")),
    ("DebugContext", "ctrl-shift-d", Some("ctrl-shift-f9")),
    ("Rerun", "cmd-r", Some("ctrl-f5")),
    // GPUI Kit's Ctrl+H (Replace) gives way to the Type Hierarchy; Replace is Ctrl+R.
    ("Replace", "cmd-alt-f", Some("ctrl-r")),
    ("ExtractField", "cmd-alt-shift-f", Some("ctrl-alt-f")),
    // The tool windows on Alt and a digit, Go to Class on Ctrl+N, New on Alt+Insert.
    ("ShowProject", "cmd-1", Some("alt-1")),
    ("ShowGit", "cmd-2", Some("alt-2")),
    ("ShowStructure", "cmd-7", Some("alt-7")),
    ("GoToClass", "cmd-alt-o", Some("ctrl-n")),
    ("NewFile", "cmd-n", Some("alt-insert")),
    // Files go to the Trash with Delete, as in the desktop's file managers.
    ("Trash", "cmd-backspace", Some("delete")),
    // Ctrl+Alt+F1 to F12 switch the virtual console.
    ("PauseProgram", "cmd-alt-f9", None),
    ("ToggleBreakpointEnabled", "cmd-alt-f8", None),
    ("EditBreakpointCondition", "cmd-alt-shift-f8", None),
];

/// A default binding's keys on Linux: from [`LINUX`], else ⌘ as Ctrl; none for no default.
pub fn linux_keys(mac: &str, action: &str) -> Option<String> {
    let short = action.rsplit("::").next().unwrap_or(action);
    if let Some((_, _, keys)) = LINUX.iter().find(|(a, k, _)| *a == short && *k == mac) {
        return keys.map(str::to_owned);
    }
    Some(mac.split(' ').map(|k| k.replace("cmd-", "ctrl-")).collect::<Vec<_>>().join(" "))
}

/// Bindings Linux has besides the Mac's: the editor's Type Hierarchy ahead of GPUI Kit's
/// Ctrl+H, Redo on Ctrl+Shift+Z too, and the terminal's keys. In a terminal the Ctrl keys
/// go to the shell (Ctrl+C interrupts, Ctrl+R searches its history, Ctrl+W deletes a word),
/// not to the commands they run elsewhere; copy, paste and find take Shift as well.
fn linux_extras() -> Vec<Binding> {
    use gpui_component::input as i;
    let mut out: Vec<Binding> = vec![
        ("ctrl-h", Box::new(TypeHierarchy), Some("EditorPanel > Input")),
        ("ctrl-shift-z", Box::new(i::Redo), Some("EditorPanel > Input")),
        ("ctrl-shift-c", Box::new(i::Copy), Some("Terminal")),
        ("ctrl-shift-v", Box::new(i::Paste), Some("Terminal")),
        ("ctrl-shift-f", Box::new(gpui::NoAction), Some("Terminal")),
        ("ctrl-\\", Box::new(gpui::NoAction), Some("Terminal")),
    ];
    const CTRL_LETTERS: [&str; 26] = [
        "ctrl-a", "ctrl-b", "ctrl-c", "ctrl-d", "ctrl-e", "ctrl-f", "ctrl-g", "ctrl-h", "ctrl-i", "ctrl-j", "ctrl-k",
        "ctrl-l", "ctrl-m", "ctrl-n", "ctrl-o", "ctrl-p", "ctrl-q", "ctrl-r", "ctrl-s", "ctrl-t", "ctrl-u", "ctrl-v",
        "ctrl-w", "ctrl-x", "ctrl-y", "ctrl-z",
    ];
    out.extend(CTRL_LETTERS.into_iter().map(|k| (k, Box::new(gpui::NoAction) as Box<dyn Action>, Some("Terminal"))));
    out
}

/// The app's bindings as written for the Mac.
fn mac_defaults() -> Vec<Binding> {
    use crate::claude_ide as c;
    use crate::editor as e;
    use crate::search as s;
    use gpui_component::input as i;
    vec![
        ("cmd-q", Box::new(Quit), None),
        ("cmd-o", Box::new(Open), None),
        ("cmd-j", Box::new(ToggleConsole), None),
        // ⌘R runs again what ran last; F5 refreshes.
        ("f5", Box::new(Refresh), None),
        ("cmd-shift-o", Box::new(Checkout), None),
        ("cmd-1", Box::new(ShowProject), None),
        ("cmd-2", Box::new(ShowGit), None),
        ("cmd-7", Box::new(ShowStructure), None),
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
        ("cmd-e", Box::new(RecentFiles), None),
        ("cmd-n", Box::new(NewFile), None),
        // Go to Class: ⌘O opens a project.
        ("cmd-alt-o", Box::new(GoToClass), None),
        ("cmd-shift-a", Box::new(SearchEverywhere), None),
        ("cmd-,", Box::new(OpenSettings), None),
        ("cmd-;", Box::new(ProjectStructure), None),
        ("escape", Box::new(s::SearchClose), Some("BranchPopup")),
        ("escape", Box::new(s::SearchClose), Some("Settings")),
        ("escape", Box::new(s::SearchClose), Some("ProjectStructure")),
        ("escape", Box::new(s::SearchClose), Some("NewProject")),
        ("cmd-shift-f", Box::new(FindInFiles), None),
        // GPUI Kit's editor takes ⇧⌘F for replace; here it stays find in files, and
        // replace is ⌥⌘F. ⌘F finds in the file.
        ("cmd-shift-f", Box::new(FindInFiles), Some("EditorPanel > Input")),
        ("cmd-alt-f", Box::new(i::Replace), Some("EditorPanel > Input")),
        ("cmd-shift-r", Box::new(ReplaceInFiles), None),
        ("cmd-l", Box::new(GoToLine), None),
        ("cmd-shift-p", Box::new(CommandPalette), None),
        // Language features, in a Java editor.
        ("f12", Box::new(i::GoToDefinition), Some("EditorPanel > Input")),
        // ⌘B and ⌥⌘B as in IntelliJ: on a declaration, ⌘B lists its usages.
        ("cmd-b", Box::new(e::GoToDeclaration), Some("EditorPanel > Input")),
        ("cmd-alt-b", Box::new(e::GoToImplementations), Some("EditorPanel > Input")),
        ("shift-f12", Box::new(e::FindReferences), Some("EditorPanel")),
        ("f2", Box::new(e::RenameSymbol), Some("EditorPanel")),
        ("shift-alt-f", Box::new(e::FormatDocument), Some("EditorPanel")),
        ("cmd-alt-l", Box::new(e::FormatSelection), Some("EditorPanel")),
        ("shift-alt-o", Box::new(e::OrganizeImports), Some("EditorPanel")),
        ("cmd-shift-space", Box::new(e::ShowSignature), Some("EditorPanel")),
        // Refactoring. Extract Field is ⌥⇧⌘F: ⌥⌘F is Replace in the editor.
        ("cmd-alt-m", Box::new(e::ExtractMethod), Some("EditorPanel")),
        ("cmd-alt-v", Box::new(e::ExtractVariable), Some("EditorPanel")),
        ("cmd-alt-c", Box::new(e::ExtractConstant), Some("EditorPanel")),
        ("cmd-alt-shift-f", Box::new(e::ExtractField), Some("EditorPanel")),
        ("cmd-alt-n", Box::new(e::Inline), Some("EditorPanel")),
        ("ctrl-space", Box::new(i::ShowCompletions), Some("EditorPanel > Input")),
        // Show Context Actions: the fixes and actions at the caret. ⌥↩ as in IntelliJ, and ⌘.
        // (GPUI Kit's Quick Fix key) too; the menus show the last, ⌥↩. ⌥⇧↩ applies the best fix.
        ("cmd-.", Box::new(e::ShowContextActions), Some("EditorPanel > Input")),
        ("alt-enter", Box::new(e::ShowContextActions), Some("EditorPanel > Input")),
        ("alt-enter", Box::new(e::ShowContextActions), Some("ProblemsPanel")),
        ("alt-shift-enter", Box::new(e::ApplyTopFix), Some("EditorPanel > Input")),
        ("escape", Box::new(e::CancelRename), Some("RenameBox > Input")),
        ("cmd-t", Box::new(GoToSymbol), None),
        ("cmd-f12", Box::new(FileStructure), Some("EditorPanel")),
        ("ctrl-h", Box::new(TypeHierarchy), Some("EditorPanel")),
        ("ctrl-alt-h", Box::new(CallHierarchy), Some("EditorPanel")),
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
        ("cmd-alt-f9", Box::new(PauseProgram), None),
        ("alt-f9", Box::new(RunToCursor), Some("EditorPanel")),
        ("alt-f8", Box::new(EvaluateExpression), None),
        ("cmd-f8", Box::new(ToggleBreakpoint), Some("EditorPanel")),
        ("cmd-alt-f8", Box::new(ToggleBreakpointEnabled), Some("EditorPanel")),
        ("cmd-alt-shift-f8", Box::new(EditBreakpointCondition), Some("EditorPanel")),
        ("cmd-shift-f8", Box::new(ViewBreakpoints), None),
        ("cmd-r", Box::new(Rerun), None),
        ("cmd-alt-r", Box::new(RerunFailedTests), None),
        ("ctrl-alt-g", Box::new(RunGradleTask), None),
        ("cmd-f9", Box::new(BuildProject), None),
        ("alt-f12", Box::new(ShowTerminal), None),
        ("ctrl-alt-p", Box::new(SpringProfiles), None),
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
        // Annotate with Git Blame: B for blame (IntelliJ leaves it unbound).
        ("cmd-alt-shift-b", Box::new(e::AnnotateWithGitBlame), Some("EditorPanel")),
        // In the go-to box the arrows pick and Esc closes, ahead of the field's own keys.
        ("up", Box::new(s::SearchUp), Some("SearchBox > Input")),
        ("down", Box::new(s::SearchDown), Some("SearchBox > Input")),
        ("escape", Box::new(s::SearchClose), Some("SearchBox > Input")),
        ("escape", Box::new(s::SearchClose), Some("TerminalFind > Input")),
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
        // All of them at once, wherever the keyboard is.
        ("cmd-alt-shift-y", Box::new(c::AcceptAllClaude), None),
        ("cmd-alt-shift-z", Box::new(c::RevertAllClaude), None),
        // A dialog that destroys something takes ⌘⌫ (or a click), never ⏎.
        ("cmd-backspace", Box::new(crate::dialogs::ConfirmDestructive), Some("SnoutDialog")),
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
        match KeyBinding::load(&keys, action, predicate, false, None, cx) {
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

/// Keys as GPUI writes them (`cmd-shift-p`), as the menus show them: ⇧⌘P on a Mac (Return
/// as ↩), Ctrl+Shift+P elsewhere.
pub fn display(keys: &str, cx: &App) -> String {
    let os = cx.platform_info().os;
    keys.split_whitespace()
        .map(|k| match Keystroke::parse_with_os(k, os) {
            Ok(k) => keystroke_label(&k, os),
            Err(_) => k.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A binding's keys as the menus show them (see [`display`]).
pub fn binding_label(b: &KeyBinding, cx: &App) -> String {
    use gpui::AsKeystroke as _;
    let os = cx.platform_info().os;
    b.keystrokes().iter().map(|k| keystroke_label(k.as_keystroke(), os)).collect::<Vec<_>>().join(" ")
}

/// One keystroke as `os` writes it: ⇧⌘P on a Mac (Return as ↩), Ctrl+Shift+P elsewhere.
fn keystroke_label(k: &Keystroke, os: gpui::OperatingSystem) -> String {
    if os != gpui::OperatingSystem::Mac {
        return plus_style(k);
    }
    let shown = k.display(os).to_string();
    match shown.strip_suffix("enter") {
        Some(mods) => format!("{mods}\u{21a9}"),
        None => shown,
    }
}

/// A keystroke as Linux and Windows write it: Ctrl+Alt+Shift+F12, Ctrl+Enter.
fn plus_style(k: &Keystroke) -> String {
    let m = k.modifiers;
    let mut parts: Vec<String> = Vec::new();
    for (on, name) in [(m.control, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift"), (m.platform, "Super")] {
        if on {
            parts.push(name.to_owned());
        }
    }
    let key = match k.key.as_str() {
        "pageup" => "Page Up".to_owned(),
        "pagedown" => "Page Down".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "escape" => "Esc".to_owned(),
        key if key.len() == 1 => key.to_uppercase(),
        key if key.starts_with('f') && key[1..].parse::<u8>().is_ok() => key.to_uppercase(),
        key => {
            let mut c = key.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    };
    parts.push(key);
    parts.join("+")
}

/// Keys written for the Mac (`cmd-shift-f`), as this system binds them by default for
/// `action` and shows them: for a hint or a tooltip that names a shortcut.
pub fn label(mac: &str, action: &dyn Action, cx: &App) -> String {
    if cfg!(target_os = "macos") {
        return display(mac, cx);
    }
    linux_keys(mac, action.name()).map(|k| display(&k, cx)).unwrap_or_default()
}

/// Keys written for the Mac, as this system shows them, for keys that are no action's (⌘⌫ in a
/// dialog, ⌘↩ in a field): ⌘ is Ctrl elsewhere.
pub fn keys(mac: &str, cx: &App) -> String {
    if cfg!(target_os = "macos") {
        return display(mac, cx);
    }
    display(&mac.replace("cmd-", "ctrl-"), cx)
}

/// The keys of `action` as the menus show them (its last binding, which they show), if
/// it has any.
pub fn shortcut(action: &dyn Action, cx: &App) -> Option<String> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let b = keymap.bindings_for_action(action).next_back()?;
    Some(display(&keys_of(b, cx), cx))
}

/// A text naming shortcuts the Mac's way (`Run (⌃R)`, `⌘⏎`), as this system shows them: as it
/// is on a Mac; on Linux each shortcut as Linux binds it ([`linux_keys`]), written
/// `Ctrl+Shift+F`, and a shortcut Linux leaves unbound taken out with its parentheses.
pub fn shown(text: &str) -> String {
    if cfg!(target_os = "macos") { text.to_owned() } else { linux_text(text) }
}

/// Keys written as the Mac shows them (`⌘⌫`), as this system writes the same keys, ⌘ as Ctrl:
/// for keys that are no command's own (a dialog's, a field's).
pub fn shown_keys(text: &str) -> String {
    if cfg!(target_os = "macos") { text.to_owned() } else { linux_text(&text.replace('⌘', "⌃")) }
}

/// [`shown`] for Linux.
pub fn linux_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let mut m = gpui::Modifiers::default();
        while let Some(c) = chars.get(i) {
            match c {
                '⌃' => m.control = true,
                '⌥' => m.alt = true,
                '⇧' => m.shift = true,
                '⌘' => m.platform = true,
                _ => break,
            }
            i += 1;
        }
        if i == start {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let key = match chars.get(i) {
            Some('F') if chars.get(i + 1).is_some_and(char::is_ascii_digit) => {
                let mut k = String::from("f");
                i += 1;
                while let Some(d) = chars.get(i).filter(|d| d.is_ascii_digit()) {
                    k.push(*d);
                    i += 1;
                }
                Some(k)
            }
            Some(c) if !c.is_whitespace() && !matches!(c, ')' | '(') => {
                i += 1;
                Some(match c {
                    '↩' | '⏎' => "enter".to_owned(),
                    '⌫' => "backspace".to_owned(),
                    '↑' => "up".to_owned(),
                    '↓' => "down".to_owned(),
                    c => c.to_lowercase().collect(),
                })
            }
            _ => None,
        };
        let Some(key) = key else {
            out.extend(&chars[start..i]);
            continue;
        };
        let mac = Keystroke { modifiers: m, key, key_char: None };
        if let Some(k) = linux_keystroke(&mac) {
            out.push_str(&plus_style(&k));
        }
    }
    // An unbound shortcut leaves "()" or "(, …)" behind.
    let out = out.replace(" ()", "").replace("()", "").replace("(, ", "(").replace(", )", ")");
    out.trim_end().to_owned()
}

/// A keystroke written for the Mac, as Linux binds it by default: the [`LINUX`] entry with
/// those keys, else ⌘ as Ctrl; none when Linux leaves it unbound.
fn linux_keystroke(mac: &Keystroke) -> Option<Keystroke> {
    let os = gpui::OperatingSystem::Mac;
    let entry = LINUX.iter().find(|(_, k, _)| Keystroke::parse_with_os(k, os).is_ok_and(|k| &k == mac));
    match entry {
        Some((_, _, None)) => None,
        Some((_, _, Some(keys))) => Keystroke::parse_with_os(keys, gpui::OperatingSystem::Linux).ok(),
        None => {
            let mut k = mac.clone();
            if k.modifiers.platform {
                k.modifiers.platform = false;
                k.modifiers.control = true;
            }
            Some(k)
        }
    }
}

/// Keys written for the Mac (`cmd-s`, `cmd-k cmd-s`), as this system binds the same command
/// by default (`ctrl-s`): for tests that press a command's keys.
pub fn platform_keys(mac: &str) -> String {
    if cfg!(target_os = "macos") {
        return mac.to_owned();
    }
    mac.split_whitespace()
        .map(|k| match Keystroke::parse_with_os(k, gpui::OperatingSystem::Mac) {
            Ok(m) => linux_keystroke(&m).map(|l| l.unparse(gpui::OperatingSystem::Linux)).unwrap_or_default(),
            Err(_) => k.to_owned(),
        })
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
            let k = platform_keys;
            assert_eq!(go.keys, [k("cmd-p")]);
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
            set(&mut mine, &go, vec![k("cmd-k")]);
            apply(&mine, cx);
            let map = cx.key_bindings();
            let found: Vec<String> = map
                .borrow()
                .bindings()
                .filter(|b| b.action().name() == "snout::GoToFile")
                .map(|b| keys_of(b, cx))
                .collect();
            assert_eq!(found, [k("cmd-k")]);
            // ⌘, on Go to File too: Settings had it.
            set(&mut mine, &go, vec![k("cmd-k"), k("cmd-,")]);
            let c = Conflicts::new(&mine, cx);
            let others: Vec<String> =
                c.others(&k("cmd-,"), None, "snout::GoToFile").iter().map(|a| c.name(a)).collect();
            assert_eq!(others, ["Settings"]);
            // The defaults again: no change is kept.
            set(&mut mine, &go, vec![k("cmd-p")]);
            assert!(mine.is_empty());
            let shown = if cfg!(target_os = "macos") { "⌘⇧P" } else { "Ctrl+Shift+P" };
            assert_eq!(display(&k("cmd-shift-p"), cx), shown);
        });
    }

    #[gpui::test]
    fn the_run_and_debug_keys_belong_to_nothing_else(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            init(&Overrides::new(), cx);
            let kit = cx.global::<KitBindings>().0.clone();
            for keys in [
                "cmd-f8",
                "cmd-alt-f8",
                "cmd-alt-shift-f8",
                "cmd-shift-f8",
                "alt-f8",
                "alt-f9",
                "cmd-alt-f9",
                "cmd-r",
                "cmd-alt-r",
                "ctrl-alt-g",
                "cmd-f9",
                "alt-f12",
                "ctrl-alt-p",
                "cmd-shift-r",
                // Project Structure.
                "cmd-;",
            ] {
                let os = cx.platform_info().os;
                let keys = platform_keys(keys);
                if keys.is_empty() {
                    // No default on this system.
                    continue;
                }
                let keys = Keystroke::parse_with_os(&keys, os).unwrap().unparse(os);
                let keys = keys.as_str();
                let taken: Vec<&str> =
                    kit.iter().filter(|b| keys_of(b, cx) == keys).map(|b| b.action().name()).collect();
                assert!(taken.is_empty(), "{keys} is GPUI Kit's: {taken:?}");
                let ours: Vec<String> = defaults(cx)
                    .iter()
                    .filter(|b| keys_of(b, cx) == keys)
                    .map(|b| b.action().name().to_owned())
                    .collect();
                assert_eq!(ours.len(), 1, "{keys}: {ours:?}");
            }
        });
    }

    #[test]
    fn linux_takes_ctrl_for_cmd_and_intellij_keys_where_ctrl_was_taken() {
        assert_eq!(linux_keys("cmd-shift-f", "snout::FindInFiles").as_deref(), Some("ctrl-shift-f"));
        assert_eq!(linux_keys("cmd-k cmd-s", "snout::X").as_deref(), Some("ctrl-k ctrl-s"));
        assert_eq!(linux_keys("ctrl-r", "snout::RunSelected").as_deref(), Some("shift-f10"));
        assert_eq!(linux_keys("cmd-r", "snout::Rerun").as_deref(), Some("ctrl-f5"));
        assert_eq!(linux_keys("cmd-alt-f9", "snout::PauseProgram"), None);
        assert_eq!(linux_keys("alt-enter", "editor::ShowContextActions").as_deref(), Some("alt-enter"));
        // Every Linux default parses, and none uses Ctrl+Alt with a function key.
        for (keys, action, _) in mac_defaults() {
            if let Some(l) = linux_keys(keys, action.name()) {
                let k = Keystroke::parse_with_os(&l, gpui::OperatingSystem::Linux).unwrap();
                assert!(!k.modifiers.platform, "{l}");
                let fkey = k.key.starts_with('f') && k.key.len() > 1 && k.key[1..].parse::<u8>().is_ok();
                assert!(!(k.modifiers.control && k.modifiers.alt && fkey), "{l} switches the console");
            }
        }
    }

    #[test]
    fn mac_shortcuts_in_text_read_as_linux_writes_them() {
        assert_eq!(linux_text("Search Everywhere (⇧⌘A)"), "Search Everywhere (Ctrl+Shift+A)");
        assert_eq!(linux_text("Run 'App' (⌃R)"), "Run 'App' (Shift+F10)");
        assert_eq!(linux_text("Rerun (⌘R)"), "Rerun (Ctrl+F5)");
        assert_eq!(linux_text("Stop (⌘F2)"), "Stop (Ctrl+F2)");
        assert_eq!(linux_text("Pause Program (⌥⌘F9)"), "Pause Program");
        assert_eq!(linux_text("Commits  ⌘↩"), "Commits  Ctrl+Enter");
        assert_eq!(linux_text("⌃⌥⇧↓ next"), "Ctrl+Alt+Shift+Down next");
        assert_eq!(linux_text("Hierarchy (⌃H, ⌃⌥H)"), "Hierarchy (Ctrl+H, Ctrl+Alt+H)");
        assert_eq!(linux_text("Move to the Trash (⌘⌫)"), "Move to the Trash (Delete)");
        assert_eq!(linux_text("Find References (⇧F12) lists them here"), "Find References (Shift+F12) lists them here");
        assert_eq!(linux_text("Press ⌘. for quick fixes."), "Press Ctrl+. for quick fixes.");
        assert_eq!(linux_text("No glyphs here"), "No glyphs here");
    }
}

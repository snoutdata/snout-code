//! The menu bar, as data. On macOS it becomes the native menu bar; elsewhere it is drawn
//! above the toolbar. Either way an item is a [`Command`], run by [`SnoutApp::run_command`]
//! through the same actions as the buttons and shortcuts, and greyed out when they would be.
//!
//! A native item that owns a shortcut takes the key before egui sees it, so the Edit items
//! hand egui the event the key would have made (copy, paste, ⌘Z …), and ⌘⌫ and ⌘↩ are left
//! off the menu: text fields and the console need them.

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use snout_core::{FileEntry, PullMode};

use super::dialogs::Dialog;
use super::{Action, Scope, SnoutApp, on_disk, stage_paths, unstage_paths};
use crate::theme::ThemeChoice;

pub(super) const SOURCE_URL: &str = "https://github.com/snoutdata/snout-code";
pub(super) const ISSUES_URL: &str = "https://github.com/snoutdata/snout-code/issues";
pub(super) const WEBSITE_URL: &str = "https://snoutdata.com";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Command {
    About,
    Quit,
    Open,
    FindRepos,
    Clone,
    NewGroup,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Console,
    SideBySide,
    Unified,
    Compact,
    Theme(ThemeChoice),
    Refresh,
    Pull,
    Fetch,
    Push,
    Commit,
    Draft,
    Stage,
    Unstage,
    Discard,
    Trash,
    Checkout,
    NewBranch,
    Github,
    Reveal,
    Minimize,
    Maximize,
    FullScreen,
    Source,
    Issues,
    Website,
}

/// An item macOS performs itself.
#[derive(Clone, Copy)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) enum System {
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    Zoom,
    FullScreen,
    BringAllToFront,
}

pub(super) enum Entry {
    Item {
        command: Command,
        label: &'static str,
        keys: Option<KeyboardShortcut>,
    },
    Separator,
    Submenu(&'static str, Vec<Entry>),
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    System(System),
}

pub(super) struct Menu {
    pub title: &'static str,
    pub entries: Vec<Entry>,
}

fn item(command: Command, label: &'static str) -> Entry {
    Entry::Item { command, label, keys: None }
}

/// An item with ⌘ (Ctrl elsewhere) and `key`, plus Shift when `shift`.
fn keyed(command: Command, label: &'static str, shift: bool, key: Key) -> Entry {
    let modifiers = if shift { Modifiers::COMMAND | Modifiers::SHIFT } else { Modifiers::COMMAND };
    Entry::Item { command, label, keys: Some(KeyboardShortcut::new(modifiers, key)) }
}

/// The whole bar. `mac` adds the application and Window menus macOS expects and leaves out
/// what they hold elsewhere.
pub(super) fn bar(mac: bool) -> Vec<Menu> {
    use Command as C;
    let mut menus = Vec::new();
    if mac {
        menus.push(Menu {
            title: "SnoutCode",
            entries: vec![
                item(C::About, "About SnoutCode"),
                Entry::Separator,
                Entry::System(System::Services),
                Entry::Separator,
                Entry::System(System::Hide),
                Entry::System(System::HideOthers),
                Entry::System(System::ShowAll),
                Entry::Separator,
                keyed(C::Quit, "Quit SnoutCode", false, Key::Q),
            ],
        });
    }
    let mut file = vec![
        keyed(C::Open, "Open Local Repository…", false, Key::O),
        item(C::FindRepos, "Find Repositories in Folder…"),
        item(C::Clone, "Clone from GitHub…"),
        Entry::Separator,
        item(C::NewGroup, "New Group…"),
    ];
    if !mac {
        file.extend([Entry::Separator, keyed(C::Quit, "Quit", false, Key::Q)]);
    }
    menus.push(Menu { title: "File", entries: file });
    menus.push(Menu {
        title: "Edit",
        entries: vec![
            keyed(C::Undo, "Undo", false, Key::Z),
            keyed(C::Redo, "Redo", true, Key::Z),
            Entry::Separator,
            keyed(C::Cut, "Cut", false, Key::X),
            keyed(C::Copy, "Copy", false, Key::C),
            keyed(C::Paste, "Paste", false, Key::V),
            keyed(C::SelectAll, "Select All", false, Key::A),
        ],
    });
    let mut view = vec![
        keyed(C::Console, "Claude Code Console", false, Key::J),
        Entry::Separator,
        item(C::SideBySide, "Side by Side Diff"),
        item(C::Unified, "Unified Diff"),
        item(C::Compact, "Compact Diff"),
        Entry::Separator,
        Entry::Submenu("Appearance", ThemeChoice::ALL.into_iter().map(|t| item(C::Theme(t), t.label())).collect()),
        Entry::Separator,
        keyed(C::Refresh, "Refresh", false, Key::R),
    ];
    if mac {
        view.extend([Entry::Separator, Entry::System(System::FullScreen)]);
    }
    menus.push(Menu { title: "View", entries: view });
    menus.push(Menu {
        title: "Repository",
        entries: vec![
            item(C::Pull, "Pull"),
            item(C::Fetch, "Fetch"),
            item(C::Push, "Push"),
            Entry::Separator,
            item(C::Commit, "Commit"),
            item(C::Draft, "Draft Commit Message"),
            Entry::Separator,
            item(C::Stage, "Stage"),
            item(C::Unstage, "Unstage"),
            item(C::Discard, "Discard Changes…"),
            item(C::Trash, "Move to Trash…"),
            Entry::Separator,
            keyed(C::Checkout, "Check Out…", true, Key::O),
            item(C::NewBranch, "New Branch…"),
            Entry::Separator,
            item(C::Github, "Open on GitHub"),
            item(C::Reveal, "Reveal in Finder"),
        ],
    });
    menus.push(Menu {
        title: "Window",
        entries: if mac {
            vec![
                Entry::System(System::Minimize),
                Entry::System(System::Zoom),
                Entry::Separator,
                Entry::System(System::BringAllToFront),
            ]
        } else {
            vec![item(C::Minimize, "Minimize"), item(C::Maximize, "Maximize"), item(C::FullScreen, "Full Screen")]
        },
    });
    let mut help = vec![
        item(C::Source, "SnoutCode on GitHub"),
        item(C::Issues, "Report an Issue"),
        item(C::Website, "SnoutData Website"),
    ];
    if !mac {
        help.extend([Entry::Separator, item(C::About, "About SnoutCode")]);
    }
    menus.push(Menu { title: "Help", entries: help });
    menus
}

impl SnoutApp {
    /// Whether the command would do anything now; its menu item is greyed out when not.
    pub(super) fn command_enabled(&self, command: Command) -> bool {
        use Command as C;
        let has_repo = self.repo.is_some();
        // Like the shortcuts, nothing that opens or changes a repository while a dialog is up.
        let free = self.dialog.is_none();
        let entries = || self.selected_entries();
        let in_tree = self.commit_view.is_none();
        match command {
            C::About | C::Quit | C::Undo | C::Redo | C::Cut | C::Copy | C::Paste | C::SelectAll => true,
            C::Minimize | C::Maximize | C::FullScreen | C::Source | C::Issues | C::Website => true,
            C::SideBySide | C::Unified | C::Compact | C::Theme(_) | C::Console => true,
            C::Open | C::FindRepos | C::Clone | C::NewGroup => free,
            C::Refresh | C::Checkout | C::NewBranch | C::Reveal => free && has_repo,
            C::Pull | C::Fetch | C::Push => free && has_repo && self.job.is_none(),
            C::Commit => free && self.can_commit(),
            C::Draft => free && has_repo && self.draft.is_none() && self.commit_scope() != Scope::Nothing,
            C::Stage => free && in_tree && entries().iter().any(FileEntry::has_unstaged),
            C::Unstage => free && in_tree && entries().iter().any(FileEntry::has_staged),
            C::Discard => free && in_tree && !entries().is_empty(),
            C::Trash => free && in_tree && entries().iter().any(on_disk),
            C::Github => self.github_url.is_some(),
        }
    }

    /// For the items that carry a check mark, whether it shows.
    pub(super) fn command_checked(&self, command: Command) -> Option<bool> {
        Some(match command {
            Command::Console => self.settings.console_open,
            Command::SideBySide => self.settings.side_by_side,
            Command::Unified => !self.settings.side_by_side,
            Command::Compact => self.settings.compact,
            Command::Theme(t) => self.settings.theme == t,
            _ => return None,
        })
    }

    pub(super) fn run_command(&mut self, command: Command, ctx: &egui::Context, actions: &mut Vec<Action>) {
        use Command as C;
        if !self.command_enabled(command) {
            return;
        }
        let entries = self.selected_entries();
        match command {
            C::About => self.dialog = Some(Dialog::About),
            C::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            C::Open => actions.push(Action::PickFolder),
            C::FindRepos => actions.push(Action::PickScanFolder),
            C::Clone => actions.push(Action::ShowClone),
            C::NewGroup => actions.push(Action::ShowNewGroup(None)),
            C::Undo => press(ctx, Modifiers::COMMAND, Key::Z),
            C::Redo => press(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
            C::Cut => ctx.input_mut(|i| i.events.push(egui::Event::Cut)),
            C::Copy => ctx.input_mut(|i| i.events.push(egui::Event::Copy)),
            C::Paste => {
                if let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                    ctx.input_mut(|i| i.events.push(egui::Event::Paste(text)));
                }
            }
            C::SelectAll => press(ctx, Modifiers::COMMAND, Key::A),
            C::Console => actions.push(Action::ToggleConsole),
            C::SideBySide => self.settings.side_by_side = true,
            C::Unified => self.settings.side_by_side = false,
            C::Compact => self.settings.compact = !self.settings.compact,
            C::Theme(t) => actions.push(Action::SetTheme(t)),
            C::Refresh => actions.push(Action::Refresh),
            C::Pull => actions.push(Action::Pull(PullMode::Default)),
            C::Fetch => actions.push(Action::Fetch),
            C::Push => actions.push(Action::Push),
            C::Commit => actions.push(Action::Commit),
            C::Draft => actions.push(Action::DraftMessage),
            C::Stage => actions.push(Action::Stage(stage_paths(&entries))),
            C::Unstage => actions.push(Action::Unstage(unstage_paths(&entries))),
            C::Discard => actions.push(Action::AskDiscard(entries)),
            C::Trash => actions.push(Action::AskDeleteFiles(entries)),
            C::Checkout => actions.push(Action::ShowCheckout),
            C::NewBranch => actions.push(Action::ShowNewBranch(None)),
            C::Github => actions.push(Action::OpenGithub),
            C::Reveal => {
                if let Some(repo) = &self.repo {
                    actions.push(Action::Reveal(repo.root().to_owned()));
                }
            }
            C::Minimize => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            C::Maximize => {
                let on = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!on));
            }
            C::FullScreen => {
                let on = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!on));
            }
            C::Source => ctx.open_url(egui::OpenUrl::new_tab(SOURCE_URL)),
            C::Issues => ctx.open_url(egui::OpenUrl::new_tab(ISSUES_URL)),
            C::Website => ctx.open_url(egui::OpenUrl::new_tab(WEBSITE_URL)),
        }
    }

    /// The bar drawn in the window, where there is no native one.
    pub(super) fn menu_bar_ui(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let mut chosen = None;
        egui::MenuBar::new().ui(ui, |ui| {
            for menu in bar(false) {
                ui.menu_button(menu.title, |ui| self.drawn_entries(ui, &menu.entries, &mut chosen));
            }
        });
        if let Some(command) = chosen {
            self.run_command(command, ui.ctx(), actions);
        }
    }

    fn drawn_entries(&self, ui: &mut egui::Ui, entries: &[Entry], chosen: &mut Option<Command>) {
        for entry in entries {
            match entry {
                Entry::Item { command, label, keys } => {
                    let mut button = match self.command_checked(*command) {
                        Some(on) => egui::Button::selectable(on, *label),
                        None => egui::Button::new(*label),
                    };
                    if let Some(keys) = keys {
                        button = button.shortcut_text(ui.ctx().format_shortcut(keys));
                    }
                    if ui.add_enabled(self.command_enabled(*command), button).clicked() {
                        *chosen = Some(*command);
                    }
                }
                Entry::Separator => {
                    ui.separator();
                }
                Entry::Submenu(title, entries) => {
                    ui.menu_button(*title, |ui| self.drawn_entries(ui, entries, chosen));
                }
                Entry::System(_) => {}
            }
        }
    }

    /// Run what was picked in the native menu since the last frame, and bring its items up
    /// to date.
    pub(super) fn native_menu(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if let Some(native) = &self.native {
            self.picked.extend(native.picked());
        }
        for command in std::mem::take(&mut self.picked) {
            self.run_command(command, ctx, actions);
        }
        if let Some(native) = &self.native {
            native.sync(|c| (self.command_enabled(c), self.command_checked(c)));
        }
    }
}

/// Hand egui a key press, as if the key had reached it.
fn press(ctx: &egui::Context, modifiers: Modifiers, key: Key) {
    ctx.input_mut(|i| {
        // Some handlers read the held modifiers rather than the event's.
        i.modifiers = modifiers;
        i.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    });
}

#[cfg(target_os = "macos")]
pub(super) use native::Native;

#[cfg(not(target_os = "macos"))]
pub(super) struct Native;

#[cfg(not(target_os = "macos"))]
impl Native {
    pub fn install(_ctx: &egui::Context) -> Option<Self> {
        None
    }

    fn picked(&self) -> Vec<Command> {
        Vec::new()
    }

    fn sync(&self, _state: impl Fn(Command) -> (bool, Option<bool>)) {}
}

#[cfg(target_os = "macos")]
mod native {
    use std::cell::RefCell;
    use std::sync::mpsc;

    use eframe::egui::{self, KeyboardShortcut, Modifiers};
    use muda::{CheckMenuItem, IsMenuItem, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

    use super::{Command, Entry, System, bar};

    enum Item {
        Plain(MenuItem),
        Check(CheckMenuItem),
    }

    /// The native menu bar. Item ids are indices into `items`.
    pub struct Native {
        _menu: muda::Menu,
        items: Vec<(Command, Item)>,
        /// What each item last showed, so AppKit is only told about changes.
        shown: RefCell<Vec<(bool, Option<bool>)>>,
        rx: mpsc::Receiver<usize>,
    }

    impl Native {
        /// Build the bar and make it the application's. Call once the event loop runs.
        pub fn install(ctx: &egui::Context) -> Option<Self> {
            // One window, so no tabs, and none of their items in the View menu.
            let mtm = objc2::MainThreadMarker::new()?;
            objc2_app_kit::NSWindow::setAllowsAutomaticWindowTabbing(false, mtm);
            let menu = muda::Menu::new();
            let mut items = Vec::new();
            for m in bar(true) {
                let sub = Submenu::new(m.title, true);
                append(&sub, &m.entries, &mut items);
                menu.append(&sub).ok()?;
                match m.title {
                    "Window" => sub.set_as_windows_menu_for_nsapp(),
                    "Help" => sub.set_as_help_menu_for_nsapp(),
                    _ => {}
                }
            }
            menu.init_for_nsapp();
            let (tx, rx) = mpsc::channel();
            let ctx = ctx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                if let Ok(i) = e.id.0.parse() {
                    let _ = tx.send(i);
                    ctx.request_repaint();
                }
            }));
            let shown = RefCell::new(vec![(true, None); items.len()]);
            Some(Self { _menu: menu, items, shown, rx })
        }

        pub(super) fn picked(&self) -> Vec<Command> {
            self.rx.try_iter().filter_map(|i| self.items.get(i).map(|(c, _)| *c)).collect()
        }

        pub(super) fn sync(&self, state: impl Fn(Command) -> (bool, Option<bool>)) {
            let mut shown = self.shown.borrow_mut();
            for ((command, item), was) in self.items.iter().zip(shown.iter_mut()) {
                let now = state(*command);
                if now == *was {
                    continue;
                }
                match item {
                    Item::Plain(i) => i.set_enabled(now.0),
                    Item::Check(i) => {
                        i.set_enabled(now.0);
                        i.set_checked(now.1.unwrap_or(false));
                    }
                }
                *was = now;
            }
        }
    }

    fn append(sub: &Submenu, entries: &[Entry], items: &mut Vec<(Command, Item)>) {
        for entry in entries {
            let added: Box<dyn IsMenuItem> = match entry {
                Entry::Item { command, label, keys } => {
                    let id = items.len().to_string();
                    let keys = keys.and_then(accelerator);
                    // Check items start unchecked; the first sync sets them.
                    let item = if matches!(
                        command,
                        Command::Console
                            | Command::SideBySide
                            | Command::Unified
                            | Command::Compact
                            | Command::Theme(_)
                    ) {
                        Item::Check(CheckMenuItem::with_id(id, label, true, false, keys))
                    } else {
                        Item::Plain(MenuItem::with_id(id, label, true, keys))
                    };
                    let boxed: Box<dyn IsMenuItem> = match &item {
                        Item::Plain(i) => Box::new(i.clone()),
                        Item::Check(i) => Box::new(i.clone()),
                    };
                    items.push((*command, item));
                    boxed
                }
                Entry::Separator => Box::new(PredefinedMenuItem::separator()),
                Entry::Submenu(title, entries) => {
                    let inner = Submenu::new(*title, true);
                    append(&inner, entries, items);
                    Box::new(inner)
                }
                Entry::System(s) => Box::new(match s {
                    System::Services => PredefinedMenuItem::services(None),
                    System::Hide => PredefinedMenuItem::hide(Some("Hide SnoutCode")),
                    System::HideOthers => PredefinedMenuItem::hide_others(None),
                    System::ShowAll => PredefinedMenuItem::show_all(None),
                    System::Minimize => PredefinedMenuItem::minimize(None),
                    System::Zoom => PredefinedMenuItem::maximize(Some("Zoom")),
                    System::FullScreen => PredefinedMenuItem::fullscreen(None),
                    System::BringAllToFront => PredefinedMenuItem::bring_all_to_front(None),
                }),
            };
            let _ = sub.append(added.as_ref());
        }
    }

    /// `⌘⇧O` as muda spells it.
    pub(super) fn accelerator(keys: KeyboardShortcut) -> Option<muda::accelerator::Accelerator> {
        let shift = if keys.modifiers.contains(Modifiers::SHIFT) { "Shift+" } else { "" };
        format!("CmdOrCtrl+{shift}{}", keys.logical_key.name()).parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shortcut on the bar is one muda can register, and none is taken twice.
    #[test]
    fn shortcuts_are_valid_and_distinct() {
        let mut seen = Vec::new();
        fn walk(entries: &[Entry], seen: &mut Vec<KeyboardShortcut>) {
            for e in entries {
                match e {
                    Entry::Item { keys: Some(k), label, .. } => {
                        #[cfg(target_os = "macos")]
                        assert!(native::accelerator(*k).is_some(), "{label}");
                        assert!(!seen.contains(k), "{label}");
                        seen.push(*k);
                    }
                    Entry::Submenu(_, entries) => walk(entries, seen),
                    _ => {}
                }
            }
        }
        for mac in [true, false] {
            seen.clear();
            for m in bar(mac) {
                walk(&m.entries, &mut seen);
            }
        }
    }
}

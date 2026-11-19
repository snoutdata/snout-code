//! Navigate ▸ Back and Forward (⌥⌘← and ⌥⌘→, Ctrl+Alt+← and → elsewhere, and the mouse's
//! side buttons): the places jumps left, across files, to go back to and forth again.
//!
//! A jump notes where the caret was before it: going to a declaration, a usage or an
//! implementation, opening a place a search or a tool window lists, Go to Line. Going back
//! notes where the caret is now, for Forward; a new jump clears what Forward had.

use std::path::PathBuf;

use gpui::{App, Context, Entity, Window};
use gpui_component::dock::PanelId;
use gpui_component::input::Position;
use lsp_types::Uri;

use super::Workspace;
use crate::editor::EditorPanel;

/// How many places Back keeps.
const KEEP: usize = 100;

/// A place in a file (or a library's class), the caret's line and column as the editor
/// counts them.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub path: PathBuf,
    /// A class file's tab: its URI and the project the server is for.
    pub class: Option<(Uri, PathBuf)>,
    pub at: Position,
}

impl Place {
    /// Whether two places are the same for going back: the same file and line.
    fn same(&self, other: &Place) -> bool {
        self.path == other.path && self.class == other.class && self.at.line == other.at.line
    }
}

/// The places to go back to, and forth again.
#[derive(Default)]
pub struct NavHistory {
    back: Vec<Place>,
    forward: Vec<Place>,
    /// Going back or forth: the jump that does it is not a new one.
    replaying: bool,
}

impl NavHistory {
    /// A jump left `from`.
    pub fn note(&mut self, from: Place) {
        if self.replaying {
            return;
        }
        if !self.back.last().is_some_and(|p| p.same(&from)) {
            self.back.push(from);
            if self.back.len() > KEEP {
                self.back.remove(0);
            }
        }
        self.forward.clear();
    }

    /// The place to go back to, from `here` (kept for Forward).
    pub fn back(&mut self, here: Option<Place>) -> Option<Place> {
        step(&mut self.back, &mut self.forward, here)
    }

    /// The place to go forth to again, from `here` (kept for Back).
    pub fn forward(&mut self, here: Option<Place>) -> Option<Place> {
        step(&mut self.forward, &mut self.back, here)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

/// Take the last place of `from` that is not `here`, keeping `here` on `to`.
fn step(from: &mut Vec<Place>, to: &mut Vec<Place>, here: Option<Place>) -> Option<Place> {
    while let Some(place) = from.pop() {
        if here.as_ref().is_some_and(|h| h.same(&place)) {
            continue;
        }
        if let Some(here) = here
            && !to.last().is_some_and(|p| p.same(&here))
        {
            to.push(here);
        }
        return Some(place);
    }
    None
}

/// Where `editor` is at `at` (its caret when none), unless it shows no file.
pub fn place_of(editor: &Entity<EditorPanel>, at: Option<Position>, cx: &App) -> Option<Place> {
    let e = editor.read(cx);
    if e.path.as_os_str().is_empty() && e.class_file.is_none() {
        return None;
    }
    let at = at.unwrap_or_else(|| e.state.read(cx).cursor_position());
    Some(Place { path: e.path.clone(), class: e.class_file.clone(), at })
}

impl Workspace {
    /// A jump is about to leave the editor in front: note where its caret is.
    pub(super) fn note_jump(&mut self, window: &Window, cx: &App) {
        if let Some(place) = self.active_editor(window, cx).and_then(|e| place_of(&e, None, cx)) {
            self.history.note(place);
        }
    }

    /// Navigate ▸ Back.
    pub fn navigate_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let here = self.active_editor(window, cx).and_then(|e| place_of(&e, None, cx));
        if let Some(place) = self.history.back(here) {
            self.show_place(place, window, cx);
        }
    }

    /// Navigate ▸ Forward.
    pub fn navigate_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let here = self.active_editor(window, cx).and_then(|e| place_of(&e, None, cx));
        if let Some(place) = self.history.forward(here) {
            self.show_place(place, window, cx);
        }
    }

    /// Go to `place`: its tab if one shows it, else the file opened again.
    fn show_place(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
        self.history.replaying = true;
        let open = self.editors(cx).into_iter().find(|e| {
            let e = e.read(cx);
            match &place.class {
                Some((uri, _)) => e.class_file.as_ref().is_some_and(|(u, _)| u == uri),
                None => e.class_file.is_none() && e.path == place.path,
            }
        });
        match (open, &place.class) {
            (Some(editor), _) => {
                self.dock.update(cx, |d, cx| d.select_panel(PanelId::from(editor.entity_id()), window, cx));
                self.set_active_editor(Some(&editor), cx);
                editor.update(cx, |e, cx| e.go_to(place.at.line, place.at.character, window, cx));
            }
            (None, None) => {
                self.open_file(place.path.clone(), Some((place.at.line, place.at.character)), false, window, cx)
            }
            // A class file's columns are its text's: ASCII, the same in either count.
            (None, Some((uri, root))) => {
                let at = lsp_types::Position::new(place.at.line, place.at.character);
                self.open_location(root.clone(), uri.clone(), at, window, cx);
            }
        }
        self.history.replaying = false;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(file: &str, line: u32) -> Place {
        Place { path: PathBuf::from(file), class: None, at: Position::new(line, 4) }
    }

    #[test]
    fn back_and_forth_through_the_places_jumps_left() {
        let mut h = NavHistory::default();
        // A.java:10 → B.java:3 → B.java:40 → C.java:7 (here).
        h.note(at("A.java", 10));
        h.note(at("B.java", 3));
        h.note(at("B.java", 40));
        assert_eq!(h.back(Some(at("C.java", 7))), Some(at("B.java", 40)));
        assert_eq!(h.back(Some(at("B.java", 40))), Some(at("B.java", 3)));
        assert!(h.can_go_forward());
        assert_eq!(h.forward(Some(at("B.java", 3))), Some(at("B.java", 40)));
        assert_eq!(h.forward(Some(at("B.java", 40))), Some(at("C.java", 7)));
        assert_eq!(h.forward(Some(at("C.java", 7))), None);
        // A new jump from the middle clears Forward.
        h.back(Some(at("C.java", 7)));
        h.note(at("B.java", 40));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn the_same_line_twice_is_one_place() {
        let mut h = NavHistory::default();
        h.note(at("A.java", 10));
        h.note(Place { at: Position::new(10, 0), ..at("A.java", 10) });
        // Back from the place itself skips it.
        assert_eq!(h.back(Some(at("A.java", 10))), None);
        let mut h = NavHistory::default();
        h.note(at("A.java", 10));
        h.replaying = true;
        h.note(at("B.java", 1));
        h.replaying = false;
        assert_eq!(h.back(None), Some(at("A.java", 10)));
        assert!(!h.can_go_back());
    }
}

//! One suite for every engine this build has: each test runs against each of them.

use super::*;

const FG: Rgb = (220, 220, 220);
const BG: Rgb = (20, 20, 30);

fn each(test: impl Fn(&mut dyn Engine)) {
    let engines = EngineKind::available();
    assert!(engines.contains(&EngineKind::Builtin));
    assert_eq!(engines.contains(&EngineKind::Ghostty), cfg!(feature = "ghostty"));
    for kind in engines {
        let mut engine = kind.create(24, 80, FG, BG).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert_eq!(engine.kind(), kind);
        test(&mut *engine);
    }
}

fn frame(e: &mut dyn Engine) -> Frame {
    let mut f = Frame::default();
    e.frame(&mut f);
    f
}

fn cell(f: &Frame, row: u16, col: u16) -> Cell {
    *f.cell(row, col).unwrap_or_else(|| panic!("no cell {row},{col} in {:?}", f.engine))
}

#[test]
fn parses_text_and_moves_the_cursor() {
    each(|e| {
        e.write(b"hello\r\nworld\x1b[3;5Hx");
        let f = frame(e);
        let kind = f.engine;
        assert_eq!(f.size(), (24, 80), "{kind:?}");
        assert_eq!(f.contents(), "hello\nworld\n    x", "{kind:?}");
        assert_eq!(f.text(&cell(&f, 1, 4)), "d");
        assert_eq!(f.text(&cell(&f, 1, 5)), "", "{kind:?}: a blank cell has no text");
        let cursor = f.cursor.expect("the cursor shows");
        assert_eq!((cursor.row, cursor.col), (2, 5), "{kind:?}");
        // Erase in line, carriage return, backspace.
        e.write(b"\x1b[1;1H\x1b[2Kbye\x08\x08Y");
        assert_eq!(frame(e).row_text(0).trim_end(), "bYe", "{kind:?}");
        e.write(b"\x1b[?25l");
        assert!(frame(e).cursor.is_none(), "{kind:?}: the cursor hides");
    });
}

#[test]
fn colors_and_attributes() {
    each(|e| {
        e.write(b"\x1b[31mr\x1b[1;32mg\x1b[0m\x1b[38;5;200mi\x1b[38;2;1;2;3mt\x1b[0m");
        e.write(b"\x1b[48;5;4mb\x1b[0m\x1b[7mv\x1b[0m\x1b[2md\x1b[0m\x1b[3mi\x1b[0m\x1b[4mu\x1b[0mn");
        let f = frame(e);
        let kind = f.engine;
        let c = |col| cell(&f, 0, col);
        assert_eq!(c(0).fg, Color::Idx(1), "{kind:?}");
        assert_eq!((c(1).fg, c(1).bold()), (Color::Idx(2), true), "{kind:?}");
        assert_eq!(c(2).fg, Color::Idx(200), "{kind:?}");
        assert_eq!(c(3).fg, Color::Rgb(1, 2, 3), "{kind:?}");
        assert_eq!((c(4).fg, c(4).bg), (Color::Default, Color::Idx(4)), "{kind:?}");
        assert!(c(5).attrs.contains(Attrs::INVERSE), "{kind:?}");
        assert!(c(6).attrs.contains(Attrs::DIM), "{kind:?}");
        assert!(c(7).attrs.contains(Attrs::ITALIC), "{kind:?}");
        assert_eq!(c(8).underline, Underline::Single, "{kind:?}");
        assert_eq!((c(9).fg, c(9).bg, c(9).attrs, c(9).underline), Default::default(), "{kind:?}: reset");
        // A cleared line keeps the background colour it was cleared with.
        e.write(b"\r\n\x1b[41m\x1b[K\x1b[0m");
        assert_eq!(cell(&frame(e), 1, 40).bg, Color::Idx(1), "{kind:?}");
    });
}

#[test]
fn wide_characters_take_two_cells() {
    each(|e| {
        e.write("中a".as_bytes());
        let f = frame(e);
        let kind = f.engine;
        assert_eq!((f.text(&cell(&f, 0, 0)), cell(&f, 0, 0).width), ("中", Width::Wide), "{kind:?}");
        assert_eq!(cell(&f, 0, 1).width, Width::Continuation, "{kind:?}");
        assert_eq!(f.text(&cell(&f, 0, 2)), "a", "{kind:?}");
        assert_eq!(f.row_text(0).trim_end(), "中a", "{kind:?}");
    });
}

#[test]
fn the_alternate_screen_comes_and_goes_like_vim_and_less() {
    each(|e| {
        e.write(b"shell prompt $ ");
        // What vim and less send: save the screen, application keys, clear, draw, scroll region.
        e.write(b"\x1b[?1049h\x1b[?1h\x1b=\x1b[H\x1b[2J~ file.txt\x1b[2;23r\x1b[24;1H-- INSERT --");
        let f = frame(e);
        let kind = f.engine;
        assert!(f.modes.alt_screen && f.modes.app_cursor, "{kind:?}: {:?}", f.modes);
        assert!(f.contents().starts_with("~ file.txt"), "{kind:?}: {}", f.contents());
        assert!(!f.contents().contains("prompt"), "{kind:?}");
        assert_eq!(f.row_text(23).trim_end(), "-- INSERT --", "{kind:?}");
        // Scrolling inside the region leaves the rows outside it.
        e.write(b"\x1b[2;1Hline two\x1b[23;1H\n");
        let f = frame(e);
        assert_eq!(f.row_text(0).trim_end(), "~ file.txt", "{kind:?}");
        assert_eq!(f.row_text(23).trim_end(), "-- INSERT --", "{kind:?}");
        assert!(!f.contents().contains("line two"), "{kind:?}: scrolled out of the region");
        // Leaving restores the shell's screen, and the alternate screen kept no history.
        e.write(b"\x1b[r\x1b[?1l\x1b>\x1b[?1049l");
        let f = frame(e);
        assert!(!f.modes.alt_screen && !f.modes.app_cursor, "{kind:?}");
        assert_eq!(f.contents(), "shell prompt $", "{kind:?}");
        assert_eq!(f.history, 0, "{kind:?}");
    });
}

#[test]
fn resizes_keeping_what_fits() {
    each(|e| {
        e.write(b"abcdefghij\r\nsecond");
        e.resize(10, 5);
        let f = frame(e);
        let kind = f.engine;
        assert_eq!(f.size(), (10, 5), "{kind:?}");
        assert!(f.contents().starts_with("abcde"), "{kind:?}: {:?}", f.contents());
        e.resize(30, 100);
        let f = frame(e);
        assert_eq!(f.size(), (30, 100), "{kind:?}");
        assert!(f.contents().contains("abcde"), "{kind:?}: {:?}", f.contents());
        assert!(f.cell(29, 99).is_some() && f.cell(30, 0).is_none(), "{kind:?}");
    });
}

#[test]
fn reflows_on_resize_with_ghostty() {
    each(|e| {
        e.write(b"abcdefghij");
        e.resize(24, 5);
        let f = frame(e);
        if f.engine == EngineKind::Ghostty {
            assert_eq!(f.row_text(0), "abcde");
            assert_eq!(f.row_text(1), "fghij");
            assert!(f.wrapped(0));
            assert_eq!(f.contents(), "abcdefghij");
        }
    });
}

#[test]
fn keeps_scrollback_and_scrolls_through_it() {
    each(|e| {
        for i in 0..100 {
            e.write(format!("line {i}\r\n").as_bytes());
        }
        let kind = e.kind();
        let (at, max) = e.scrollback();
        assert_eq!(at, 0, "{kind:?}");
        assert_eq!(max, 77, "{kind:?}: 101 lines on 24 rows");
        assert_eq!(frame(e).row_text(0).trim_end(), "line 77", "{kind:?}");
        e.set_scrollback(10);
        let f = frame(e);
        assert_eq!((f.scrollback, f.history), (10, 77), "{kind:?}");
        assert_eq!(f.row_text(0).trim_end(), "line 67", "{kind:?}");
        assert!(f.cursor.is_none(), "{kind:?}: no cursor while scrolled back");
        e.set_scrollback(1000);
        assert_eq!(e.scrollback(), (77, 77), "{kind:?}: no further than the history");
        assert_eq!(frame(e).row_text(0).trim_end(), "line 0", "{kind:?}");
        let lines = e.lines();
        assert_eq!(lines.len(), 101, "{kind:?}");
        assert_eq!((lines[0].as_str(), lines[99].as_str(), lines[100].as_str()), ("line 0", "line 99", ""));
        assert_eq!(e.scrollback().0, 77, "{kind:?}: the search left the view where it was");
        e.set_scrollback(0);
        let f = frame(e);
        assert_eq!((f.scrollback, f.row_text(0).trim_end()), (0, "line 77"), "{kind:?}");
        assert!(f.cursor.is_some(), "{kind:?}");
    });
}

#[test]
fn title_and_modes() {
    each(|e| {
        e.write(b"\x1b]2;my title\x07");
        let f = frame(e);
        let kind = f.engine;
        assert_eq!(f.title, "my title", "{kind:?}");
        assert_eq!(f.modes, Modes::default(), "{kind:?}");
        e.write(b"\x1b[?2004h\x1b[?1000h\x1b[?1006h");
        let m = frame(e).modes;
        assert!(m.bracketed_paste && m.mouse && m.mouse_sgr, "{kind:?}: {m:?}");
        e.write(b"\x1b[?2004l\x1b[?1000l\x1b[?1006l");
        let m = frame(e).modes;
        assert!(!m.bracketed_paste && !m.mouse && !m.mouse_sgr, "{kind:?}: {m:?}");
    });
}

#[test]
fn answers_the_queries_tuis_send() {
    each(|e| {
        let kind = e.kind();
        let mut ask = |q: &[u8]| {
            e.write(q);
            String::from_utf8_lossy(&e.take_replies()).into_owned()
        };
        assert_eq!(ask(b"\x1b[3;7H\x1b[6n"), "\x1b[3;7R", "{kind:?}: cursor position");
        assert_eq!(ask(b"\x1b[5n"), "\x1b[0n", "{kind:?}: status");
        assert!(ask(b"\x1b[c").starts_with("\x1b[?62;"), "{kind:?}: device attributes");
        let bg = ask(b"\x1b]11;?\x1b\\");
        assert!(bg.starts_with("\x1b]11;rgb:1414/1414/1e1e"), "{kind:?}: background {bg:?}");
        assert_eq!(ask(b"plain text"), "", "{kind:?}");
    });
}

#[test]
fn ghostty_speaks_the_kitty_keyboard_protocol() {
    use crate::pty::{Key, KeyInput, Mods};
    each(|e| {
        let enter =
            KeyInput { key: Some(Key::Enter), mods: Mods { shift: true, ..Mods::default() }, ..KeyInput::default() };
        assert_eq!(e.encode_key(&enter), None, "{:?}: legacy keys until the child asks", e.kind());
        e.write(b"\x1b[>1u");
        let f = frame(e);
        if f.engine == EngineKind::Ghostty {
            assert_eq!(f.modes.kitty_keyboard, 1);
            assert_eq!(e.encode_key(&enter).as_deref(), Some(&b"\x1b[13;2u"[..]));
            let a = KeyInput { text: Some("a"), base: Some('a'), ..KeyInput::default() };
            assert_eq!(e.encode_key(&a).as_deref(), Some(&b"a"[..]));
            let ctrl_c = KeyInput {
                key: Some(Key::Char('c')),
                mods: Mods { ctrl: true, ..Mods::default() },
                ..KeyInput::default()
            };
            assert_eq!(e.encode_key(&ctrl_c).as_deref(), Some(&b"\x1b[99;5u"[..]));
            e.write(b"\x1b[<u");
            assert_eq!(e.encode_key(&enter), None);
        } else {
            assert_eq!(f.modes.kitty_keyboard, 0);
        }
    });
}

#[test]
fn ghostty_keeps_hyperlinks() {
    each(|e| {
        e.write(b"see \x1b]8;;https://example.com/docs\x1b\\the docs\x1b]8;;\x1b\\ now");
        let f = frame(e);
        if f.engine == EngineKind::Ghostty {
            assert_eq!(f.link(&cell(&f, 0, 5)), Some("https://example.com/docs"));
            assert_eq!(f.link_at(0, 4).as_deref(), Some("https://example.com/docs"));
            assert_eq!(f.link(&cell(&f, 0, 0)), None);
        }
        assert_eq!(f.contents(), "see the docs now", "{:?}", f.engine);
    });
}

#[test]
fn selection_and_links_from_the_text() {
    each(|e| {
        e.write(b"one two\r\nopen https://example.com/a(b). now\r\nthree");
        let f = frame(e);
        let kind = f.engine;
        assert_eq!(f.selected_text(((0, 4), (1, 3))).as_deref(), Some("two\nopen"), "{kind:?}");
        assert_eq!(f.selected_text(((1, 3), (0, 4))).as_deref(), Some("two\nopen"), "{kind:?}: either way round");
        assert_eq!(f.selected_text(((0, 1), (0, 1))), None, "{kind:?}");
        assert_eq!(f.link_at(1, 10).as_deref(), Some("https://example.com/a(b)"), "{kind:?}");
        assert_eq!(f.link_at(1, 2), None, "{kind:?}");
    });
}

#[test]
fn finds_text_in_rows_and_lines() {
    each(|e| {
        e.write("Error: one\r\n中 error two ERROR\r\n".as_bytes());
        let f = frame(e);
        let kind = f.engine;
        assert_eq!(f.find_in_row(0, "error"), [(0, 4)], "{kind:?}");
        assert_eq!(f.find_in_row(1, "error"), [(3, 7), (13, 17)], "{kind:?}: columns, past the wide character");
        assert!(f.find_in_row(2, "error").is_empty(), "{kind:?}");
        let lines = e.lines();
        assert_eq!(find_spans(&lines[1], "error").len(), 2, "{kind:?}");
    });
}

#[test]
fn finds_web_addresses() {
    let spans = |t: &str| url_spans(t).into_iter().map(|(s, e)| t[s..e].to_owned()).collect::<Vec<_>>();
    assert_eq!(spans("go to https://a.example/x?y=1, then"), ["https://a.example/x?y=1"]);
    assert_eq!(spans("(see http://b.example/p)"), ["http://b.example/p"]);
    assert_eq!(spans("<https://c.example> and file:///tmp/x.txt."), ["https://c.example", "file:///tmp/x.txt"]);
    assert!(spans("https:// nothing").is_empty());
}

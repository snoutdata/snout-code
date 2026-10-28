//! A console line as the Run and Build windows show it: ANSI colours and styles (SGR), and
//! the places it names made into links: `at demo.App.main(App.java:12)` in a stack trace,
//! and `/path/to/App.java:12:5` as compilers print them.

use std::path::PathBuf;

/// A colour a program asked for: one of the 256 palette colours, or an RGB one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

/// A place a line names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// A file by its path; line and column from 1 (column 0 when unknown).
    Path { path: PathBuf, line: u32, column: u32 },
    /// A stack frame: the class (`demo.App`, nested classes cut off), its file's name and
    /// the line, from 1. The file is found under the project's source folders.
    Frame { class: String, file: String, line: u32 },
}

impl Link {
    /// The file a frame's class lives in, relative to a source folder: `demo/App.java`.
    pub fn relative_file(&self) -> Option<PathBuf> {
        match self {
            Link::Frame { class, file, .. } => {
                let package = class.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
                let mut out = PathBuf::from(package.replace('.', "/"));
                out.push(file);
                Some(out)
            }
            Link::Path { .. } => None,
        }
    }

    pub fn line(&self) -> u32 {
        match self {
            Link::Path { line, .. } | Link::Frame { line, .. } => *line,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub link: Option<Link>,
}

/// Split `line` into styled spans, starting in `style` and leaving it as the line ends (a
/// colour can run on to the next line). Escape sequences other than SGR are dropped.
pub fn parse(line: &str, style: &mut Style) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut text = String::new();
    let mut chars = line.chars().peekable();
    let flush = |text: &mut String, style: Style, out: &mut Vec<Span>| {
        if !text.is_empty() {
            out.push(Span { text: std::mem::take(text), style, link: None });
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => {
                match chars.peek() {
                    Some('[') => {
                        chars.next();
                        let mut params = String::new();
                        let mut end = None;
                        for c in chars.by_ref() {
                            if ('\u{40}'..='\u{7e}').contains(&c) {
                                end = Some(c);
                                break;
                            }
                            params.push(c);
                        }
                        if end == Some('m') {
                            flush(&mut text, *style, &mut out);
                            apply_sgr(&params, style);
                        }
                    }
                    Some(']') => {
                        // OSC: up to BEL or ESC \.
                        chars.next();
                        while let Some(c) = chars.next() {
                            if c == '\u{7}' {
                                break;
                            }
                            if c == '\u{1b}' {
                                chars.next();
                                break;
                            }
                        }
                    }
                    _ => {
                        chars.next();
                    }
                }
            }
            '\r' => text.clear(),
            '\t' => text.push_str("    "),
            c if (c as u32) < 0x20 => {}
            c => text.push(c),
        }
    }
    flush(&mut text, *style, &mut out);
    out.into_iter().flat_map(link_spans).collect()
}

fn apply_sgr(params: &str, style: &mut Style) {
    let codes: Vec<u32> =
        if params.is_empty() { vec![0] } else { params.split([';', ':']).map(|p| p.parse().unwrap_or(0)).collect() };
    let mut i = 0;
    while i < codes.len() {
        let code = codes[i];
        match code {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            30..=37 => style.fg = Some(Color::Indexed((code - 30) as u8)),
            39 => style.fg = None,
            40..=47 => style.bg = Some(Color::Indexed((code - 40) as u8)),
            49 => style.bg = None,
            90..=97 => style.fg = Some(Color::Indexed((code - 90 + 8) as u8)),
            100..=107 => style.bg = Some(Color::Indexed((code - 100 + 8) as u8)),
            38 | 48 => {
                let color = match codes.get(i + 1) {
                    Some(5) => {
                        let c = codes.get(i + 2).copied().unwrap_or(0).min(255) as u8;
                        i += 2;
                        Some(Color::Indexed(c))
                    }
                    Some(2) => {
                        let at = |k: usize| codes.get(i + k).copied().unwrap_or(0).min(255) as u8;
                        let c = Color::Rgb(at(2), at(3), at(4));
                        i += 4;
                        Some(c)
                    }
                    _ => None,
                };
                if code == 38 {
                    style.fg = color;
                } else {
                    style.bg = color;
                }
            }
            _ => {}
        }
        i += 1;
    }
}

/// Cut `span` where it names places, each place a span of its own with its link.
fn link_spans(span: Span) -> Vec<Span> {
    let links = find_links(&span.text);
    if links.is_empty() {
        return vec![span];
    }
    let mut out = Vec::new();
    let mut at = 0;
    for (range, link) in links {
        if range.start > at {
            out.push(Span { text: span.text[at..range.start].to_owned(), style: span.style, link: None });
        }
        out.push(Span { text: span.text[range.clone()].to_owned(), style: span.style, link: Some(link) });
        at = range.end;
    }
    if at < span.text.len() {
        out.push(Span { text: span.text[at..].to_owned(), style: span.style, link: None });
    }
    out
}

/// The places `text` names, by byte range, in order.
pub fn find_links(text: &str) -> Vec<(std::ops::Range<usize>, Link)> {
    let mut out = Vec::new();
    // Stack frames: `pkg.Class.method(File.java:12)`.
    let mut from = 0;
    while let Some(open) = text[from..].find('(').map(|i| i + from) {
        let Some(close) = text[open..].find(')').map(|i| i + open) else { break };
        let inner = &text[open + 1..close];
        if let Some((file, line)) = inner.rsplit_once(':')
            && let Ok(line) = line.parse::<u32>()
            && file.contains('.')
            && !file.contains(['/', ' '])
        {
            let head = &text[..open];
            let start = head.rfind(|c: char| c.is_whitespace()).map_or(0, |i| i + 1);
            // `java.base/java.lang.Thread.run`, `app//demo.App.main`: the module goes.
            let qualified = head[start..].rsplit('/').next().unwrap_or_default();
            if let Some((class, _method)) = qualified.rsplit_once('.') {
                let class = class.split('$').next().unwrap_or(class).to_owned();
                if !class.is_empty() {
                    out.push((open + 1..close, Link::Frame { class, file: file.to_owned(), line }));
                }
            }
        }
        from = close + 1;
    }
    // Paths: `/abs/File.java:12` or `/abs/File.java:12:5`, as a word of their own.
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        let starts = bytes[i] == b'/' && (i == 0 || matches!(bytes[i - 1], b' ' | b'\t' | b'(' | b'[' | b'\'' | b'"'));
        if !starts {
            i += 1;
            continue;
        }
        let end = text[i..].find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '\'' || c == '"');
        let end = end.map_or(text.len(), |e| e + i);
        let word = &text[i..end];
        if let Some((range, link)) = path_link(word) {
            let range = range.start + i..range.end + i;
            if !out.iter().any(|(r, _)| r.start < range.end && range.start < r.end) {
                out.push((range, link));
            }
        }
        i = end.max(i + 1);
    }
    out.sort_by_key(|(r, _)| r.start);
    out
}

/// `/a/B.java:12:5:` → the path with its line and column, and the part that names them.
fn path_link(word: &str) -> Option<(std::ops::Range<usize>, Link)> {
    let mut parts = word.split(':');
    let path = parts.next()?;
    let file = path.rsplit('/').next()?;
    if !file.contains('.') || file.starts_with('.') {
        return None;
    }
    let line: u32 = parts.next()?.parse().ok()?;
    let column_text = parts.next().filter(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_digit()));
    let column: u32 = column_text.and_then(|c| c.parse().ok()).unwrap_or(0);
    let len = path.len() + 1 + line.to_string().len() + column_text.map_or(0, |c| c.len() + 1);
    Some((0..len, Link::Path { path: PathBuf::from(path), line, column }))
}

/// The xterm palette's first 16 colours as hue, saturation and lightness, tuned for the
/// dark and the light theme.
pub fn base16(index: u8, dark: bool) -> (f32, f32, f32) {
    let (h, s, l): (f32, f32, f32) = match index % 8 {
        0 => (0., 0., if dark { 0.35 } else { 0.2 }),
        1 => (0.0, 0.7, 0.6),
        2 => (0.33, 0.5, 0.5),
        3 => (0.13, 0.75, 0.5),
        4 => (0.6, 0.7, 0.62),
        5 => (0.83, 0.5, 0.65),
        6 => (0.5, 0.55, 0.5),
        _ => (0., 0., if dark { 0.8 } else { 0.4 }),
    };
    let bright = index >= 8;
    let l = if dark {
        if bright { (l + 0.12).min(0.9) } else { l }
    } else if bright {
        (l - 0.05).max(0.2)
    } else {
        (l - 0.12).max(0.15)
    };
    (h, s, l)
}

/// One of the 256 palette colours as RGB.
pub fn indexed_rgb(index: u8) -> Option<(u8, u8, u8)> {
    match index {
        0..=15 => None,
        16..=231 => {
            let i = index - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Some((level(i / 36), level((i / 6) % 6), level(i % 6)))
        }
        _ => {
            let v = 8 + (index - 232) * 10;
            Some((v, v, v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn colours_carry_until_reset_and_other_escapes_go() {
        let mut style = Style::default();
        let spans = parse("\u{1b}[1;32mOK\u{1b}[0m done \u{1b}[2K\u{1b}]0;title\u{7}\u{1b}[38;5;196mred", &mut style);
        assert_eq!(plain(&spans), "OK done red");
        assert_eq!(spans[0].style.fg, Some(Color::Indexed(2)));
        assert!(spans[0].style.bold);
        assert_eq!(spans[1].style, Style::default());
        assert_eq!(spans[2].style.fg, Some(Color::Indexed(196)));
        // The colour runs on to the next line.
        let next = parse("still red", &mut style);
        assert_eq!(next[0].style.fg, Some(Color::Indexed(196)));
        let rgb = parse("\u{1b}[38;2;1;2;3mx", &mut Style::default());
        assert_eq!(rgb[0].style.fg, Some(Color::Rgb(1, 2, 3)));
    }

    #[test]
    fn stack_frames_link_to_their_class_and_line() {
        let spans = parse("\tat demo.app.Main$Inner.run(Main.java:14) ~[main/:na]", &mut Style::default());
        let link = spans.iter().find_map(|s| s.link.clone().map(|l| (s.text.clone(), l))).unwrap();
        assert_eq!(link.0, "Main.java:14");
        assert_eq!(link.1, Link::Frame { class: "demo.app.Main".into(), file: "Main.java".into(), line: 14 });
        assert_eq!(link.1.relative_file(), Some(PathBuf::from("demo/app/Main.java")));
        let module = find_links("at java.base/java.lang.Thread.run(Thread.java:1583)");
        assert_eq!(
            module[0].1,
            Link::Frame { class: "java.lang.Thread".into(), file: "Thread.java".into(), line: 1583 }
        );
        assert!(find_links("at jdk.internal.reflect.NativeMethodAccessorImpl.invoke0(Native Method)").is_empty());
    }

    #[test]
    fn compiler_paths_link_with_line_and_column() {
        let text = "/w/app/src/Main.java:12: error: ';' expected";
        let links = find_links(text);
        assert_eq!(&text[links[0].0.clone()], "/w/app/src/Main.java:12");
        assert_eq!(links[0].1, Link::Path { path: "/w/app/src/Main.java".into(), line: 12, column: 0 });
        let zinc = "[Error] /w/core/A.scala:3:7: not found: value x";
        let links = find_links(zinc);
        assert_eq!(&zinc[links[0].0.clone()], "/w/core/A.scala:3:7");
        assert_eq!(links[0].1, Link::Path { path: "/w/core/A.scala".into(), line: 3, column: 7 });
        assert!(find_links("see /usr/bin for more, and 12:30 today").is_empty());
    }
}

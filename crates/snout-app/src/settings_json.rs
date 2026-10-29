//! Settings as text: the user's `settings.json` and a project's `.snout/settings.json` opened
//! in the editor get completion, hover and problems from a JSON schema made from the settings'
//! own Rust definitions ([`Settings`] and [`ProjectSettings`], whose doc comments are the
//! descriptions), so the schema cannot drift from what the app reads.
//!
//! No language server is involved: the text is scanned here, for the key or value at the
//! caret (which works while the JSON is half typed) and, whole, for its problems.

use std::ops::Range;
use std::path::Path;
use std::sync::LazyLock;

use serde_json::{Value, json};

use crate::project_settings::ProjectSettings;
use crate::settings::Settings;

/// Which settings a file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// The user's, for every project.
    User,
    /// A project's own.
    Project,
}

/// The settings `path` holds, if it is a settings file: a project's `.snout/settings.json`,
/// or the user's settings file `user`.
pub fn which(path: &Path, user: Option<&Path>) -> Option<Which> {
    if path.ends_with(crate::project_settings::FILE) {
        Some(Which::Project)
    } else if user.is_some_and(|u| u == path) {
        Some(Which::User)
    } else {
        None
    }
}

/// The JSON schema of `which` settings, made from their Rust definitions.
pub fn schema(which: Which) -> &'static Value {
    static USER: LazyLock<Value> = LazyLock::new(|| finish(schemars::schema_for!(Settings)));
    static PROJECT: LazyLock<Value> = LazyLock::new(|| finish(schemars::schema_for!(ProjectSettings)));
    match which {
        Which::User => &USER,
        Which::Project => &PROJECT,
    }
}

/// The schema as JSON, with `$schema` allowed as a key (editors write it), and each
/// description one paragraph (a doc comment's line breaks are only where it wrapped).
fn finish(schema: schemars::Schema) -> Value {
    fn unwrap_lines(v: &mut Value) {
        match v {
            Value::Object(map) => {
                for (k, v) in map.iter_mut() {
                    match v {
                        Value::String(s) if k == "description" => {
                            *s = s.split_whitespace().collect::<Vec<_>>().join(" ")
                        }
                        _ => unwrap_lines(v),
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(unwrap_lines),
            _ => {}
        }
    }
    let mut v = schema.to_value();
    unwrap_lines(&mut v);
    if let Some(props) = v.get_mut("properties").and_then(Value::as_object_mut) {
        props.insert(
            "$schema".into(),
            json!({ "type": "string", "description": "The schema of this file, for other editors." }),
        );
    }
    v
}

// ----- Reading the schema -------------------------------------------------------------------

/// `schema` with its `$ref` followed (within `root`).
fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    let mut s = schema;
    for _ in 0..8 {
        let Some(r) = s.get("$ref").and_then(Value::as_str) else { break };
        let Some(path) = r.strip_prefix("#/") else { break };
        let mut at = root;
        for part in path.split('/') {
            match at.get(part) {
                Some(next) => at = next,
                None => return s,
            }
        }
        s = at;
    }
    s
}

/// The alternatives of `schema` (`anyOf`, `oneOf`), or none.
fn alternatives(schema: &Value) -> Option<&Vec<Value>> {
    schema.get("anyOf").or_else(|| schema.get("oneOf")).and_then(Value::as_array)
}

/// The schema of the value at `path` (keys from the top), if the schema knows it.
pub fn schema_at<'a>(root: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut s = resolve(root, root);
    for key in path {
        s = property(root, s, key)?;
    }
    Some(s)
}

/// The schema of `key` of an object of schema `s`.
fn property<'a>(root: &'a Value, s: &'a Value, key: &str) -> Option<&'a Value> {
    let s = resolve(root, s);
    if let Some(p) = s.get("properties").and_then(|p| p.get(key)) {
        return Some(resolve(root, p));
    }
    if let Some(extra) = s.get("additionalProperties").filter(|a| a.is_object()) {
        return Some(resolve(root, extra));
    }
    alternatives(s)?.iter().find_map(|alt| property(root, alt, key).filter(|p| !is_null_only(p)))
}

fn is_null_only(s: &Value) -> bool {
    s.get("type").and_then(Value::as_str) == Some("null")
}

/// The keys an object of schema `s` knows, with their schemas.
fn properties<'a>(root: &'a Value, s: &'a Value) -> Vec<(&'a str, &'a Value)> {
    let s = resolve(root, s);
    s.get("properties")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.as_str(), resolve(root, v))).collect())
        .unwrap_or_default()
}

/// The JSON types `s` allows ("integer", "string", …), alternatives included.
fn types(root: &Value, s: &Value) -> Vec<String> {
    let s = resolve(root, s);
    let mut out: Vec<String> = match s.get("type") {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(ts)) => ts.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    };
    if out.is_empty() {
        if let Some(e) = s.get("enum").and_then(Value::as_array) {
            out.extend(e.iter().map(|v| json_type(v).to_owned()));
        } else if let Some(c) = s.get("const") {
            out.push(json_type(c).to_owned());
        }
    }
    for alt in alternatives(s).into_iter().flatten() {
        out.extend(types(root, alt));
    }
    out.dedup();
    out
}

fn json_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// The values `s` allows outright (an enum's, a constant's), alternatives included.
fn choices(root: &Value, s: &Value) -> Vec<Value> {
    let s = resolve(root, s);
    let mut out = Vec::new();
    if let Some(e) = s.get("enum").and_then(Value::as_array) {
        out.extend(e.iter().cloned());
    }
    if let Some(c) = s.get("const") {
        out.push(c.clone());
    }
    for alt in alternatives(s).into_iter().flatten() {
        out.extend(choices(root, alt));
    }
    out
}

/// What `s` takes, in a few words: "an integer from 1", "true or false", "\"Light\" or …".
pub fn describe(root: &Value, s: &Value) -> String {
    let named = choices(root, s);
    let mut words: Vec<String> = named.iter().map(Value::to_string).collect();
    let named_types: Vec<&str> = named.iter().map(json_type).collect();
    let rest: Vec<String> = types(root, s)
        .iter()
        .filter(|t| *t != "null" && !named_types.contains(&t.as_str()))
        .map(|t| {
            match t.as_str() {
                "boolean" => "true or false",
                "integer" => "a whole number",
                "number" => "a number",
                "string" => "a string",
                "array" => "a list",
                "object" => "an object",
                other => other,
            }
            .to_owned()
        })
        .collect();
    words.extend(rest);
    let (min, max) = (s.get("minimum").and_then(Value::as_f64), s.get("maximum").and_then(Value::as_f64));
    if let (Some(min), Some(max), [only]) = (min, max, words.as_slice())
        && only.starts_with('a')
        && max < 1e9
    {
        return format!("{only} from {min} to {max}");
    }
    if words.is_empty() { "any value".to_owned() } else { one_of(&words) }
}

fn one_of(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} or {last}", init.join(", ")),
    }
}

/// The description of `s`.
fn description(s: &Value) -> Option<&str> {
    s.get("description").and_then(Value::as_str)
}

/// The value a new `key` of schema `s` starts with: its default, else one that fits.
fn starting_value(root: &Value, s: &Value) -> Value {
    if let Some(d) = s.get("default").filter(|d| !d.is_null()) {
        return d.clone();
    }
    if let Some(c) = choices(root, s).into_iter().next() {
        return c;
    }
    let ts = types(root, s);
    let ts: Vec<&str> = ts.iter().map(String::as_str).filter(|t| *t != "null").collect();
    match ts.first().copied() {
        Some("boolean") => json!(false),
        Some("integer") | Some("number") => json!(0),
        Some("string") => json!(""),
        Some("array") => json!([]),
        Some("object") => json!({}),
        _ => Value::Null,
    }
}

// ----- Scanning the text ----------------------------------------------------------------------

/// What is at an offset of the text, as far as the text up to it shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Spot {
    /// A key of the object at `path` is being written (or the caret is on one): `range` is
    /// the key's text so far, quotes included, `quoted` whether it starts with a quote.
    Key { path: Vec<String>, range: Range<usize>, quoted: bool, present: Vec<String> },
    /// The value of `path`'s last key: `range` is the value's text so far.
    Value { path: Vec<String>, range: Range<usize> },
    /// Neither (inside a list, between punctuation).
    Other,
}

/// Where the containers are while scanning.
#[derive(Debug)]
enum Frame {
    Object { key: Option<String>, after_colon: bool, keys: Vec<String> },
    Array,
}

/// The key or value at byte `offset` of `text`. A token the offset is inside (or right after)
/// counts whole, so hovering a key finds it; the text after the token is not read.
pub fn spot(text: &str, offset: usize) -> Spot {
    let bytes = text.as_bytes();
    let offset = offset.min(text.len());
    let mut stack: Vec<Frame> = Vec::new();
    let mut i = 0;
    let path_of = |stack: &[Frame]| -> Vec<String> {
        stack
            .iter()
            .filter_map(|f| match f {
                Frame::Object { key: Some(k), after_colon: true, .. } => Some(k.clone()),
                _ => None,
            })
            .collect()
    };
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            if i >= offset {
                break;
            }
            i += 1;
            continue;
        }
        let start = i;
        // One token: a string, a word or number, or punctuation.
        let end = match c {
            b'"' => {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'"' && bytes[j] != b'\n' {
                    j += if bytes[j] == b'\\' { 2 } else { 1 };
                }
                if j < bytes.len() && bytes[j] == b'"' { j + 1 } else { j.min(bytes.len()) }
            }
            b'{' | b'}' | b'[' | b']' | b':' | b',' => i + 1,
            _ => {
                let mut j = i;
                while j < bytes.len() && !b"{}[]:,\" \t\r\n".contains(&bytes[j]) {
                    j += 1;
                }
                j
            }
        };
        if start > offset {
            break;
        }
        let punctuation = b"{}[]:,".contains(&c);
        if punctuation && offset == start {
            // The caret before a bracket, colon or comma is in the place before it.
            break;
        }
        let at_caret = start <= offset && offset <= end;
        let in_key_place = matches!(stack.last(), Some(Frame::Object { after_colon: false, .. }));
        match c {
            b'{' => stack.push(Frame::Object { key: None, after_colon: false, keys: Vec::new() }),
            b'[' => stack.push(Frame::Array),
            b'}' | b']' => {
                stack.pop();
                if let Some(Frame::Object { key, after_colon, .. }) = stack.last_mut() {
                    *key = None;
                    *after_colon = false;
                }
            }
            b':' => {
                if let Some(Frame::Object { after_colon, .. }) = stack.last_mut() {
                    *after_colon = true;
                }
            }
            b',' => {
                if let Some(Frame::Object { key, after_colon, .. }) = stack.last_mut() {
                    *key = None;
                    *after_colon = false;
                }
            }
            _ if in_key_place => {
                let name = unquote(&text[start..end]);
                if at_caret {
                    let path = path_of(&stack);
                    let present = match stack.last() {
                        Some(Frame::Object { keys, .. }) => keys.clone(),
                        _ => Vec::new(),
                    };
                    return Spot::Key { path, range: start..end, quoted: c == b'"', present };
                }
                if let Some(Frame::Object { key, keys, .. }) = stack.last_mut() {
                    keys.push(name.clone());
                    *key = Some(name);
                }
            }
            _ => {
                if at_caret {
                    return match stack.last() {
                        Some(Frame::Object { after_colon: true, .. }) => {
                            Spot::Value { path: path_of(&stack), range: start..end }
                        }
                        _ => Spot::Other,
                    };
                }
            }
        }
        i = end.max(start + 1);
        if i > offset {
            break;
        }
    }
    // The caret in the space between tokens.
    match stack.last() {
        Some(Frame::Object { after_colon: false, keys, .. }) => {
            Spot::Key { path: path_of(&stack), range: offset..offset, quoted: false, present: keys.clone() }
        }
        Some(Frame::Object { after_colon: true, .. }) => Spot::Value { path: path_of(&stack), range: offset..offset },
        _ => Spot::Other,
    }
}

/// A key's text without its quotes.
fn unquote(s: &str) -> String {
    let s = s.strip_prefix('"').unwrap_or(s);
    let s = s.strip_suffix('"').unwrap_or(s);
    serde_json::from_str::<String>(&format!("\"{s}\"")).unwrap_or_else(|_| s.to_owned())
}

// ----- Completion and hover --------------------------------------------------------------------

/// A completion: what the list shows and what it puts in place of `range`.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub label: String,
    pub detail: String,
    pub doc: Option<String>,
    pub range: Range<usize>,
    pub insert: String,
    pub is_key: bool,
}

/// What can be written at `offset` of `text`.
pub fn completions(which: Which, text: &str, offset: usize) -> Vec<Completion> {
    let root = schema(which);
    match spot(text, offset) {
        Spot::Key { path, range, present, .. } => {
            let Some(obj) = schema_at(root, &path) else { return Vec::new() };
            // A key already closed and followed by its colon keeps what follows.
            let rest = &text[range.end.min(text.len())..];
            let has_colon = rest.trim_start().starts_with(':');
            let mut out: Vec<Completion> = properties(root, obj)
                .into_iter()
                .filter(|(k, _)| !present.iter().any(|p| p == k) && *k != "$schema")
                .map(|(k, s)| {
                    let key = serde_json::to_string(k).unwrap_or_default();
                    let insert = if has_colon {
                        key
                    } else {
                        let value = serde_json::to_string(&starting_value(root, s)).unwrap_or_default();
                        format!("{key}: {value}")
                    };
                    Completion {
                        label: k.to_owned(),
                        detail: describe(root, s),
                        doc: description(s).map(str::to_owned),
                        range: range.clone(),
                        insert,
                        is_key: true,
                    }
                })
                .collect();
            out.sort_by(|a, b| a.label.cmp(&b.label));
            out
        }
        Spot::Value { path, range } => {
            let Some(s) = schema_at(root, &path) else { return Vec::new() };
            let mut values = choices(root, s);
            if types(root, s).iter().any(|t| t == "boolean") {
                values.extend([json!(true), json!(false)]);
            }
            if values.is_empty() {
                values.push(starting_value(root, s));
            }
            let mut seen = Vec::new();
            values
                .into_iter()
                .filter(|v| !v.is_null() || types(root, s).iter().any(|t| t == "null"))
                .filter_map(|v| {
                    let text = v.to_string();
                    if seen.contains(&text) {
                        return None;
                    }
                    seen.push(text.clone());
                    Some(Completion {
                        label: text.clone(),
                        detail: describe(root, s),
                        doc: description(s).map(str::to_owned),
                        range: range.clone(),
                        insert: text,
                        is_key: false,
                    })
                })
                .collect()
        }
        Spot::Other => Vec::new(),
    }
}

/// What to show over the key or value at `offset`: the key, what it takes, its description
/// and default (Markdown), and the range it is about.
pub fn hover(which: Which, text: &str, offset: usize) -> Option<(String, Range<usize>)> {
    let root = schema(which);
    let (path, range) = match spot(text, offset) {
        Spot::Key { mut path, range, .. } if !range.is_empty() => {
            path.push(unquote(&text[range.clone()]));
            (path, range)
        }
        Spot::Value { path, range } if !range.is_empty() && !path.is_empty() => (path, range),
        _ => return None,
    };
    let s = schema_at(root, &path)?;
    let key = path.last()?;
    let mut md = format!("**{key}**: {}", describe(root, s));
    if let Some(d) = description(s) {
        md.push_str("\n\n");
        md.push_str(d);
    }
    if let Some(d) = s.get("default").filter(|d| !d.is_null()) {
        let shown = d.to_string();
        if shown.len() <= 60 {
            md.push_str(&format!("\n\nDefault: `{shown}`"));
        }
    }
    Some((md, range))
}

// ----- Problems --------------------------------------------------------------------------------

/// A problem of the text: where, how bad, and what.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub range: Range<usize>,
    pub error: bool,
    pub message: String,
}

/// A parsed JSON value with where each part is.
#[derive(Debug, Clone)]
struct Node {
    span: Range<usize>,
    value: Value,
    /// An object's members: key, key's span, value.
    members: Vec<(String, Range<usize>, Node)>,
    items: Vec<Node>,
}

/// The problems of `text` as `which` settings: JSON that cannot be read, keys the settings
/// do not have, values of the wrong type or out of range.
pub fn problems(which: Which, text: &str) -> Vec<Problem> {
    let mut p = Parser { text: text.as_bytes(), i: 0 };
    p.ws();
    if p.i == text.len() {
        return Vec::new();
    }
    let node = match p.value().and_then(|n| {
        p.ws();
        if p.i < text.len() { Err((p.i, "Unexpected text after the settings".to_owned())) } else { Ok(n) }
    }) {
        Ok(n) => n,
        Err((at, message)) => {
            let at = floor_char(text, at.min(text.len()));
            let end = next_char(text, at);
            return vec![Problem { range: at..end, error: true, message }];
        }
    };
    let root = schema(which);
    let mut out = Vec::new();
    check(root, resolve(root, root), &node, &[], &mut out);
    out
}

fn floor_char(text: &str, mut at: usize) -> usize {
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn next_char(text: &str, at: usize) -> usize {
    text[at..].chars().next().map_or(at, |c| at + c.len_utf8())
}

/// Check `node` against schema `s`, adding what is wrong to `out`.
fn check(root: &Value, s: &Value, node: &Node, path: &[String], out: &mut Vec<Problem>) {
    let s = resolve(root, s);
    if let Some(alts) = alternatives(s) {
        // Fine when an alternative takes it. Else, when one of the value's own type finds
        // something inside it (a key it does not know), that says more than "takes ...".
        let mut closest: Option<Vec<Problem>> = None;
        for alt in alts {
            let mut found = Vec::new();
            check(root, alt, node, path, &mut found);
            if found.is_empty() {
                return;
            }
            let inside = !found.iter().any(|p| p.range == node.span);
            if inside && closest.as_ref().is_none_or(|c| found.len() < c.len()) {
                closest = Some(found);
            }
        }
        match closest {
            Some(found) => out.extend(found),
            None => {
                let what = path.last().map_or("The settings".to_owned(), |k| format!("`{k}`"));
                out.push(Problem {
                    range: node.span.clone(),
                    error: true,
                    message: format!("{what} takes {}", describe(root, s)),
                });
            }
        }
        return;
    }
    let what = || path.last().map_or("The settings".to_owned(), |k| format!("`{k}`"));
    let ts = types(root, s);
    let kind = json_type(&node.value);
    let fits = ts.is_empty() || ts.iter().any(|t| t == kind || (t == "number" && kind == "integer"));
    if !fits {
        out.push(Problem {
            range: node.span.clone(),
            error: true,
            message: format!("{} takes {}", what(), describe(root, s)),
        });
        return;
    }
    let named = choices(root, s);
    if !named.is_empty() && !named.contains(&node.value) {
        out.push(Problem {
            range: node.span.clone(),
            error: true,
            message: format!("{} takes {}", what(), describe(root, s)),
        });
        return;
    }
    if let Some(n) = node.value.as_f64() {
        if let Some(min) = s.get("minimum").and_then(Value::as_f64)
            && n < min
        {
            out.push(Problem {
                range: node.span.clone(),
                error: true,
                message: format!("{} is at least {min}", what()),
            });
        }
        if let Some(max) = s.get("maximum").and_then(Value::as_f64)
            && n > max
        {
            out.push(Problem {
                range: node.span.clone(),
                error: true,
                message: format!("{} is at most {max}", what()),
            });
        }
    }
    if node.value.is_object() {
        let known = s.get("properties").and_then(Value::as_object);
        let extra = s.get("additionalProperties");
        for (key, key_span, value) in &node.members {
            let mut p = path.to_vec();
            p.push(key.clone());
            match known.and_then(|k| k.get(key)) {
                Some(ks) => check(root, ks, value, &p, out),
                None => match extra {
                    Some(e) if e.is_object() => check(root, e, value, &p, out),
                    Some(Value::Bool(true)) => {}
                    _ if known.is_some() || extra == Some(&Value::Bool(false)) => out.push(Problem {
                        range: key_span.clone(),
                        error: false,
                        message: format!("Unknown setting `{key}`: SnoutCode ignores it"),
                    }),
                    _ => {}
                },
            }
        }
        for req in s.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            if !node.members.iter().any(|(k, _, _)| k == req) {
                out.push(Problem {
                    range: node.span.clone(),
                    error: true,
                    message: format!("{} needs `{req}`", what()),
                });
            }
        }
    }
    if let Some(items) = s.get("items").filter(|i| i.is_object()) {
        for item in &node.items {
            check(root, items, item, path, out);
        }
    }
}

/// A strict JSON reader that keeps where each value is.
struct Parser<'a> {
    text: &'a [u8],
    i: usize,
}

type Parsed<T> = Result<T, (usize, String)>;

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.text.len() && self.text[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Parsed<Node> {
        self.ws();
        let start = self.i;
        let Some(&c) = self.text.get(self.i) else { return Err((self.i, "A value is missing here".into())) };
        match c {
            b'{' => {
                self.i += 1;
                let mut members = Vec::new();
                let mut map = serde_json::Map::new();
                self.ws();
                if self.text.get(self.i) == Some(&b'}') {
                    self.i += 1;
                } else {
                    loop {
                        self.ws();
                        let key_start = self.i;
                        if self.text.get(self.i) != Some(&b'"') {
                            return Err((self.i, "A key in double quotes is expected here".into()));
                        }
                        let key = self.string()?;
                        let key_span = key_start..self.i;
                        self.ws();
                        if self.text.get(self.i) != Some(&b':') {
                            return Err((self.i, format!("A colon is expected after \"{key}\"")));
                        }
                        self.i += 1;
                        let value = self.value()?;
                        map.insert(key.clone(), value.value.clone());
                        members.push((key, key_span, value));
                        self.ws();
                        match self.text.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err((self.i, "A comma or a closing brace is expected here".into())),
                        }
                    }
                }
                Ok(Node { span: start..self.i, value: Value::Object(map), members, items: Vec::new() })
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.text.get(self.i) == Some(&b']') {
                    self.i += 1;
                } else {
                    loop {
                        items.push(self.value()?);
                        self.ws();
                        match self.text.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err((self.i, "A comma or a closing bracket is expected here".into())),
                        }
                    }
                }
                let value = Value::Array(items.iter().map(|n| n.value.clone()).collect());
                Ok(Node { span: start..self.i, value, members: Vec::new(), items })
            }
            b'"' => {
                let s = self.string()?;
                Ok(Node { span: start..self.i, value: Value::String(s), members: Vec::new(), items: Vec::new() })
            }
            _ => {
                while self.i < self.text.len() && !b"{}[]:,\" \t\r\n".contains(&self.text[self.i]) {
                    self.i += 1;
                }
                let word = std::str::from_utf8(&self.text[start..self.i]).unwrap_or_default();
                let value = serde_json::from_str::<Value>(word)
                    .ok()
                    .filter(|v| !v.is_object() && !v.is_array() && !v.is_string())
                    .ok_or_else(|| {
                        let shown = if word.is_empty() { "this".to_owned() } else { format!("`{word}`") };
                        (start, format!("{shown} is not a JSON value (strings take double quotes)"))
                    })?;
                Ok(Node { span: start..self.i, value, members: Vec::new(), items: Vec::new() })
            }
        }
    }

    /// A string, the caret on its opening quote.
    fn string(&mut self) -> Parsed<String> {
        let start = self.i;
        self.i += 1;
        while self.i < self.text.len() {
            match self.text[self.i] {
                b'\\' => self.i += 2,
                b'"' => {
                    self.i += 1;
                    let raw = std::str::from_utf8(&self.text[start..self.i]).unwrap_or_default();
                    return serde_json::from_str::<String>(raw)
                        .map_err(|e| (start, format!("This string cannot be read: {e}")));
                }
                b'\n' => break,
                _ => self.i += 1,
            }
        }
        Err((start, "This string has no closing quote".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schemas_are_made_from_the_settings_and_know_every_key() {
        // Every key the app writes is in the user's schema: nothing written reads as unknown.
        let written = serde_json::to_value(Settings::default()).unwrap();
        let root = schema(Which::User);
        let props = root["properties"].as_object().unwrap();
        for key in written.as_object().unwrap().keys() {
            assert!(props.contains_key(key), "{key} is in the schema");
        }
        assert!(description(&props["tab_size"]).is_some_and(|d| d.contains("indentation")), "doc comments describe");
        for key in crate::project_settings::Key::ALL {
            assert!(schema(Which::Project)["properties"].get(key.name()).is_some(), "{}", key.name());
        }
        assert!(problems(Which::User, &serde_json::to_string_pretty(&Settings::default()).unwrap()).is_empty());
    }

    #[test]
    fn the_problems_are_unknown_keys_wrong_types_and_broken_json() {
        let text = r#"{ "tab_size": "two", "soft_wrap": true, "tabsize": 2, "spring_profiles": "some" }"#;
        let found = problems(Which::Project, text);
        let at = |needle: &str| text.find(needle).unwrap();
        assert!(
            found.iter().any(|p| p.error && p.range.start == at("\"two\"") && p.message.contains("tab_size")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|p| !p.error && p.range.start == at("\"tabsize\"") && p.message.contains("Unknown")),
            "{found:?}"
        );
        assert!(found.iter().any(|p| p.error && p.range.start == at("\"some\"")), "{found:?}");
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(problems(Which::Project, r#"{ "spring_profiles": ["dev"], "jdk": "/j" }"#).is_empty());
        assert!(problems(Which::User, r#"{ "theme": "Dark", "colors": "Deep" }"#).is_empty());
        let bad = problems(Which::User, r#"{ "theme": "Purple" }"#);
        assert!(bad[0].message.contains("\"Dark\""), "{bad:?}");
        let broken = problems(Which::User, "{ \"tab_size\": 2,\n}");
        assert_eq!(broken.len(), 1);
        assert!(broken[0].error && broken[0].message.contains("key"), "{broken:?}");
        assert!(problems(Which::User, "").is_empty());
    }

    #[test]
    fn completion_offers_the_keys_left_and_the_values_a_key_takes() {
        let text = "{\n  \"tab_size\": 2,\n  \"so\n}";
        let at = text.find("\"so").unwrap() + 3;
        let items = completions(Which::Project, text, at);
        let wrap = items.iter().find(|c| c.label == "soft_wrap").expect("soft_wrap offered");
        assert_eq!(wrap.insert, "\"soft_wrap\": false");
        assert_eq!(&text[wrap.range.clone()], "\"so");
        assert!(!items.iter().any(|c| c.label == "tab_size"), "a key already there is not offered");
        // A value: the choices of the key.
        let text = r#"{ "theme": "#;
        let values: Vec<String> = completions(Which::User, text, text.len()).into_iter().map(|c| c.label).collect();
        assert!(values.contains(&"\"Dark\"".to_owned()) && values.contains(&"\"System\"".to_owned()), "{values:?}");
        let text = r#"{ "soft_wrap": tr"#;
        let values: Vec<String> = completions(Which::User, text, text.len()).into_iter().map(|c| c.label).collect();
        assert_eq!(values, ["true", "false"]);
    }

    #[test]
    fn hover_describes_the_key_under_the_pointer() {
        let text = r#"{ "tab_size": 2 }"#;
        let (md, range) = hover(Which::Project, text, 6).unwrap();
        assert!(md.starts_with("**tab_size**"), "{md}");
        assert!(md.contains("indentation"), "{md}");
        assert_eq!(&text[range], "\"tab_size\"");
        let (md, _) = hover(Which::User, r#"{ "tab_size": 2 }"#, 15).unwrap();
        assert!(md.contains("Default: `4`"), "{md}");
        assert!(hover(Which::User, "{ }", 1).is_none());
    }
}

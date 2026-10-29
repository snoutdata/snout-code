//! Go to Class: the classes, interfaces, enums and records of the project, its libraries
//! and the JDK, by name. The engine's `workspace/symbol` matches a name's start and its
//! capitals (`AL` finds `ArrayList`); a second query with the typed letters as a pattern
//! (`a*r*l*s*t*`) catches the rest, and the answers together are ranked by how well the
//! name matches, the project's own classes first.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{App, Entity, Task};
use lsp_types::{SymbolInformation, SymbolKind, Uri};
use serde_json::json;

use super::LangHub;
use crate::search::{Item, LIMIT, SymbolSource};

/// The queries for `typed`: as typed, and its letters as a pattern, when that differs.
pub fn queries(typed: &str) -> Vec<String> {
    let q = typed.trim();
    let mut out = vec![q.to_owned()];
    let plain = q.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    if plain && q.chars().count() >= 2 {
        let mut pattern: String = q.chars().flat_map(|c| [c, '*']).collect();
        pattern.pop();
        out.push(pattern);
    }
    out
}

/// Whether `kind` is a type.
pub fn is_class(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::CLASS | SymbolKind::INTERFACE | SymbolKind::ENUM | SymbolKind::STRUCT)
}

/// The library a class comes from, by its `jdt://` URI: the jar's name, or the JDK's
/// module (`java.base`). None for a file of the project.
pub fn library_of(uri: &Uri) -> Option<String> {
    let s = uri.as_str();
    let rest = s.strip_prefix("jdt://contents/")?;
    let first = rest.split('/').next().unwrap_or_default();
    let name = percent_decode(first);
    (!name.is_empty()).then_some(name)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The rows for the types among `found` that match `typed`, best first: by how well the name
/// matches, then the project's own before the libraries', then the shorter name. Each shows
/// its package, and where a library's comes from.
pub fn rank(found: Vec<SymbolInformation>, typed: &str, root: &Path) -> Vec<Item> {
    use fuzzy_matcher::FuzzyMatcher as _;
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().ignore_case();
    let q = typed.trim();
    let name_part = q.rsplit('.').next().unwrap_or(q);
    let mut seen = std::collections::HashSet::new();
    let mut hits: Vec<(i64, bool, SymbolInformation)> = found
        .into_iter()
        .filter(|s| is_class(s.kind))
        .filter(|s| seen.insert((s.name.clone(), s.container_name.clone(), s.location.uri.as_str().to_owned())))
        .filter_map(|s| {
            let score = if name_part.is_empty() { Some(0) } else { matcher.fuzzy_match(&s.name, name_part) }?;
            // A name that starts as typed, or is it, goes first.
            let bonus = if s.name.eq_ignore_ascii_case(name_part) {
                1000
            } else if s.name.to_lowercase().starts_with(&name_part.to_lowercase()) {
                200
            } else {
                0
            };
            let library = library_of(&s.location.uri).is_some() || snout_lang::uri::to_path(&s.location.uri).is_none();
            Some((score + bonus, library, s))
        })
        .collect();
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.name.len().cmp(&b.2.name.len())).then(a.2.name.cmp(&b.2.name))
    });
    hits.into_iter()
        .take(LIMIT)
        .map(|(_, _, s)| {
            let package = s.container_name.clone().unwrap_or_default();
            let place = library_of(&s.location.uri);
            let detail = match place {
                Some(lib) if package.is_empty() => lib,
                Some(lib) => format!("{package}  {lib}"),
                None => package,
            };
            Item {
                path: s.name.clone(),
                detail: Some(detail).filter(|d| !d.is_empty()),
                target: Some((root.to_owned(), s.location.uri, s.location.range.start)),
                symbol: Some((s.kind, None)),
                ..Default::default()
            }
        })
        .collect()
}

/// The classes of the build at `root` that match what is typed, from its engine.
pub fn source(hub: Entity<LangHub>, root: PathBuf) -> SymbolSource {
    Rc::new(move |typed: String, cx: &mut App| {
        if typed.trim().is_empty() {
            return Task::ready(Vec::new());
        }
        let calls: Vec<_> = queries(&typed)
            .into_iter()
            .map(|q| hub.read(cx).request(&root, "workspace/symbol", json!({ "query": q })))
            .collect();
        let root = root.clone();
        cx.background_executor().spawn(async move {
            let mut found: Vec<SymbolInformation> = Vec::new();
            for call in calls {
                if let Ok(v) = call.await {
                    found.extend(serde_json::from_value::<Vec<SymbolInformation>>(v).unwrap_or_default());
                }
            }
            rank(found, &typed, &root)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr as _;

    #[allow(deprecated)]
    fn info(name: &str, kind: SymbolKind, package: &str, uri: &str) -> SymbolInformation {
        SymbolInformation {
            name: name.into(),
            kind,
            tags: None,
            deprecated: None,
            location: lsp_types::Location { uri: Uri::from_str(uri).unwrap(), range: Default::default() },
            container_name: Some(package.into()),
        }
    }

    #[test]
    fn ranks_classes_by_how_well_they_match() {
        assert_eq!(queries("arlst"), ["arlst", "a*r*l*s*t"]);
        assert_eq!(queries("java.util.List"), ["java.util.List"]);
        assert_eq!(queries("A"), ["A"]);
        let jdk = "jdt://contents/java.base/java.util/ArrayList.class?=shop/%5C/jdk";
        let found = vec![
            info("ArrayList", SymbolKind::CLASS, "java.util", jdk),
            info("Arrays", SymbolKind::CLASS, "java.util", "jdt://contents/java.base/java.util/Arrays.class"),
            info("arrayList", SymbolKind::FIELD, "demo", "file:///w/A.java"),
            info("ArrayLister", SymbolKind::CLASS, "demo.shop", "file:///w/src/ArrayLister.java"),
            info("ArrayList", SymbolKind::CLASS, "java.util", jdk),
        ];
        let rows = rank(found, "arrlist", Path::new("/w"));
        let mut names: Vec<(&str, Option<&str>)> =
            rows.iter().map(|r| (r.path.as_str(), r.detail.as_deref())).collect();
        names.sort();
        // Types only, each once, with its package and library.
        assert_eq!(names, [("ArrayList", Some("java.util  java.base")), ("ArrayLister", Some("demo.shop"))]);
        // The exact name first; the project's before a library's otherwise alike.
        let rows = rank(
            vec![
                info("Shape", SymbolKind::INTERFACE, "java.awt", "jdt://contents/java.desktop/java.awt/Shape.class"),
                info("Shape", SymbolKind::INTERFACE, "demo.shop", "file:///w/Shape.java"),
                info("ShapeBuilder", SymbolKind::CLASS, "demo.shop", "file:///w/ShapeBuilder.java"),
            ],
            "Shape",
            Path::new("/w"),
        );
        let names: Vec<&str> = rows.iter().map(|r| r.detail.as_deref().unwrap_or_default()).collect();
        assert_eq!(names, ["demo.shop", "java.awt  java.desktop", "demo.shop"]);
        assert_eq!(
            library_of(&Uri::from_str("jdt://contents/guava-33.0.jar/com.google/X.class").unwrap()).as_deref(),
            Some("guava-33.0.jar")
        );
    }
}

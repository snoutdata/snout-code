//! A Maven `pom.xml` in the editor: completion of dependency and plugin coordinates from the
//! local repository (`~/.m2/repository`, or where `settings.xml` puts it), by what is typed
//! in a `<groupId>`, an `<artifactId>` (of the block's group) or a `<version>` (of the
//! block's artifact). Each completion lists one folder of the repository, off the UI thread,
//! on the window's host.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Result;
use gpui::{App, Context, Entity, Task, Window};
use gpui_component::input::{CompletionProvider, EditorState, Rope};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit, TextEdit,
};
use snout_host::HostRef;

use super::EditorPanel;
use crate::lang::text;

/// What is being typed: the element, what it already holds, where that is, and the block's
/// group and artifact.
#[derive(Debug, PartialEq, Eq)]
pub struct PomSpot {
    pub element: &'static str,
    pub typed: String,
    pub range: std::ops::Range<usize>,
    pub group: Option<String>,
    pub artifact: Option<String>,
}

/// The coordinate element the caret at `offset` is in, if any.
pub fn spot(text: &str, offset: usize) -> Option<PomSpot> {
    let before = &text[..offset.min(text.len())];
    let open = before.rfind('<')?;
    let tag_end = before[open..].find('>')? + open;
    let element = match &before[open + 1..tag_end] {
        "groupId" => "groupId",
        "artifactId" => "artifactId",
        "version" => "version",
        _ => return None,
    };
    let typed = &before[tag_end + 1..];
    if typed.contains('<') || typed.contains('\n') {
        return None;
    }
    // The block: from the nearest dependency, plugin, parent or path opening before the caret.
    let start = ["<dependency>", "<plugin>", "<parent>", "<path>", "<extension>"]
        .iter()
        .filter_map(|t| before.rfind(t))
        .max()
        .unwrap_or(0);
    let end = ["</dependency>", "</plugin>", "</parent>", "</path>", "</extension>"]
        .iter()
        .filter_map(|t| text[offset..].find(t).map(|i| i + offset))
        .min()
        .unwrap_or(text.len());
    let block = &text[start..end];
    let value = |name: &str| {
        let open = format!("<{name}>");
        let at = block.find(&open)? + open.len();
        let close = block[at..].find('<')? + at;
        let v = block[at..close].trim();
        (!v.is_empty() && !v.contains("${")).then(|| v.to_owned())
    };
    Some(PomSpot {
        element,
        typed: typed.trim_start().to_owned(),
        range: offset - typed.trim_start().len()..offset,
        group: if element == "groupId" { None } else { value("groupId") },
        artifact: if element == "version" { value("artifactId") } else { None },
    })
}

/// The completions for `s` in the repository at `repo`, folders listed by `list`.
pub fn completions(s: &PomSpot, repo: &Path, list: &dyn Fn(&Path) -> Vec<String>) -> Vec<String> {
    match s.element {
        "groupId" => {
            // Complete one segment of the group at a time: `org.spring` lists `org/`.
            let (parent, _) = s.typed.rsplit_once('.').unwrap_or(("", s.typed.as_str()));
            let dir = parent.split('.').filter(|p| !p.is_empty()).fold(repo.to_path_buf(), |d, p| d.join(p));
            let mut out: Vec<String> = list(&dir)
                .into_iter()
                .filter(|n| !n.starts_with('.'))
                .map(|n| if parent.is_empty() { n } else { format!("{parent}.{n}") })
                .filter(|g| g.starts_with(&s.typed))
                .collect();
            out.sort();
            out
        }
        "artifactId" => {
            let Some(group) = &s.group else { return Vec::new() };
            let dir = group.split('.').fold(repo.to_path_buf(), |d, p| d.join(p));
            let mut out: Vec<String> = list(&dir)
                .into_iter()
                .filter(|n| !n.starts_with('.') && n.starts_with(&s.typed))
                .filter(|n| !list(&dir.join(n)).is_empty())
                .collect();
            out.sort();
            out
        }
        "version" => {
            let (Some(group), Some(artifact)) = (&s.group, &s.artifact) else { return Vec::new() };
            let dir = group.split('.').fold(repo.to_path_buf(), |d, p| d.join(p)).join(artifact);
            let mut out: Vec<String> = list(&dir)
                .into_iter()
                .filter(|n| n.chars().next().is_some_and(|c| c.is_ascii_digit()) && n.starts_with(&s.typed))
                .collect();
            out.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
            out
        }
        _ => Vec::new(),
    }
}

fn version_key(v: &str) -> Vec<u64> {
    v.split(['.', '-']).map(|p| p.parse().unwrap_or(0)).collect()
}

/// The local repository on a host: `settings.xml`'s `localRepository`, else `~/.m2/repository`.
pub fn local_repository(host: &dyn snout_host::Host) -> PathBuf {
    let home = host.info().home.clone();
    let settings = host.read_to_string(&home.join(".m2/settings.xml")).unwrap_or_default();
    if let Some(at) = settings.find("<localRepository>") {
        let rest = &settings[at + 17..];
        if let Some(end) = rest.find("</localRepository>") {
            let v = rest[..end].trim().replace("${user.home}", &home.to_string_lossy());
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    home.join(".m2/repository")
}

/// Whether `path` is a Maven POM.
pub fn is_pom(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == "pom.xml") || path.extension().is_some_and(|e| e == "pom")
}

struct PomProviders(HostRef);

impl CompletionProvider for PomProviders {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let text = rope.to_string();
        let Some(s) = spot(&text, offset) else {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        };
        let host = self.0.clone();
        let range = text::range_to_kit(rope, text::range_to_lsp(rope, &s.range));
        cx.background_executor().spawn(async move {
            let repo = local_repository(&*host);
            let list = |d: &Path| -> Vec<String> {
                host.read_dir(d)
                    .map(|es| {
                        es.into_iter()
                            .filter(|e| e.is_dir)
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let detail = match s.element {
                "groupId" => "group in the local repository",
                "artifactId" => "artifact in the local repository",
                _ => "version in the local repository",
            };
            let items = completions(&s, &repo, &list)
                .into_iter()
                .take(200)
                .enumerate()
                .map(|(i, label)| CompletionItem {
                    label: label.clone(),
                    kind: Some(if s.element == "version" {
                        CompletionItemKind::VALUE
                    } else {
                        CompletionItemKind::MODULE
                    }),
                    detail: Some(detail.into()),
                    sort_text: Some(format!("{i:04}")),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text: label })),
                    ..Default::default()
                })
                .collect();
            Ok(CompletionResponse::List(lsp_types::CompletionList { is_incomplete: true, items }))
        })
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        new_text.chars().last().is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '>'))
    }
}

fn install(state: &Entity<EditorState>, cx: &mut App) {
    let providers = Rc::new(PomProviders(crate::host::host(cx)));
    state.update(cx, |s, cx| {
        s.lsp_mut().completion_provider = Some(providers);
        cx.notify();
    });
}

impl EditorPanel {
    /// A POM: completion of coordinates from the local repository.
    pub(super) fn attach_pom_lang(&mut self, cx: &mut Context<Self>) -> bool {
        if !is_pom(&self.path) {
            return false;
        }
        install(&self.state, cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POM: &str = "<project>\n  <dependencies>\n    <dependency>\n      <groupId>org.apache.commons</groupId>\n      \
                       <artifactId>commons-la</artifactId>\n      <version>3.</version>\n    </dependency>\n  </dependencies>\n</project>\n";

    fn repo(d: &Path) -> Vec<String> {
        let s = d.to_string_lossy().replace('\\', "/");
        let names: &[&str] = match s.as_str() {
            "/r" => &["org", "com", ".cache"],
            "/r/org" => &["apache", "junit"],
            "/r/org/apache" => &["commons", "maven"],
            "/r/org/apache/commons" => &["commons-lang3", "commons-text"],
            "/r/org/apache/commons/commons-lang3" => &["3.12.0", "3.17.0", "maven-metadata.xml"],
            "/r/org/apache/commons/commons-text" => &["1.12.0"],
            _ => &[],
        };
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn completes_groups_artifacts_and_versions_from_the_repository() {
        let at = |needle: &str| POM.find(needle).unwrap() + needle.len();
        let g = spot(POM, at("<groupId>org.apa")).unwrap();
        assert_eq!(g.element, "groupId");
        assert_eq!(completions(&g, Path::new("/r"), &repo), ["org.apache"]);
        let a = spot(POM, at("commons-la")).unwrap();
        assert_eq!(a.group.as_deref(), Some("org.apache.commons"));
        assert_eq!(completions(&a, Path::new("/r"), &repo), ["commons-lang3"]);
        let v = spot(POM, at("<version>3.")).unwrap();
        assert_eq!(v.artifact.as_deref(), Some("commons-la"));
        let mut versions = POM.replace("commons-la<", "commons-lang3<");
        let at3 = versions.find("<version>3.").unwrap() + "<version>3.".len();
        let v = spot(&versions, at3).unwrap();
        assert_eq!(completions(&v, Path::new("/r"), &repo), ["3.17.0", "3.12.0"], "newest first");
        versions.clear();
        assert!(spot(POM, at("<project>")).is_none(), "outside a coordinate");
    }
}

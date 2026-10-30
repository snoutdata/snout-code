//! Where Gradle declares what the Project Structure window shows, for "Edit in build file" and
//! "Show in build file": the line of a build script, settings script, version catalog or wrapper
//! properties file that says it. Read through the window's host, off the UI thread. Found by
//! reading the text (a dependency's coordinates, its catalog alias, a Java level's keyword): a
//! declaration made some other way (a convention plugin, a computed string) is not found, and
//! the file opens at its top instead.

use std::path::{Path, PathBuf};

use snout_host::Host;

use super::data::{ModuleInfo, Structure};

/// A file to open, at a line (from 0) when the declaration was found in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub file: PathBuf,
    pub line: Option<u32>,
}

/// What to find.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A library, by group and name.
    Library { group: String, name: String },
    /// A dependency on another project, by its Gradle path.
    Project { path: String },
    /// The module's Java level (source or target compatibility, toolchain, release).
    Level,
    /// The Gradle version the wrapper downloads.
    GradleVersion,
    /// The module's `include` in the settings script.
    Include,
    /// A plugin, by id.
    Plugin(String),
}

/// A library of a version catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogLibrary {
    pub alias: String,
    pub group: String,
    pub name: String,
    pub line: u32,
}

/// `gradle/libs.versions.toml`, as much as finding declarations needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    pub libraries: Vec<CatalogLibrary>,
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches(|c| c == '"' || c == '\'')
}

/// The value of `key = "…"` inside an inline table `{ … }`.
fn inline_value<'a>(table: &'a str, key: &str) -> Option<&'a str> {
    let body = table.trim().trim_start_matches('{').trim_end_matches('}');
    body.split(',').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        (k.trim() == key).then(|| unquote(v))
    })
}

/// The libraries a version catalog declares, with their lines.
pub fn parse_catalog(text: &str) -> Catalog {
    let mut out = Catalog::default();
    let mut section = "";
    for (i, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').trim();
            continue;
        }
        if section != "libraries" {
            continue;
        }
        let Some((alias, value)) = line.split_once('=') else { continue };
        let alias = unquote(alias).to_owned();
        let value = value.trim();
        let (group, name) = if value.starts_with('{') {
            match inline_value(value, "module") {
                Some(module) => match module.split_once(':') {
                    Some((g, n)) => (g.to_owned(), n.to_owned()),
                    None => continue,
                },
                None => match (inline_value(value, "group"), inline_value(value, "name")) {
                    (Some(g), Some(n)) => (g.to_owned(), n.to_owned()),
                    _ => continue,
                },
            }
        } else {
            let mut parts = unquote(value).split(':');
            match (parts.next(), parts.next()) {
                (Some(g), Some(n)) => (g.to_owned(), n.to_owned()),
                _ => continue,
            }
        };
        out.libraries.push(CatalogLibrary { alias, group, name, line: i as u32 });
    }
    out
}

/// How a build script reaches a catalog alias: `junit-bom` is `libs.junit.bom`.
pub fn accessor(alias: &str) -> String {
    format!("libs.{}", alias.replace(['-', '_'], "."))
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with('*') || t.starts_with("/*")
}

/// The first line of `text` (not a comment) that `matches` takes.
fn first_line(text: &str, matches: impl Fn(&str) -> bool) -> Option<u32> {
    text.lines().enumerate().find(|(_, l)| !is_comment(l) && matches(l)).map(|(i, _)| i as u32)
}

/// Whether `line` holds `needle` with no identifier character or dot right after it.
fn holds_word(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(at, _)| {
        let next = line[at + needle.len()..].chars().next();
        !next.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    })
}

/// The line of a build script declaring the library `group:name`: as a string
/// (`"group:name:1.0"`), a map (`group: 'g', name: 'n'`) or a catalog alias (`libs.guava`).
pub fn library_line(script: &str, group: &str, name: &str, catalog: Option<&Catalog>) -> Option<u32> {
    let coords = format!("{group}:{name}");
    let quoted = |l: &str| {
        ['"', '\''].iter().any(|q| {
            let at = format!("{q}{coords}");
            l.match_indices(&at).any(|(i, _)| matches!(l[i + at.len()..].chars().next(), Some(':' | '"' | '\'' | '@')))
        })
    };
    let mapped = |l: &str| {
        let (g, n) = (format!("'{group}'"), format!("'{name}'"));
        let (gd, nd) = (format!("\"{group}\""), format!("\"{name}\""));
        l.contains("group")
            && l.contains("name")
            && (l.contains(&g) || l.contains(&gd))
            && (l.contains(&n) || l.contains(&nd))
    };
    if let Some(line) = first_line(script, |l| quoted(l) || mapped(l)) {
        return Some(line);
    }
    let aliases: Vec<String> = catalog
        .map(|c| {
            c.libraries.iter().filter(|l| l.group == group && l.name == name).map(|l| accessor(&l.alias)).collect()
        })
        .unwrap_or_default();
    first_line(script, |l| aliases.iter().any(|a| holds_word(l, a)))
}

/// The line of a build script depending on the project at `path`: `project(":core")` or the
/// type-safe `projects.core`.
pub fn project_line(script: &str, path: &str) -> Option<u32> {
    let quoted = [format!("\"{path}\""), format!("'{path}'")];
    let typed =
        format!("projects.{}", path.trim_start_matches(':').split(':').map(camel).collect::<Vec<_>>().join("."));
    first_line(script, |l| (l.contains("project(") && quoted.iter().any(|q| l.contains(q))) || holds_word(l, &typed))
}

/// A project name as type-safe accessors spell it: `web-api` is `webApi`.
fn camel(name: &str) -> String {
    let mut out = String::new();
    for (i, part) in name.split(['-', '_']).enumerate() {
        let mut chars = part.chars();
        match (i, chars.next()) {
            (0, Some(c)) => out.extend(std::iter::once(c).chain(chars)),
            (_, Some(c)) => out.extend(c.to_uppercase().chain(chars)),
            _ => {}
        }
    }
    out
}

/// The line setting the Java level: a source or target compatibility, a toolchain's language
/// version, `jvmToolchain`, or a `--release`.
pub fn level_line(script: &str) -> Option<u32> {
    const KEYS: [&str; 5] =
        ["languageVersion", "jvmToolchain", "sourceCompatibility", "targetCompatibility", "release"];
    first_line(script, |l| {
        KEYS.iter().any(|k| {
            l.match_indices(k).any(|(at, _)| {
                let before = l[..at].chars().last();
                let after = l[at + k.len()..].trim_start().chars().next();
                !before.is_some_and(|c| c.is_alphanumeric())
                    && matches!(after, Some('=' | '(' | '.' | ' '))
                    && (*k != "release" || l.contains("options") || l.contains(".set") || l.contains("release ="))
            })
        })
    })
}

/// The wrapper's `distributionUrl` line.
pub fn wrapper_line(properties: &str) -> Option<u32> {
    first_line(properties, |l| l.trim_start().starts_with("distributionUrl"))
}

/// The settings script's line including the project at `path` (or the build `name` with
/// `includeBuild`).
pub fn include_line(settings: &str, path: &str, included: Option<&str>) -> Option<u32> {
    if let Some(name) = included.filter(|_| path == ":") {
        return first_line(settings, |l| l.contains("includeBuild") && l.contains(name));
    }
    let bare = path.trim_start_matches(':');
    if bare.is_empty() {
        return first_line(settings, |l| l.contains("rootProject.name"));
    }
    let forms = [format!("\"{path}\""), format!("'{path}'"), format!("\"{bare}\""), format!("'{bare}'")];
    first_line(settings, |l| l.contains("include") && forms.iter().any(|f| l.contains(f)))
}

/// The line applying the plugin `id`.
pub fn plugin_line(script: &str, id: &str) -> Option<u32> {
    let forms = [format!("\"{id}\""), format!("'{id}'")];
    first_line(script, |l| forms.iter().any(|f| l.contains(f)))
}

fn settings_file(host: &dyn Host, dir: &Path) -> Option<PathBuf> {
    ["settings.gradle.kts", "settings.gradle"].iter().map(|n| dir.join(n)).find(|f| host.is_file(f))
}

fn build_file(host: &dyn Host, dir: &Path) -> Option<PathBuf> {
    ["build.gradle.kts", "build.gradle"].iter().map(|n| dir.join(n)).find(|f| host.is_file(f))
}

/// The folder of the build a module belongs to: its included build's, or its build's root.
fn build_dir(s: &Structure, m: &ModuleInfo) -> PathBuf {
    let root = s.build_of(m).map(|b| b.root_dir.clone()).unwrap_or_else(|| s.root.clone());
    if m.included.is_none() {
        return root;
    }
    // An included build's root project is the module of its build at `:`.
    s.modules
        .iter()
        .find(|o| o.build == m.build && o.included == m.included && o.gradle_path == ":")
        .map(|o| o.dir.clone())
        .unwrap_or(root)
}

/// The version catalog of the build `m` belongs to, if it has one.
pub fn catalog_of(host: &dyn Host, s: &Structure, m: &ModuleInfo) -> Option<(PathBuf, Catalog)> {
    let file = build_dir(s, m).join("gradle/libs.versions.toml");
    let text = host.read_to_string(&file).ok()?;
    Some((file, parse_catalog(&text)))
}

/// The catalog line of the library `group:name`, when the build's catalog declares it.
pub fn catalog_place(host: &dyn Host, s: &Structure, m: &ModuleInfo, group: &str, name: &str) -> Option<Place> {
    let (file, catalog) = catalog_of(host, s, m)?;
    let lib = catalog.libraries.iter().find(|l| l.group == group && l.name == name)?;
    Some(Place { file, line: Some(lib.line) })
}

/// Where `target` of the module `m` is declared: the line that says it, else the file it
/// belongs in.
pub fn find(host: &dyn Host, s: &Structure, m: &ModuleInfo, target: &Target) -> Option<Place> {
    if m.build_file.as_ref().is_some_and(|f| f.file_name().is_some_and(|n| n == "pom.xml")) {
        return find_in_poms(host, s, m, target);
    }
    let dir = build_dir(s, m);
    let root_script = build_file(host, &dir);
    // The module's own script, the scripts it applies, then the build's root script (which
    // may set things for every subproject).
    let mut scripts: Vec<PathBuf> = m.build_file.iter().cloned().collect();
    scripts.extend(m.applied.iter().cloned());
    if let Some(r) = &root_script
        && !scripts.contains(r)
    {
        scripts.push(r.clone());
    }
    let read = |f: &Path| host.read_to_string(f).ok();
    match target {
        Target::GradleVersion => {
            let file = dir.join("gradle/wrapper/gradle-wrapper.properties");
            let text = read(&file)?;
            Some(Place { line: wrapper_line(&text), file })
        }
        Target::Include => {
            let file = settings_file(host, &dir).or_else(|| settings_file(host, &s.root))?;
            let text = read(&file).unwrap_or_default();
            Some(Place { line: include_line(&text, &m.gradle_path, m.included.as_deref()), file })
        }
        _ => {
            let catalog = match target {
                Target::Library { .. } => catalog_of(host, s, m).map(|(_, c)| c),
                _ => None,
            };
            for file in &scripts {
                let Some(text) = read(file) else { continue };
                let line = match target {
                    Target::Library { group, name } => library_line(&text, group, name, catalog.as_ref()),
                    Target::Project { path } => project_line(&text, path),
                    Target::Level => level_line(&text),
                    Target::Plugin(id) => plugin_line(&text, id),
                    Target::GradleVersion | Target::Include => None,
                };
                if line.is_some() {
                    return Some(Place { file: file.clone(), line });
                }
            }
            scripts.into_iter().next().map(|file| Place { file, line: None })
        }
    }
}

// ----- Maven ---------------------------------------------------------------------------------

/// The line of the `<artifactId>` of the first `<tag>` block (a dependency, a plugin) that
/// names `artifact`, and `group` when given; lines from 0.
pub fn pom_block_line(pom: &str, tag: &str, group: Option<&str>, artifact: &str) -> Option<u32> {
    let lines: Vec<&str> = pom.lines().collect();
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    let mut i = 0;
    while i < lines.len() {
        if lines[i].contains(&open) {
            let start = i;
            while i < lines.len() && !lines[i].contains(&close) {
                i += 1;
            }
            let block = &lines[start..=i.min(lines.len() - 1)];
            let tagged =
                |name: &str, value: &str| block.iter().position(|l| l.contains(&format!("<{name}>{value}</{name}>")));
            if let Some(at) = tagged("artifactId", artifact)
                && group.is_none_or(|g| {
                    tagged("groupId", g).is_some() || (g == "org.apache.maven.plugins" && tag == "plugin")
                })
            {
                return Some((start + at) as u32);
            }
        }
        i += 1;
    }
    None
}

/// The line that sets the Java level in a POM: the compiler properties, else the compiler
/// plugin's `release` or `source`.
pub fn pom_level_line(pom: &str) -> Option<u32> {
    first_line(pom, |l| {
        let l = l.trim_start();
        ["<maven.compiler.release>", "<maven.compiler.source>", "<java.version>", "<release>", "<source>"]
            .iter()
            .any(|t| l.starts_with(t))
    })
}

/// The `<module>` line that lists `name` in a POM.
pub fn pom_module_line(pom: &str, name: &str) -> Option<u32> {
    first_line(pom, |l| {
        l.trim().strip_prefix("<module>").and_then(|r| r.strip_suffix("</module>")).is_some_and(|m| {
            let m = m.trim().trim_end_matches('/');
            m == name || m.rsplit('/').next() == Some(name)
        })
    })
}

/// Where `target` of a Maven module is declared: its own POM, then its parents' in the build
/// (dependency management, plugin management, properties), up to the build's root POM.
fn find_in_poms(host: &dyn Host, s: &Structure, m: &ModuleInfo, target: &Target) -> Option<Place> {
    let own = m.build_file.clone()?;
    let root = s.build_of(m).map(|b| b.root_dir.clone()).unwrap_or_else(|| s.root.clone());
    let mut poms = vec![own.clone()];
    let mut dir = m.dir.parent().map(Path::to_path_buf);
    while let Some(d) = dir {
        if !d.starts_with(&root) {
            break;
        }
        let pom = d.join("pom.xml");
        if host.is_file(&pom) {
            poms.push(pom);
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    let read = |f: &Path| host.read_to_string(f).ok();
    match target {
        Target::GradleVersion => {
            let file = root.join(".mvn/wrapper/maven-wrapper.properties");
            match read(&file) {
                Some(text) => {
                    Some(Place { line: first_line(&text, |l| l.trim_start().starts_with("distributionUrl")), file })
                }
                None => Some(Place { file: root.join("pom.xml"), line: None }),
            }
        }
        Target::Include => {
            let name = m.dir.file_name()?.to_string_lossy().into_owned();
            for pom in poms.iter().skip(1) {
                if let Some(text) = read(pom)
                    && let Some(line) = pom_module_line(&text, &name)
                {
                    return Some(Place { file: pom.clone(), line: Some(line) });
                }
            }
            Some(Place { file: own, line: None })
        }
        _ => {
            for pom in &poms {
                let Some(text) = read(pom) else { continue };
                let line = match target {
                    Target::Library { group, name } => pom_block_line(&text, "dependency", Some(group), name),
                    Target::Project { path } => pom_block_line(&text, "dependency", None, path.trim_start_matches(':')),
                    Target::Level => pom_level_line(&text),
                    Target::Plugin(id) => {
                        let (g, a) = id.split_once(':').unwrap_or(("", id.as_str()));
                        pom_block_line(&text, "plugin", (!g.is_empty()).then_some(g), a)
                    }
                    Target::GradleVersion | Target::Include => None,
                };
                if line.is_some() {
                    return Some(Place { file: pom.clone(), line });
                }
            }
            Some(Place { file: own, line: None })
        }
    }
}

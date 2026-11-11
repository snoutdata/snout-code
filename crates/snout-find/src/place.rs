//! What part of a project a file is: its own code, its tests, a source folder beside it, or
//! what a build or a package manager put there. A search ranks them in that order.

/// Worst first, so `a > b` means `a` ranks higher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Place {
    /// Made by a build or fetched by a package manager: `build/`, `target/`, `node_modules/`.
    Generated,
    /// A source folder outside the project's (`../shared/src/A.java`).
    Outside,
    /// A test: under `test/` or `__tests__/`, or named `FooTest.java`, `foo.spec.ts`.
    Test,
    /// The project's own code, and everything else.
    Source,
}

/// Folders a build writes or a package manager fills. One under `src/` is a package, not that
/// (`src/main/java/com/acme/build/`).
const GENERATED: &[&str] = &[
    "build",
    "target",
    "out",
    "dist",
    "node_modules",
    "bower_components",
    "vendor",
    "generated",
    "generated-sources",
    "generated-test-sources",
    "coverage",
    ".gradle",
    ".mvn",
    ".next",
    ".nuxt",
    ".angular",
    ".svelte-kit",
    ".turbo",
    ".cache",
    "__pycache__",
];

const TESTS: &[&str] = &["test", "tests", "__tests__", "androidTest", "testFixtures", "integrationTest", "spec", "e2e"];

impl Place {
    /// The place of `path`, from the project's folder with `/` between folders.
    pub fn of(path: &str) -> Place {
        if path.starts_with("../") {
            return Place::Outside;
        }
        let mut parts: Vec<&str> = path.split('/').collect();
        let name = parts.pop().unwrap_or("");
        let mut in_src = false;
        let mut test = false;
        for p in &parts {
            if !in_src && GENERATED.contains(p) {
                return Place::Generated;
            }
            in_src |= *p == "src";
            test |= TESTS.contains(p);
        }
        let stem = name.split('.').next().unwrap_or(name);
        test |= ["Test", "Tests", "IT", "Spec"].iter().any(|s| stem.len() > s.len() && stem.ends_with(s))
            || name.contains(".test.")
            || name.contains(".spec.")
            || stem.ends_with("_test")
            || stem.starts_with("test_");
        if test { Place::Test } else { Place::Source }
    }
}

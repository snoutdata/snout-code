use std::fs;
use std::path::{Path, PathBuf};

/// Directories that are large and never contain repositories worth listing.
const SKIP: &[&str] = &["node_modules", "target", "vendor", "build", "dist", "Library", "Pods"];

/// Find Git working trees under `root`, descending at most `max_depth` levels.
///
/// A repository's own subdirectories are not searched (so submodules and vendored
/// checkouts are not listed separately). Hidden directories and symlinks are skipped.
pub fn find_repos(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, max_depth, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        found.push(dir.to_owned());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        // `file_type` does not follow symlinks.
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP.contains(&name.as_ref()) {
            continue;
        }
        walk(&entry.path(), depth - 1, found);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_repos_but_not_inside_repos() {
        let root = std::env::temp_dir().join(format!("snout-discover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for dir in
            ["a/.git", "group/b/.git", "group/b/sub/c/.git", "node_modules/d/.git", ".hidden/e/.git", "x/y/z/w/f/.git"]
        {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let found: Vec<String> = find_repos(&root, 3)
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(found, ["a", "group/b"]);
        fs::remove_dir_all(&root).unwrap();
    }
}

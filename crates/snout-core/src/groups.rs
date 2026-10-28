//! Named groups of repositories in the repository list.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A named set of repositories in the list. A repository is in one group at most.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoGroup {
    pub name: String,
    pub repos: Vec<PathBuf>,
}

/// Put `repo` in the group named `group`, or in none.
pub fn move_to_group(groups: &mut [RepoGroup], repo: &Path, group: Option<&str>) {
    for g in groups.iter_mut() {
        g.repos.retain(|p| p != repo);
    }
    if let Some(g) = group.and_then(|name| groups.iter_mut().find(|g| g.name == name)) {
        g.repos.push(repo.to_owned());
    }
}

/// Why `name` cannot name a group (other than the one being renamed), if it cannot.
pub fn group_name_problem(groups: &[RepoGroup], name: &str, renaming: Option<&str>) -> Option<&'static str> {
    let taken = groups.iter().any(|g| g.name.eq_ignore_ascii_case(name) && Some(g.name.as_str()) != renaming);
    taken.then_some("There is already a group with this name")
}

/// Create the group `name`, or rename `rename` to it, and put `repo` in it if given.
pub fn save_group(groups: &mut Vec<RepoGroup>, rename: Option<&str>, name: &str, repo: Option<&Path>) {
    match rename.and_then(|old| groups.iter_mut().find(|g| g.name == old)) {
        Some(g) => g.name = name.to_owned(),
        None => groups.push(RepoGroup { name: name.to_owned(), repos: Vec::new() }),
    }
    if let Some(repo) = repo {
        move_to_group(groups, repo, Some(name));
    }
}

/// The last part of a path, for lists.
pub fn dir_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// The list as shown: groups sorted by name with their (listed) repositories sorted, and
/// the repositories in no group, sorted.
pub fn arrange(repos: &[PathBuf], groups: &[RepoGroup]) -> (Vec<RepoGroup>, Vec<PathBuf>) {
    let by_name = |a: &PathBuf, b: &PathBuf| dir_name(a).to_lowercase().cmp(&dir_name(b).to_lowercase());
    let mut groups: Vec<RepoGroup> = groups.to_vec();
    groups.sort_by_key(|g| g.name.to_lowercase());
    for g in &mut groups {
        g.repos.retain(|p| repos.contains(p));
        g.repos.sort_by(by_name);
    }
    let mut loose: Vec<PathBuf> =
        repos.iter().filter(|p| !groups.iter().any(|g| g.repos.contains(p))).cloned().collect();
    loose.sort_by(by_name);
    (groups, loose)
}

/// `s` cut to `max` characters, with an ellipsis when cut.
pub fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_owned() } else { s.chars().take(max - 1).chain(['…']).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repository_lives_in_one_group_at_most() {
        let mut groups = vec![
            RepoGroup { name: "Work".into(), repos: vec!["/a".into()] },
            RepoGroup { name: "Home".into(), repos: Vec::new() },
        ];
        move_to_group(&mut groups, Path::new("/a"), Some("Home"));
        assert!(groups[0].repos.is_empty() && groups[1].repos == [PathBuf::from("/a")]);
        move_to_group(&mut groups, Path::new("/a"), None);
        assert!(groups.iter().all(|g| g.repos.is_empty()));
        assert!(group_name_problem(&groups, "work", None).is_some());
        assert!(group_name_problem(&groups, "Work", Some("Work")).is_none());
        assert_eq!(short("feature/a-long-name", 8), "feature…");
    }

    #[test]
    fn arranging_sorts_groups_and_leaves_the_rest_loose() {
        let repos: Vec<PathBuf> = ["/x/zeta", "/x/Alpha", "/x/mid"].map(PathBuf::from).to_vec();
        let mut groups = vec![RepoGroup { name: "b".into(), repos: Vec::new() }];
        save_group(&mut groups, None, "A", Some(Path::new("/x/mid")));
        save_group(&mut groups, Some("b"), "B", None);
        let (groups, loose) = arrange(&repos, &groups);
        assert_eq!(groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(groups[0].repos, [PathBuf::from("/x/mid")]);
        assert_eq!(loose, [PathBuf::from("/x/Alpha"), PathBuf::from("/x/zeta")]);
    }
}

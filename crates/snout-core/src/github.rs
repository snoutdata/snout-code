/// Map a GitHub remote URL (HTTPS, SSH or scp-style) to the repository's web page.
///
/// Returns `None` for non-GitHub remotes.
pub fn github_web_url(remote_url: &str) -> Option<String> {
    let url = remote_url.trim();
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| {
            // https://user@github.com/owner/repo
            let rest = url.strip_prefix("https://")?;
            rest.split_once("@github.com/").map(|(_, p)| p)
        })?;
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("https://github.com/{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::github_web_url;

    #[test]
    fn maps_common_forms() {
        for url in [
            "git@github.com:joel/snout.git",
            "ssh://git@github.com/joel/snout.git",
            "https://github.com/joel/snout",
            "https://github.com/joel/snout.git",
            "https://token@github.com/joel/snout.git",
        ] {
            assert_eq!(github_web_url(url).as_deref(), Some("https://github.com/joel/snout"), "{url}");
        }
        assert_eq!(github_web_url("https://gitlab.com/joel/snout.git"), None);
    }
}

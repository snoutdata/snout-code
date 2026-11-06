//! Versions: our tags are plain `x.y.z`.

/// Whether `remote` is newer than `current`. Segments compare as numbers, so 1.0.10 is newer than
/// 1.0.9; a missing or unreadable segment reads as 0.
pub fn is_newer(remote: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> { v.trim().split('.').map(|n| n.parse().unwrap_or(0)).collect() };
    let (r, c) = (parse(remote), parse(current));
    for i in 0..r.len().max(c.len()) {
        let (a, b) = (r.get(i).copied().unwrap_or(0), c.get(i).copied().unwrap_or(0));
        if a != b {
            return a > b;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(is_newer("1.0.10", "1.0.9"));
        assert!(is_newer("1.1", "1.0.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.9", "1.0.10"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0", "1.0.0"), "a missing segment is 0");
        assert!(!is_newer("nonsense", "1.0.0"));
    }
}

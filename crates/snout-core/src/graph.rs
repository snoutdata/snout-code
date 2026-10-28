//! Lanes for the commit graph beside the Journal: which lane each commit sits in, and the
//! lines that join it to its children and parents. Drawing is up to the front end.

use crate::Commit;

/// One row of the graph. Each edge joins two lanes over half a row: `top` edges run from the
/// top of the row to the commit's height, the others from there to the bottom.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct GraphRow {
    pub lane: usize,
    pub edges: Vec<Edge>,
    /// Lanes in use on this row.
    pub width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub top: bool,
}

/// Lay out `commits` (children before parents, as `git log --date-order` lists them).
pub fn layout(commits: &[Commit]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<&str>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());
    for c in commits {
        let id = c.id.as_str();
        let free = |lanes: &mut Vec<Option<&str>>| match lanes.iter().position(Option::is_none) {
            Some(i) => i,
            None => {
                lanes.push(None);
                lanes.len() - 1
            }
        };
        let lane = match lanes.iter().position(|l| *l == Some(id)) {
            Some(i) => i,
            None => free(&mut lanes),
        };
        let mut edges = Vec::new();
        // Lines arriving from above: every branch waiting for this commit converges on it,
        // the rest pass by.
        let mut passing = vec![false; lanes.len()];
        for (i, l) in lanes.iter().enumerate() {
            match l {
                Some(x) if *x == id => edges.push(Edge { from: i, to: lane, top: true }),
                Some(_) => {
                    edges.push(Edge { from: i, to: i, top: true });
                    passing[i] = true;
                }
                None => {}
            }
        }
        for l in lanes.iter_mut().filter(|l| **l == Some(id)) {
            *l = None;
        }
        // Lines leaving downwards, to each parent: the first parent keeps this lane unless
        // another branch is already heading for it.
        for (k, parent) in c.parents.iter().enumerate() {
            let to = match lanes.iter().position(|l| *l == Some(parent.as_str())) {
                Some(j) => j,
                None => {
                    let j = if k == 0 { lane } else { free(&mut lanes) };
                    lanes[j] = Some(parent.as_str());
                    j
                }
            };
            edges.push(Edge { from: lane, to, top: false });
        }
        for (i, pass) in passing.iter().enumerate() {
            if *pass {
                edges.push(Edge { from: i, to: i, top: false });
            }
        }
        while lanes.last() == Some(&None) {
            lanes.pop();
        }
        let width = edges.iter().map(|e| e.from.max(e.to)).chain([lane]).max().unwrap_or(0) + 1;
        rows.push(GraphRow { lane, edges, width });
    }
    rows
}

/// Lanes the graph needs, capped at 12 (wider graphs are clipped).
pub fn lane_count(rows: &[GraphRow]) -> usize {
    rows.iter().map(|r| r.width).max().unwrap_or(1).min(12)
}

/// Whether the Journal filter `needle` (already lower-cased and trimmed) matches `c`: its
/// message, author, SHA prefix or one of its refs.
pub fn journal_matches(c: &Commit, needle: &str) -> bool {
    needle.is_empty()
        || c.summary.to_lowercase().contains(needle)
        || c.author.to_lowercase().contains(needle)
        || c.id.starts_with(needle)
        || c.refs.iter().any(|r| r.to_lowercase().contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: id.into(),
            short_id: id.into(),
            author: String::new(),
            email: String::new(),
            time: 0,
            date: String::new(),
            refs: Vec::new(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            summary: String::new(),
            unpushed: false,
        }
    }

    #[test]
    fn a_branch_and_its_merge_take_a_second_lane() {
        // m merges b into a; both come from root.
        let rows = layout(&[commit("m", &["a", "b"]), commit("a", &["r"]), commit("b", &["r"]), commit("r", &[])]);
        let lanes: Vec<usize> = rows.iter().map(|r| r.lane).collect();
        assert_eq!(lanes, [0, 0, 1, 0]);
        assert!(rows[0].edges.contains(&Edge { from: 0, to: 1, top: false }), "merge forks right");
        assert!(rows[1].edges.contains(&Edge { from: 1, to: 1, top: true }), "b's lane passes a");
        assert!(rows[2].edges.contains(&Edge { from: 1, to: 0, top: false }), "b heads back to the root's lane");
        assert_eq!(rows[3].width, 1);
        assert_eq!(lane_count(&rows), 2);
    }

    #[test]
    fn a_straight_history_is_one_lane() {
        let rows = layout(&[commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])]);
        assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1));
        assert!(rows[2].edges.iter().all(|e| e.top), "nothing below the root");
    }

    #[test]
    fn journal_filter_matches_message_author_sha_and_refs() {
        let mut c = commit("abc123", &[]);
        c.summary = "Fix the Thing".into();
        c.refs = vec!["tag: v1.0".into()];
        for needle in ["", "thing", "abc", "v1.0"] {
            assert!(journal_matches(&c, needle), "{needle}");
        }
        assert!(!journal_matches(&c, "nothing"));
    }
}

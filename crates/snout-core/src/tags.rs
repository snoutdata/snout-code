use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tag {
    pub name: String,
    /// Short id of the tagged commit (peeled, for annotated tags).
    pub target: String,
    pub subject: String,
    /// Creation date, `YYYY-MM-DD`.
    pub date: String,
}

pub(crate) const FORMAT: &str = "--format=%(refname:short)%00%(objectname:short)%00%(*objectname:short)%00%(contents:subject)%00%(creatordate:short)";

pub(crate) fn parse(raw: &str) -> Vec<Tag> {
    raw.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.splitn(5, '\0').collect();
            (f.len() == 5).then(|| Tag {
                name: f[0].to_owned(),
                // Lightweight tags point at the commit directly; annotated ones need peeling.
                target: if f[2].is_empty() { f[1] } else { f[2] }.to_owned(),
                subject: f[3].to_owned(),
                date: f[4].to_owned(),
            })
        })
        .collect()
}

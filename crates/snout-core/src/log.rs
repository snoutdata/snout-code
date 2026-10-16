use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub email: String,
    /// Unix timestamp of the author date.
    pub time: i64,
    /// Author date formatted in local time, `YYYY-MM-DD HH:MM`.
    pub date: String,
    /// Decorations such as `HEAD -> main`, `origin/main`, `tag: v1.0`.
    pub refs: Vec<String>,
    pub summary: String,
    /// Reachable from HEAD but not from its upstream (i.e. would be pushed).
    pub unpushed: bool,
}

pub(crate) const FORMAT: &str = "--format=%H%x00%h%x00%an%x00%ae%x00%at%x00%ad%x00%D%x00%s%x1e";
pub(crate) const DATE: &str = "--date=format-local:%Y-%m-%d %H:%M";

pub(crate) fn parse(raw: &str) -> Vec<Commit> {
    raw.split('\x1e')
        .map(|r| r.trim_start_matches('\n'))
        .filter(|r| !r.is_empty())
        .filter_map(|r| {
            let f: Vec<&str> = r.splitn(8, '\0').collect();
            (f.len() == 8).then(|| Commit {
                id: f[0].to_owned(),
                short_id: f[1].to_owned(),
                author: f[2].to_owned(),
                email: f[3].to_owned(),
                time: f[4].parse().unwrap_or(0),
                date: f[5].to_owned(),
                refs: f[6].split(", ").filter(|s| !s.is_empty()).map(str::to_owned).collect(),
                summary: f[7].to_owned(),
                unpushed: false,
            })
        })
        .collect()
}

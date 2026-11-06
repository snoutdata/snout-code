//! When an update fails: whose problem it is, and whether anything is expected of the reader.
//! An error's own text is never what a person reads; it goes to the log, and its useful line to
//! the tooltip.

/// What kind of failure this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// This computer cannot reach anything: asleep, offline, a captive portal.
    Offline,
    /// The feed answered, badly: a 5xx, or a body that is not the feed. Ours, not theirs.
    FeedUnavailable,
    /// GitHub is rate limiting this address, which a shared office network can do to everyone.
    RateLimited,
    /// The feed is reachable and has no release to offer.
    NoRelease,
    /// A download or the feed did not match what it should be. The one kind that is alarming.
    Signature,
    /// Nowhere to put the download.
    Disk,
    Unknown,
}

/// A failure as the app shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub kind: Kind,
    /// One sentence, addressed to the person, saying whose problem it is and what happens next.
    pub message: String,
    /// The useful line of the original error, for a tooltip and the log.
    pub detail: Option<String>,
}

impl Kind {
    pub fn message(self) -> &'static str {
        match self {
            Kind::Offline => "Updates could not be checked: this computer is offline. It will try again later.",
            Kind::FeedUnavailable => {
                "Updates could not be checked: GitHub is not answering. Nothing is wrong with your installation, and it will try again later."
            }
            Kind::RateLimited => {
                "Updates could not be checked: GitHub is rate limiting this network. It will try again later."
            }
            Kind::NoRelease => "Updates could not be checked: no published release was found to update to.",
            Kind::Signature => {
                "An update was downloaded and refused: it did not match the signature it should have. Nothing was installed."
            }
            Kind::Disk => "The update could not be saved: there is not enough disk space.",
            Kind::Unknown => "Updates could not be checked.",
        }
    }
}

/// The first line of an error, capped.
pub fn detail_of(raw: &str) -> Option<String> {
    let first = raw.lines().map(str::trim).find(|l| !l.is_empty())?;
    let one = first.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(if one.chars().count() > 200 { one.chars().take(199).chain(['…']).collect() } else { one })
}

/// What kind of failure `raw` (curl's words, or ours) with the HTTP status `status` is, and the
/// sentence for it. The order is the logic, since the messages overlap: a machine with no network
/// produces everything, so offline is judged first; a checksum failure is about the file, the
/// request having worked, so it comes before the transport.
pub fn describe(raw: &str, status: Option<u16>) -> Failure {
    let kind = kind_of(raw, status);
    Failure { kind, message: kind.message().to_owned(), detail: detail_of(raw) }
}

fn kind_of(raw: &str, status: Option<u16>) -> Kind {
    let text = raw.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| text.contains(w));
    if any(&[
        "could not resolve",
        "failed to connect",
        "couldn't connect",
        "connection refused",
        "network is unreachable",
        "timed out",
        "connection reset",
        "ssl connect error",
        "recv failure",
    ]) {
        return Kind::Offline;
    }
    if any(&["no space left", "not enough space", "failure writing output", "failed writing"]) {
        return Kind::Disk;
    }
    if any(&["signature", "checksum", "sha-256", "not signed"]) {
        return Kind::Signature;
    }
    if status == Some(429) || status == Some(403) || text.contains("rate limit") {
        return Kind::RateLimited;
    }
    if status.is_some_and(|s| s >= 500) || any(&["bad gateway", "service unavailable", "not the feed"]) {
        return Kind::FeedUnavailable;
    }
    if status == Some(404) {
        return Kind::NoRelease;
    }
    Kind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_failure_says_whose_problem_it_is() {
        assert_eq!(describe("curl: (6) Could not resolve host: github.com", None).kind, Kind::Offline);
        assert_eq!(describe("curl: (28) Operation timed out after 30000 ms", None).kind, Kind::Offline);
        assert_eq!(describe("HTTP 504", Some(504)).kind, Kind::FeedUnavailable);
        assert_eq!(describe("HTTP 429", Some(429)).kind, Kind::RateLimited);
        assert_eq!(describe("HTTP 404", Some(404)).kind, Kind::NoRelease);
        assert_eq!(describe("curl: (23) Failure writing output to destination", None).kind, Kind::Disk);
        assert_eq!(describe("the feed's signature does not verify", None).kind, Kind::Signature);
        assert_eq!(describe("the download's SHA-256 is not the feed's", None).kind, Kind::Signature);
        assert_eq!(describe("something else", None).kind, Kind::Unknown);
    }

    #[test]
    fn a_machine_with_no_network_is_offline_whatever_else_the_error_says() {
        let raw = "curl: (7) Failed to connect to github.com: signature of nothing, HTTP 504";
        assert_eq!(describe(raw, Some(504)).kind, Kind::Offline);
    }

    #[test]
    fn the_detail_is_one_short_line() {
        assert_eq!(detail_of("\n  HTTP 504:   Gateway\nheaders\nmore").as_deref(), Some("HTTP 504: Gateway"));
        assert_eq!(detail_of(&"x".repeat(500)).unwrap().chars().count(), 200);
        assert_eq!(detail_of("  \n "), None);
    }
}

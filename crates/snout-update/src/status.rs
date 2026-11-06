//! Where an update stands, and the words for it.

use crate::failure::Failure;

/// Why the app cannot put a new version in place itself: it says so and offers the download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualReason {
    /// The system's package manager owns the install (Linux).
    PackageManager,
    /// It runs from where it cannot replace itself: a disk image, a folder it cannot write to.
    CannotReplace,
    /// It tried, and the version never landed.
    DidNotLand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Checking,
    Available {
        version: String,
        notes: Option<String>,
    },
    /// `from_source`: a build from the source, which has no feed and asked nothing.
    NotAvailable {
        version: String,
        from_source: bool,
    },
    /// `preparing`: the download is done and the update is being made ready beside the live app.
    Downloading {
        percent: Option<u32>,
        preparing: bool,
    },
    /// Ready: a restart, or the next quit, puts it in place.
    Downloaded {
        version: String,
        notes: Option<String>,
    },
    /// A newer version is out, and getting it is up to the user.
    Manual {
        version: String,
        reason: ManualReason,
    },
    Error(Failure),
}

/// A build from the source has no update feed.
pub const FROM_SOURCE_TEXT: &str =
    "This build was made from source and does not update itself. Build it again from the latest source to update.";

impl UpdateStatus {
    /// The sentence for the toast.
    pub fn text(&self) -> String {
        match self {
            Self::Checking => "Checking for updates…".into(),
            Self::Available { version, .. } => format!("Update found (v{version}), downloading…"),
            Self::Downloading { preparing: true, .. } => "Preparing update…".into(),
            Self::Downloading { .. } => "Downloading update…".into(),
            Self::Downloaded { version, .. } => format!("Update ready: v{version}"),
            Self::NotAvailable { from_source: true, .. } => FROM_SOURCE_TEXT.into(),
            Self::NotAvailable { version, .. } => format!("You're up to date (v{version})"),
            Self::Manual { version, reason: ManualReason::PackageManager } => {
                format!("SnoutCode v{version} is available.")
            }
            Self::Manual { version, reason: ManualReason::CannotReplace } => format!(
                "SnoutCode v{version} is available, and cannot install itself from where it runs. Move it to your applications, or download it."
            ),
            Self::Manual { version, reason: ManualReason::DidNotLand } => {
                format!("SnoutCode v{version} did not install. Download it to install it by hand.")
            }
            // Already a sentence (see `failure`).
            Self::Error(failure) => failure.message.clone(),
        }
    }

    /// Work in the background, for the status bar: its stage and how far it is.
    pub fn task(&self) -> Option<(String, Option<u32>)> {
        match self {
            Self::Available { .. } => Some(("Downloading update".into(), Some(0))),
            Self::Downloading { preparing: true, .. } => Some(("Preparing update".into(), None)),
            Self::Downloading { percent, .. } => Some(("Downloading update".into(), *percent)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_say_where_it_stands() {
        let ready = UpdateStatus::Downloaded { version: "1.0.1".into(), notes: None };
        assert_eq!(ready.text(), "Update ready: v1.0.1");
        assert!(ready.task().is_none());
        let up = UpdateStatus::NotAvailable { version: "1.0.0".into(), from_source: false };
        assert_eq!(up.text(), "You're up to date (v1.0.0)");
        let source = UpdateStatus::NotAvailable { version: "1.0.0".into(), from_source: true };
        assert!(source.text().contains("made from source"));
        let got = UpdateStatus::Downloading { percent: Some(42), preparing: false };
        assert_eq!(got.task(), Some(("Downloading update".into(), Some(42))));
        let prep = UpdateStatus::Downloading { percent: None, preparing: true };
        assert_eq!(prep.task(), Some(("Preparing update".into(), None)));
    }
}

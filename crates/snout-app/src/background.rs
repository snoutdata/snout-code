//! What runs in the background after the window shows: snout.sh's engine build (in
//! development), each Java server's stages (reading the Gradle model, compiling Scala,
//! starting Java, indexing) and Git's jobs. The status bar shows the one in front, with a
//! thin progress bar; a click lists them all.

use std::path::Path;

use gpui::App;

use crate::lang::LangHub;
use crate::lang::hub::Stage;

/// The variable that names the engine build's status file. snout.sh starts the app first
/// and builds the engine beside it, writing a line per step: `stage <what it does>`, then
/// `done`, or `failed <why>`.
pub const STATUS_ENV: &str = "SNOUT_ENGINE_BUILD_STATUS";

/// The engine build as its status file tells it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EngineBuild {
    /// What it does now.
    pub stage: String,
    pub done: bool,
    pub failed: Option<String>,
}

impl EngineBuild {
    pub fn parse(text: &str) -> Self {
        let mut b = EngineBuild { stage: "Building snout-engine".into(), ..Default::default() };
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if let Some(stage) = line.strip_prefix("stage ") {
                b.stage = stage.trim().to_owned();
            } else if line == "done" {
                b.done = true;
            } else if let Some(why) = line.strip_prefix("failed") {
                b.failed = Some(why.trim().to_owned());
            }
        }
        b
    }

    pub fn read(file: &Path) -> Self {
        Self::parse(&std::fs::read_to_string(file).unwrap_or_default())
    }

    /// The build is over, one way or the other.
    pub fn finished(&self) -> bool {
        self.done || self.failed.is_some()
    }
}

/// A task running in the background: what it is, what it does now, and how far along it is
/// when that is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgTask {
    pub title: String,
    pub stage: String,
    pub percent: Option<u32>,
}

/// A Java server's stage, as the background tasks name it.
pub fn stage_text(stage: &Stage) -> String {
    match stage {
        Stage::Importing => "Reading the Gradle model".into(),
        Stage::CompilingScala => "Compiling Scala".into(),
        Stage::Starting => "Starting Java".into(),
        Stage::Indexing => "Indexing".into(),
        Stage::Restarting => "Restarting Java".into(),
        Stage::WaitingForEngine => "Waiting for snout-engine".into(),
        other => other.label(),
    }
}

/// Everything running in the background now, the engine build first.
pub fn tasks(hub: Option<&gpui::Entity<LangHub>>, model: &crate::model::Model, cx: &App) -> Vec<BgTask> {
    let mut out = Vec::new();
    if let Some(hub) = hub {
        let hub = hub.read(cx);
        if let Some(b) = hub.engine_build.as_ref().filter(|b| !b.finished()) {
            out.push(BgTask { title: "Building snout-engine".into(), stage: b.stage.clone(), percent: None });
        }
        let mut servers: Vec<_> = hub.servers.values().filter(|s| s.busy()).collect();
        servers.sort_by(|a, b| a.root.cmp(&b.root));
        for s in servers {
            let work = s.progress.current();
            let stage = match (&s.stage, work) {
                (Stage::Indexing | Stage::Ready, Some(w)) => w.label(),
                (stage, _) => stage_text(stage),
            };
            out.push(BgTask {
                title: format!("Java: {}", crate::model::Model::dir_name(&s.root)),
                stage,
                percent: work.and_then(|w| w.percentage),
            });
        }
    }
    if let Some(task) = crate::project_model::store(cx).and_then(|s| s.read(cx).task()) {
        out.push(task);
    }
    if let Some(runs) = crate::run::runs(cx) {
        out.extend(runs.read(cx).tasks());
    }
    if let Some(job) = &model.job {
        out.push(BgTask { title: "Git".into(), stage: job.clone(), percent: None });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_file_says_what_the_build_does() {
        let b = EngineBuild::parse("stage Downloading JDK 21\nstage Building snout-engine\n");
        assert_eq!(b.stage, "Building snout-engine");
        assert!(!b.finished());
        assert!(EngineBuild::parse("stage x\ndone\n").done);
        let failed = EngineBuild::parse("stage x\nfailed Gradle stopped\n");
        assert_eq!(failed.failed.as_deref(), Some("Gradle stopped"));
        assert!(failed.finished());
        assert_eq!(EngineBuild::parse("").stage, "Building snout-engine");
        assert_eq!(stage_text(&Stage::Importing), "Reading the Gradle model");
        assert_eq!(stage_text(&Stage::Starting), "Starting Java");
    }
}

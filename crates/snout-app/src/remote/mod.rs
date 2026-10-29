//! Remote SSH: the window on this Mac, the project on another machine reached with the
//! system's ssh, where `snout-server` (installed by the app) serves it as a `snout_host::Host`.

use std::path::{Path, PathBuf};

/// The folder of the Linux builds of `snout-server` the app installs on remote machines:
/// `SNOUT_SERVER_DIST`, else the app bundle's `Contents/Resources/server`, else (development)
/// `target/snout-server-dist` of the workspace the app was built in, made by
/// `scripts/build-server-linux.sh`.
pub fn server_binaries() -> Option<PathBuf> {
    let has =
        |d: &Path| d.join("snout-server-linux-x86_64").is_file() || d.join("snout-server-linux-aarch64").is_file();
    if let Some(d) = std::env::var_os("SNOUT_SERVER_DIST").map(PathBuf::from) {
        return has(&d).then_some(d);
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1) {
            let candidate = dir.join("Resources/server");
            if has(&candidate) {
                return Some(candidate);
            }
        }
    }
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snout-server-dist");
    has(&built).then(|| built.canonicalize().unwrap_or(built))
}

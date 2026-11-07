//! The real thing: Windows with a WSL distro (`SNOUT_WSL_DISTRO`) and the Linux servers
//! (`SNOUT_SERVER_BINARIES`). Installs the server through `wsl.exe`, connects over its stdin and
//! stdout, and finds an agent that is only in the Linux profile. Run by CI; see `.github/workflows/windows.yml`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use snout_host::Host as _;
use snout_remote::{RemoteHost, RemoteSession, SshTarget};

#[test]
#[ignore = "needs Windows with a WSL distro: SNOUT_WSL_DISTRO and SNOUT_SERVER_BINARIES"]
fn a_distro_connects_like_any_remote_machine() {
    let distro = std::env::var("SNOUT_WSL_DISTRO").expect("SNOUT_WSL_DISTRO");
    let binaries = PathBuf::from(std::env::var("SNOUT_SERVER_BINARIES").expect("SNOUT_SERVER_BINARIES"));
    let session = RemoteSession::open(SshTarget::wsl(&distro), &binaries).expect("the server installs and connects");
    assert!(session.installed().path.contains(".snout-code/server"));
    let client = session.client().unwrap();
    let release = client.read_to_string("/etc/os-release").expect("files are Linux's");
    assert!(release.contains("Ubuntu"), "{release}");
    let host = RemoteHost::connect(Arc::new(session)).expect("the host");
    let info = host.info();
    assert_eq!(info.os, "linux");
    assert!(info.home.starts_with("/"), "{:?}", info.home);
    // Amp as its installer leaves it, in the Linux profile.
    assert!(host.is_file(&info.home.join(".amp/bin/amp")), "the agent is seen");
    assert!(host.is_dir(Path::new("/tmp")));
    // It comes back: a second session finds the server already there.
    let again = RemoteSession::open(SshTarget::wsl(&distro), &binaries).unwrap();
    assert!(!again.installed().uploaded, "it is not uploaded twice");
    host.close();
}

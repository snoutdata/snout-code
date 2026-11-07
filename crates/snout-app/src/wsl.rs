//! A project inside WSL, opened from Windows (`\\\\wsl.localhost\\Ubuntu\\home\\ann\\app`). The window stays
//! on Windows and everything else runs inside the distro, as on a machine of Remote SSH: the server
//! there owns the files, Git, the terminals, the agents and the Java engine, and `wsl.exe` carries the
//! connection in place of `ssh` (see `snout_remote::SshTarget::wsl`).

use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WslProject {
    pub distro: String,
    /// The folder as Linux names it.
    pub path: String,
}

impl WslProject {
    /// What the connection to its distro is called.
    pub fn connection(&self) -> String {
        format!("{}{}", snout_remote::WSL_PREFIX, self.distro)
    }
}

/// The project `root` is in, if it is a folder of a WSL distro. On any system: this reads the text.
pub fn of(root: &Path) -> Option<WslProject> {
    let text = root.to_string_lossy().replace('/', "\\");
    let text = text.strip_prefix(r"\\?\UNC\").map(|rest| format!(r"\\{rest}")).unwrap_or(text);
    let lower = text.to_ascii_lowercase();
    let rest =
        [r"\\wsl.localhost\", r"\\wsl$\"].into_iter().find_map(|p| lower.starts_with(p).then(|| &text[p.len()..]))?;
    let mut parts = rest.split('\\').filter(|p| !p.is_empty());
    let distro = parts.next()?.to_owned();
    let path = format!("/{}", parts.collect::<Vec<_>>().join("/"));
    Some(WslProject { distro, path })
}

/// The WSL project of `root` on `host`: only a Windows machine reaches a distro through such a path.
pub fn of_host(host: &dyn snout_host::Host, root: &Path) -> Option<WslProject> {
    if host.info().is_windows() { of(root) } else { None }
}

/// The distros in what `wsl.exe -l -q` prints: UTF-16, one to a line. Docker's own are not for working in.
pub fn distros_in(output: &[u8]) -> Vec<String> {
    let units: Vec<u16> = output.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}').to_owned())
        .filter(|l| !l.is_empty() && !l.starts_with("docker-desktop"))
        .collect()
}

/// The distros of this machine: none where there is no WSL.
pub fn distros() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let mut c = std::process::Command::new("wsl.exe");
    snout_host::default_signals(&mut c);
    c.args(["-l", "-q"]).output().map(|out| distros_in(&out.stdout)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_of_a_distro_is_known_by_either_of_its_names() {
        let want = WslProject { distro: "Ubuntu".into(), path: "/home/ann/new_frontend/frontend".into() };
        for p in [
            r"\\wsl.localhost\Ubuntu\home\ann\new_frontend\frontend",
            r"\\wsl$\Ubuntu\home\ann\new_frontend\frontend",
            r"\\WSL.LOCALHOST\Ubuntu\home\ann\new_frontend\frontend\",
            r"\\?\UNC\wsl.localhost\Ubuntu\home\ann\new_frontend\frontend",
            "//wsl.localhost/Ubuntu/home/ann/new_frontend/frontend",
        ] {
            assert_eq!(of(Path::new(p)).as_ref(), Some(&want), "{p}");
        }
        assert_eq!(want.connection(), "WSL: Ubuntu");
        assert_eq!(of(Path::new(r"\\wsl.localhost\Ubuntu")).unwrap().path, "/");
        for p in [r"C:\Users\ann\app", r"\\server\share\app", "/home/ann/app", r"\\wsl.localhost\"] {
            assert_eq!(of(Path::new(p)), None, "{p}");
        }
    }

    #[test]
    fn the_distros_come_out_of_wsl_exes_utf16() {
        let text = "\u{feff}Ubuntu\r\ndocker-desktop\r\nDebian\r\n\r\n";
        let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(distros_in(&bytes), ["Ubuntu", "Debian"]);
        assert_eq!(distros_in(b""), Vec::<String>::new());
    }
}

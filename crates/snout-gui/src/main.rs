mod app;
mod art;
mod diff_view;
mod graph;
mod terminal;
mod theme;

fn main() -> eframe::Result {
    #[cfg(target_os = "macos")]
    login_path();
    // `snout [path]` opens (and remembers) the repository containing `path`.
    let open_arg = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("SnoutCode")
        .with_inner_size([1400.0, 900.0])
        .with_min_inner_size([800.0, 500.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon/snout-512.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        // winit's own menu bar would replace the one snout puts up.
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_default_menu(false);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "snout",
        options,
        Box::new(move |cc| {
            let mut app = app::SnoutApp::new(cc, open_arg);
            app.install_menu(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

/// Opened from Finder, the app gets launchd's bare PATH, without Homebrew or anything the
/// shell adds, so Git's hooks and credential helpers and Claude Code's tools would go
/// missing. Take the login shell's PATH instead (from a terminal, it is already there).
#[cfg(target_os = "macos")]
fn login_path() {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    const MARK: &str = "__SNOUT_PATH__";
    let path = std::env::var("PATH").unwrap_or_default();
    if path.split(':').any(|d| d == "/opt/homebrew/bin" || d == "/usr/local/bin") {
        return;
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // Interactive too, since many set PATH in .zshrc; the marks skip anything it prints.
        let out = Command::new(shell)
            .args(["-ilc", &format!("printf '{MARK}%s{MARK}' \"$PATH\"")])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let _ = tx.send(out);
    });
    // A shell that hangs costs three seconds, once, and leaves the fallback.
    let found = rx
        .recv_timeout(Duration::from_secs(3))
        .ok()
        .and_then(Result::ok)
        .and_then(|o| String::from_utf8_lossy(&o.stdout).split(MARK).nth(1).map(str::to_owned))
        .filter(|p| !p.is_empty());
    let path = found.unwrap_or_else(|| format!("{path}:/opt/homebrew/bin:/usr/local/bin"));
    // SAFETY: first thing in main; the only other thread is the shell's, done with its env.
    unsafe { std::env::set_var("PATH", path) };
}

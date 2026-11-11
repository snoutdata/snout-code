//! macOS: a global menu bar, the window's own title bar, Finder, and the app bundle.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The menus are the system's menu bar.
pub const IN_WINDOW_MENUS: bool = false;

/// The file manager's command that shows a file there.
pub fn reveal() -> String {
    t!("reveal-mac")
}

/// The machine the app runs on, as messages name it.
pub fn this_machine() -> &'static str {
    ts!("machine-mac")
}

/// The window keeps macOS's own decorations (the app draws in its title bar).
pub fn window_decorations() -> Option<gpui::WindowDecorations> {
    None
}

/// The bundle's `Contents/Resources`: in an ancestor of the program, as `Contents/MacOS/x`.
pub fn resource_dirs(exe: &Path) -> Vec<PathBuf> {
    exe.ancestors().skip(1).map(|d| d.join("Resources")).collect()
}

/// When this process started, by the kernel's record of it.
pub fn process_start() -> Option<SystemTime> {
    // SAFETY: `proc_bsdinfo` is plain data, valid all zeros.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: `info` is a `proc_bsdinfo` of the size given.
    let got = unsafe {
        libc::proc_pidinfo(std::process::id() as libc::c_int, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size)
    };
    (got == size).then(|| {
        SystemTime::UNIX_EPOCH
            + Duration::from_secs(info.pbi_start_tvsec)
            + Duration::from_micros(info.pbi_start_tvusec)
    })
}

/// A long run ended while the app was in the background: the Dock icon bounces once and
/// wears `badge` until the app is in front again.
pub fn attention(badge: &str) {
    use objc2_app_kit::{NSApplication, NSRequestUserAttentionType};
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if app.isActive() {
        return;
    }
    app.requestUserAttention(NSRequestUserAttentionType::InformationalRequest);
    app.dockTile().setBadgeLabel(Some(&objc2_foundation::NSString::from_str(badge)));
}

/// The app is in front: the Dock icon's badge goes.
pub fn clear_attention() {
    use objc2_app_kit::NSApplication;
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    NSApplication::sharedApplication(mtm).dockTile().setBadgeLabel(None);
}

static OPEN_ASKED: std::sync::OnceLock<futures::channel::mpsc::UnboundedSender<Vec<String>>> =
    std::sync::OnceLock::new();

objc2::define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this adds no Drop.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = objc2::MainThreadOnly]
    #[name = "SnoutOpenService"]
    struct OpenService;

    impl OpenService {
        /// `NSMessage` in the bundle's `NSServices` (`scripts/package-mac.sh`).
        #[unsafe(method(openInSnoutCode:userData:error:))]
        fn open_in_snout_code(
            &self,
            board: &objc2_app_kit::NSPasteboard,
            _user_data: Option<&objc2_foundation::NSString>,
            _error: *mut *mut objc2_foundation::NSString,
        ) {
            let kind = objc2_foundation::NSString::from_str("public.file-url");
            let urls: Vec<String> = board
                .pasteboardItems()
                .into_iter()
                .flatten()
                .filter_map(|item| item.stringForType(&kind))
                .map(|url| file_url(url.to_string()))
                .collect();
            if let Some(asked) = OPEN_ASKED.get() {
                let _ = asked.unbounded_send(urls);
            }
        }
    }
);

/// `url` as a path: Finder hands a file over by reference (`file:///.file/id=6571367.1131259`),
/// which names no path until the system resolves it. Any other URL is as it was.
pub fn file_url(url: String) -> String {
    if !url.starts_with("file:///.file/") {
        return url;
    }
    let resolved = objc2_foundation::NSURL::URLWithString(&objc2_foundation::NSString::from_str(&url))
        .and_then(|u| u.filePathURL())
        .and_then(|u| u.absoluteString())
        .map(|s| s.to_string());
    resolved.unwrap_or(url)
}

/// Answer the Services menu's "Open in SnoutCode". The port's name is `NSPortName` in the
/// bundle's `NSServices`.
pub fn offer_open_service(asked: futures::channel::mpsc::UnboundedSender<Vec<String>>) {
    use objc2::{MainThreadOnly as _, msg_send, rc::Retained};
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    let _ = OPEN_ASKED.set(asked);
    // SAFETY: a plain NSObject subclass, initialised on the main thread.
    let provider: Retained<OpenService> = unsafe { msg_send![OpenService::alloc(mtm), init] };
    let name = objc2_foundation::NSString::from_str("SnoutCode");
    // SAFETY: the provider implements the method the bundle's `NSServices` names.
    unsafe { objc2_app_kit::NSRegisterServicesProvider(Some(&provider), &name) };
    // The system keeps no reference: it lives as long as the app.
    std::mem::forget(provider);
}

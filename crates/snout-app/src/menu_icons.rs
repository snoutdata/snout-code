//! Icons in the native macOS menu bar. GPUI builds the bar's `NSMenu` itself and gives its
//! items no images, so after each [`install`] (which hands GPUI the menus) the bar is walked
//! here with AppKit and each item given its image. Items are matched by what they run (the
//! action's name) or, for submenus, by their title, along the same path through the menus
//! GPUI followed to build them, and only when the item's title is the one expected: an item
//! macOS adds itself (Enter Full Screen, the window list) is left as it is.
//!
//! An image is an SF Symbol where one fits, as macOS's own menus use; otherwise the app's
//! Lucide icon, drawn as a template image so it takes the menu's colours. From macOS 27,
//! AppKit hides menu images unless an item asks for them to show, so each item does.
//!
//! `SNOUT_MENU_ICONS=dump` prints every item and the image it got, for checking;
//! `SNOUT_MENU_ICONS=lucide` (or `lucide-dump`) uses the Lucide icons throughout, and `watch`
//! counts the items with an image every two seconds for a while.

use gpui::{App, Menu, MenuItem};

use crate::components::IconName;

/// Hand GPUI the menu bar, then give its items their images.
pub fn install(menus: Vec<Menu>, cx: &App) {
    let plan = plan(&menus);
    cx.set_menus(menus);
    #[cfg(target_os = "macos")]
    mac::decorate(&plan);
    #[cfg(target_os = "macos")]
    if std::env::var("SNOUT_MENU_ICONS").is_ok_and(|v| v.contains("watch")) {
        cx.spawn(async move |cx| {
            for _ in 0..20 {
                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
                cx.update(|_| mac::report());
            }
        })
        .detach();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = plan;
}

/// An item of the bar as GPUI builds it: its title, and the icon it should have.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub title: String,
    pub icon: Option<Icon>,
    /// A submenu's items.
    pub items: Vec<Node>,
}

/// An item's image: an SF Symbol, and the Lucide icon in its place where the symbol is
/// missing (an older macOS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    pub symbol: Option<&'static str>,
    pub lucide: IconName,
}

/// The bar's items, in the order GPUI makes them (separators included, as untitled items).
pub fn plan(menus: &[Menu]) -> Vec<Node> {
    menus
        .iter()
        .map(|m| Node { title: m.name.to_string(), icon: None, items: m.items.iter().map(node).collect() })
        .collect()
}

fn node(item: &MenuItem) -> Node {
    match item {
        MenuItem::Separator => Node { title: String::new(), icon: None, items: Vec::new() },
        MenuItem::Action { name, action, .. } => {
            Node { title: name.to_string(), icon: for_action(action.name()), items: Vec::new() }
        }
        MenuItem::Submenu(m) => {
            Node { title: m.name.to_string(), icon: for_submenu(&m.name), items: m.items.iter().map(node).collect() }
        }
        MenuItem::SystemMenu(m) => Node { title: m.name.to_string(), icon: for_submenu(&m.name), items: Vec::new() },
    }
}

const fn sf(symbol: &'static str, lucide: IconName) -> Option<Icon> {
    Some(Icon { symbol: Some(symbol), lucide })
}

/// A submenu's icon, by its title.
fn for_submenu(title: &str) -> Option<Icon> {
    use IconName as L;
    match title {
        "Open Recent" => sf("clock.arrow.circlepath", L::Clock),
        "Tool Window Actions" => sf("sidebar.left", L::PanelLeft),
        "Appearance" => sf("circle.lefthalf.filled", L::SunMoon),
        "Services" => sf("gearshape.2", L::Cog),
        _ => None,
    }
}

/// The icon of the item that runs the action named `name` (`snout::Open`), following the
/// conventions of macOS's own menus (Undo, Cut, Copy, Paste, Find, Zoom, Minimize…).
pub fn for_action(name: &str) -> Option<Icon> {
    use IconName as L;
    let short = name.rsplit("::").next().unwrap_or(name);
    match short {
        // The app menu.
        "About" => sf("info.circle", L::Info),
        "OpenSettings" => sf("gearshape", L::Settings),
        "OpenSettingsJson" => sf("curlybraces", L::FileBraces),
        "OpenProjectSettingsJson" => sf("curlybraces.square", L::FolderCog),
        "Quit" => sf("power", L::Power),
        // File.
        "NewProject" => sf("plus.square", L::SquarePlus),
        "Open" => sf("folder", L::FolderOpen),
        "CloseProject" => sf("xmark.square", L::SquareX),
        "ClearRecent" => sf("xmark.bin", L::Trash),
        "ProjectStructure" => sf("square.stack.3d.up", L::Layers),
        "ConnectSsh" => sf("network", L::Network),
        "CloseRemote" => sf("network.slash", L::Unplug),
        "FindRepos" => sf("folder.badge.questionmark", L::FolderSearch),
        "CloneRepo" => sf("square.and.arrow.down.on.square", L::FolderDown),
        "NewGroup" => sf("folder.badge.plus", L::FolderPlus),
        "SearchEverywhere" => sf("magnifyingglass", L::Search),
        "GoToFile" => sf("doc.text.magnifyingglass", L::FileSearch),
        "FindInFiles" => sf("text.magnifyingglass", L::TextSearch),
        "ReplaceInFiles" => sf("arrow.2.squarepath", L::Replace),
        "UndoReplaceInFiles" => sf("arrow.uturn.backward.circle", L::Undo2),
        "CommandPalette" => sf("command", L::Command),
        "Save" => sf("square.and.arrow.down", L::Save),
        "CloseTab" => sf("xmark", L::X),
        "OpenInTerminal" => sf("terminal", L::SquareTerminal),
        // Edit.
        "Undo" => sf("arrow.uturn.backward", L::Undo2),
        "Redo" => sf("arrow.uturn.forward", L::Redo2),
        "Cut" => sf("scissors", L::Scissors),
        "Copy" => sf("doc.on.doc", L::Copy),
        "Paste" => sf("doc.on.clipboard", L::ClipboardPaste),
        "SelectAll" => sf("selection.pin.in.out", L::TextCursor),
        "Search" => sf("magnifyingglass", L::Search),
        "Replace" => sf("arrow.left.arrow.right", L::Replace),
        "GoToLine" => sf("number", L::Hash),
        "ToggleComment" => sf("text.bubble", L::MessageSquareCode),
        "DuplicateLine" => sf("plus.square.on.square", L::CopyPlus),
        "MoveLineUp" => sf("arrow.up", L::ArrowUp),
        "MoveLineDown" => sf("arrow.down", L::ArrowDown),
        // View.
        "ShowProject" => sf("folder", L::Folder),
        "ShowGit" => sf("arrow.triangle.branch", L::GitBranch),
        "ShowStructure" => sf("list.bullet.indent", L::ListTree),
        "ShowHierarchy" => sf("point.3.connected.trianglepath.dotted", L::Network),
        "LocateFile" => sf("scope", L::LocateFixed),
        "CollapseAll" | "CollapseGradle" | "CollapseSpring" => sf("rectangle.compress.vertical", L::ChevronsDownUp),
        "HideSide" => sf("sidebar.left", L::PanelLeftClose),
        "SortStructure" => sf("arrow.up.arrow.down", L::ArrowDownAZ),
        "ClearOutput" => sf("clear", L::Eraser),
        "ToggleConsole" => sf("sparkle", L::Sparkles),
        "SideBySide" => sf("rectangle.split.2x1", L::Columns2),
        "Unified" => sf("rectangle.split.1x2", L::Rows2),
        "Compact" => sf("arrow.down.right.and.arrow.up.left", L::FoldVertical),
        "ZoomIn" => sf("plus.magnifyingglass", L::ZoomIn),
        "ZoomOut" => sf("minus.magnifyingglass", L::ZoomOut),
        "ZoomReset" => sf("1.magnifyingglass", L::Search),
        "ToggleSoftWrap" => sf("text.word.spacing", L::TextWrap),
        "ToggleParamHints" => sf("tag", L::Tag),
        "SplitRight" => sf("square.split.2x1", L::SquareSplitHorizontal),
        "ShowProcesses" => sf("cpu", L::Cpu),
        "ThemeSystem" => sf("circle.lefthalf.filled", L::SunMoon),
        "ThemeLight" => sf("sun.max", L::Sun),
        "ThemeDark" => sf("moon", L::Moon),
        "Refresh" => sf("arrow.clockwise", L::RefreshCw),
        // Code.
        "GoToDefinition" => sf("arrow.right.circle", L::SquareArrowRight),
        "FindReferences" => sf("link", L::Link),
        "ToggleCodeActions" | "ShowContextActions" => sf("lightbulb", L::Lightbulb),
        "ApplyTopFix" => sf("wand.and.stars", L::WandSparkles),
        "FormatDocument" => sf("text.alignleft", L::TextAlignJustify),
        "FormatSelection" => sf("text.justify.left", L::TextAlignStart),
        "OrganizeImports" => sf("arrow.up.arrow.down.square", L::ArrowUpDown),
        "ShowSignature" => sf("info.bubble", L::MessageSquareMore),
        "ShowProblems" => sf("exclamationmark.triangle", L::TriangleAlert),
        "SpringAtCaret" => sf("leaf", L::Leaf),
        "SpringProfiles" => sf("person.crop.rectangle.stack", L::Layers),
        "SendToClaude" => sf("paperplane", L::Send),
        "RecentFiles" => sf("clock.arrow.circlepath", L::FileClock),
        "NewFile" => sf("doc.badge.plus", L::FilePlus),
        "AcceptAllClaude" => sf("checkmark.seal", L::CheckCheck),
        "RevertAllClaude" => sf("arrow.uturn.backward.circle", L::Undo2),
        "ShowAgentChanges" => sf("doc.badge.clock", L::FileClock),
        "AnnotateWithAgents" => sf("person.text.rectangle", L::UserPen),
        // Navigate.
        "GoToClass" => sf("c.square", L::SquareCode),
        "GoToSymbol" => sf("at", L::AtSign),
        "GoToSymbolInFile" => sf("list.bullet.rectangle", L::ListTree),
        "FileStructure" => sf("list.bullet.indent", L::ListTree),
        "TypeHierarchy" => sf("square.stack.3d.down.right", L::Network),
        "CallHierarchy" => sf("phone.arrow.down.left", L::PhoneIncoming),
        // Refactor.
        "RenameSymbol" => sf("pencil", L::Pencil),
        "ExtractMethod" => sf("function", L::SquareFunction),
        "ExtractVariable" => sf("x.squareroot", L::Variable),
        "ExtractConstant" => sf("lock.square", L::Lock),
        "ExtractField" => sf("rectangle.and.pencil.and.ellipsis", L::TextCursorInput),
        "Inline" => sf("arrow.down.to.line.compact", L::FoldVertical),
        // Run.
        "RunSelected" => sf("play", L::Play),
        "DebugSelected" => sf("ant", L::Bug),
        "RunContext" => sf("play.circle", L::CirclePlay),
        "DebugContext" => sf("ant.circle", L::Bug),
        "StopRuns" => sf("stop", L::Square),
        "Rerun" => sf("arrow.clockwise.circle", L::RotateCw),
        "RerunFailedTests" => sf("exclamationmark.arrow.circlepath", L::RotateCcw),
        "RunGradleTask" => sf("hammer", L::Hammer),
        "EditConfigurations" => sf("slider.horizontal.3", L::SlidersHorizontal),
        "ToggleBreakpoint" => sf("smallcircle.filled.circle", L::CircleDot),
        "ToggleBreakpointEnabled" => sf("circle.slash", L::CircleSlash),
        "EditBreakpointCondition" => sf("questionmark.diamond", L::Diamond),
        "ViewBreakpoints" => sf("list.bullet.circle", L::ListChecks),
        "StepOver" => sf("arrow.uturn.right", L::Redo),
        "StepInto" => sf("arrow.down.to.line", L::ArrowDownToLine),
        "StepOut" => sf("arrow.up.to.line", L::ArrowUpToLine),
        "RunToCursor" => sf("arrow.right.to.line", L::ArrowRightToLine),
        "PauseProgram" => sf("pause", L::Pause),
        "Resume" => sf("playpause", L::StepForward),
        "EvaluateExpression" => sf("equal.circle", L::Calculator),
        // Git.
        "Pull" => sf("arrow.down.circle", L::ArrowDownToLine),
        "Fetch" => sf("arrow.triangle.2.circlepath", L::RefreshCw),
        "Push" => sf("arrow.up.circle", L::ArrowUpFromLine),
        "Commit" => sf("checkmark.circle", L::GitCommitHorizontal),
        "Draft" => sf("sparkles", L::Sparkles),
        "Stage" => sf("plus.circle", L::CirclePlus),
        "Unstage" => sf("minus.circle", L::CircleMinus),
        "Discard" => sf("xmark.circle", L::CircleX),
        "Trash" => sf("trash", L::Trash),
        "Checkout" => sf("arrow.triangle.swap", L::GitBranch),
        "NewBranch" => sf("plus.square.dashed", L::GitBranchPlus),
        "Github" => sf("safari", L::Globe),
        "Reveal" => sf("arrow.up.forward.app", L::FolderOpen),
        // Tools.
        "StopIdleProcesses" => sf("stop.circle", L::CircleStop),
        "RestartJava" => sf("arrow.clockwise.circle", L::RotateCw),
        "SyncGradle" => sf("arrow.triangle.2.circlepath.circle", L::RefreshCw),
        "RefreshGradle" => sf("arrow.clockwise", L::RefreshCw),
        "RefreshSpring" => sf("arrow.clockwise", L::RefreshCw),
        "ShowSpring" => sf("leaf", L::Leaf),
        // Window.
        "Minimize" => sf("minus.rectangle", L::Minimize2),
        "Zoom" => sf("arrow.up.left.and.arrow.down.right", L::Maximize2),
        // Help.
        "Source" => sf("chevron.left.forwardslash.chevron.right", L::Code),
        "Issues" => sf("exclamationmark.bubble", L::MessageSquareWarning),
        "Website" => sf("globe", L::Globe),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use objc2::rc::Retained;
    use objc2::{AnyThread as _, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage, NSMenu};
    use objc2_foundation::{NSData, NSSize, NSString};

    use super::{Icon, Node};

    /// Where an image came from, for `SNOUT_MENU_ICONS=dump`.
    #[derive(Clone, Copy, Debug)]
    enum Source {
        Symbol,
        Lucide,
    }

    /// An image made, and where it came from.
    type Made = (Retained<NSImage>, Source);

    thread_local! {
        /// The images made, by symbol or Lucide icon: the bar is rebuilt often.
        static IMAGES: RefCell<HashMap<String, Option<Made>>> = RefCell::new(HashMap::new());
    }

    /// Give the main menu's items the images of `plan` (see the module's documentation).
    pub fn decorate(plan: &[Node]) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let app = NSApplication::sharedApplication(mtm);
        let Some(bar) = app.mainMenu() else { return };
        let dump = std::env::var("SNOUT_MENU_ICONS").is_ok_and(|v| v.contains("dump"));
        walk(&bar, plan, dump, 0);
    }

    fn walk(menu: &NSMenu, plan: &[Node], dump: bool, depth: usize) {
        let n = usize::try_from(menu.numberOfItems()).unwrap_or(0);
        for (i, node) in plan.iter().enumerate().take(n) {
            let Some(item) = menu.itemAtIndex(i as isize) else { continue };
            if item.isSeparatorItem() || item.title().to_string() != node.title {
                continue;
            }
            // The bar's own items (File, Edit…) take no image.
            let image = if depth == 0 { None } else { node.icon.and_then(image_for) };
            if let Some((img, _)) = &image {
                item.setImage(Some(img));
                show_image(&item);
            }
            if dump {
                let how = match &image {
                    Some((_, Source::Symbol)) => "symbol",
                    Some((_, Source::Lucide)) => "lucide",
                    None => "none",
                };
                let set = if item.image().is_some() { "set" } else { "unset" };
                println!("[menu-icons] {}{} : {how} ({set})", "  ".repeat(depth), node.title);
            }
            if let Some(sub) = item.submenu() {
                walk(&sub, &node.items, dump, depth + 1);
            }
        }
    }

    /// Ask AppKit to show the item's image. From macOS 27 it hides menu images unless an item
    /// prefers them visible (`preferredImageVisibility`, which older systems lack).
    fn show_image(item: &objc2_app_kit::NSMenuItem) {
        use objc2::{msg_send, sel};
        use objc2_foundation::NSObjectProtocol as _;
        if item.respondsToSelector(sel!(setPreferredImageVisibility:)) {
            // NSMenuItemImageVisibilityVisible.
            let visible: isize = 1;
            // SAFETY: the item answers the selector, which takes an NSInteger.
            unsafe {
                let _: () = msg_send![item, setPreferredImageVisibility: visible];
            }
        }
    }

    /// How many of the main menu's items have an image now (`SNOUT_MENU_ICONS=watch`).
    pub fn report() {
        fn count(menu: &NSMenu) -> (usize, usize) {
            let mut n = (0, 0);
            for i in 0..menu.numberOfItems() {
                let Some(item) = menu.itemAtIndex(i) else { continue };
                n.0 += 1;
                n.1 += usize::from(item.image().is_some());
                if let Some(sub) = item.submenu() {
                    let (a, b) = count(&sub);
                    n = (n.0 + a, n.1 + b);
                }
            }
            n
        }
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let Some(bar) = NSApplication::sharedApplication(mtm).mainMenu() {
            let (items, images) = count(&bar);
            println!("[menu-icons] now: {images} of {items} items have an image");
        }
    }

    /// The image of `icon`: its SF Symbol, else its Lucide icon as a template image.
    fn image_for(icon: Icon) -> Option<(Retained<NSImage>, Source)> {
        let force_lucide = std::env::var("SNOUT_MENU_ICONS").is_ok_and(|v| v.contains("lucide"));
        if let Some(symbol) = icon.symbol.filter(|_| !force_lucide) {
            let made = cached(&format!("sf:{symbol}"), || {
                let img =
                    NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(symbol), None)?;
                Some((img, Source::Symbol))
            });
            if made.is_some() {
                return made;
            }
        }
        let path = gpui_component::IconNamed::path(icon.lucide);
        cached(&format!("lucide:{path}"), || Some((lucide(&path)?, Source::Lucide)))
    }

    fn cached(
        key: &str,
        make: impl FnOnce() -> Option<(Retained<NSImage>, Source)>,
    ) -> Option<(Retained<NSImage>, Source)> {
        IMAGES.with(|m| {
            if let Some(found) = m.borrow().get(key) {
                return found.clone();
            }
            let made = make();
            m.borrow_mut().insert(key.to_owned(), made.clone());
            made
        })
    }

    /// The Lucide icon at `path` among GPUI Kit's assets, drawn 16 points square at twice the
    /// pixels, as a template image (macOS colours it as the menu's text).
    fn lucide(path: &str) -> Option<Retained<NSImage>> {
        use gpui::AssetSource as _;
        let svg = gpui_kit_assets::AllAssets.load(path).ok()??;
        let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).ok()?;
        let px = 32u32;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
        let scale = px as f32 / tree.size().width().max(tree.size().height());
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
        let png = pixmap.encode_png().ok()?;
        let data = NSData::with_bytes(&png);
        let img = NSImage::initWithData(NSImage::alloc(), &data)?;
        img.setSize(NSSize::new(16., 16.));
        img.setTemplate(true);
        Some(img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every item of the bar that runs something has an icon, save those left plain on
    /// purpose; and the plan follows the bar's shape.
    #[test]
    fn the_menu_bar_items_have_icons() {
        let s = crate::settings::Settings::default();
        let bar = crate::menus::bar(&s, &[], &[]);
        let plan = plan(&bar);
        assert_eq!(plan.len(), bar.len());
        let mut missing = Vec::new();
        fn walk(nodes: &[Node], top: bool, missing: &mut Vec<String>) {
            for n in nodes {
                let plain = n.title.is_empty() || top || n.icon.is_some();
                if !plain {
                    missing.push(n.title.clone());
                }
                walk(&n.items, false, missing);
            }
        }
        walk(&plan, true, &mut missing);
        assert!(missing.is_empty(), "menu items without an icon: {missing:?}");
    }
}

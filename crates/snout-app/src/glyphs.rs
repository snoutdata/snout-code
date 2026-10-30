//! The window's line icons, drawn here: one thin stroke on a 16 point grid, so the title
//! bar, the tool window stripes and the tool windows' toolbars read as one set.

use gpui::{Hsla, IntoElement, PathBuilder, Pixels, Point, Styled as _, Window, canvas, point, px};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Folder,
    Branch,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    Play,
    Bug,
    Search,
    Gear,
    Bell,
    Terminal,
    Problems,
    Output,
    Spark,
    Build,
    CollapseAll,
    Locate,
    Close,
    Lock,
    Warning,
    Check,
    Minus,
    More,
    Refresh,
    Fetch,
    Pull,
    Push,
    Plus,
    Commit,
    Keyboard,
    Brush,
    Code,
    Tools,
    /// A module: a folder with a block in its corner.
    Module,
    /// Source roots, by what they hold: a folder with a dot, a check (tests), two lines
    /// (resources), lines and a dot (test resources), a spark (generated), a stroke
    /// through (excluded).
    SourceFolder,
    TestFolder,
    ResourceFolder,
    TestResourceFolder,
    GeneratedFolder,
    ExcludedFolder,
    /// External libraries: books on a shelf.
    Library,
    Jar,
    Package,
    /// A JDK: a cup.
    Jdk,
    /// The Run tool window: a play triangle in outline.
    RunWindow,
    /// A filled square: stop a run.
    Stop,
    /// Two bars: pause the program.
    Pause,
    /// A bar and a triangle: resume the program.
    Resume,
    /// Rerun: a circular arrow round a small triangle.
    Rerun,
    /// Rerun the failed tests: a circular arrow round a cross.
    RerunFailed,
    /// Step over: an arc over a dot.
    StepOver,
    /// Step into: an arrow down to a dot.
    StepInto,
    /// Step out: an arrow up from a dot.
    StepOut,
    /// The Build tool window: a hammer.
    Hammer,
    /// A watch: an eye.
    Eye,
    /// A variable: a small box with a dot.
    Variable,
    /// A stack frame: three bars, stacked.
    Frames,
    /// A Spring bean: a leaf on its stem, with its midrib.
    Leaf,
    /// An injection point: an arrow into a socket.
    Inject,
    /// An endpoint: a globe.
    Endpoint,
    /// Copy: two sheets, one over the other.
    Copy,
    /// Claude's changes: a page with a plus and a minus on it.
    ClaudeChanges,
    /// Sorted by name: bars growing shorter, an arrow down beside them.
    Sort,
    /// The Structure window: an outline, a bar and the bars it holds.
    Structure,
    /// The Hierarchy window: a box above two it branches to.
    Hierarchy,
    /// Maven: a quill.
    Feather,
}

/// One stroke of an icon, in grid units (16 across).
enum Part {
    /// A polyline; closed when the flag says so.
    Line(&'static [(f32, f32)], bool),
    /// A circle: centre and radius; filled when the flag says so.
    Circle(f32, f32, f32, bool),
    /// A filled polygon.
    Fill(&'static [(f32, f32)]),
    /// A quadratic curve: from, control, to.
    Curve((f32, f32), (f32, f32), (f32, f32)),
}

use Part::*;

/// The folder the source roots' glyphs draw on.
const FOLDER: Part = Line(&[(1.5, 13.), (1.5, 3.5), (6., 3.5), (7.5, 5.5), (14.5, 5.5), (14.5, 13.)], true);

impl Icon {
    fn parts(self) -> &'static [Part] {
        match self {
            Icon::Folder => &[Line(&[(1.5, 13.), (1.5, 3.5), (6., 3.5), (7.5, 5.5), (14.5, 5.5), (14.5, 13.)], true)],
            Icon::Branch => &[
                Circle(4.5, 3.5, 1.6, false),
                Circle(4.5, 12.5, 1.6, false),
                Circle(11.5, 4.5, 1.6, false),
                Line(&[(4.5, 5.1), (4.5, 10.9)], false),
                Curve((11.5, 6.1), (11.5, 9.), (5.4, 10.9)),
            ],
            Icon::ChevronDown => &[Line(&[(4.5, 6.5), (8., 10.), (11.5, 6.5)], false)],
            Icon::ChevronRight => &[Line(&[(6.5, 4.5), (10., 8.), (6.5, 11.5)], false)],
            Icon::ChevronUp => &[Line(&[(4.5, 9.5), (8., 6.), (11.5, 9.5)], false)],
            Icon::Play => &[Fill(&[(4.5, 3.), (13., 8.), (4.5, 13.)])],
            Icon::Bug => &[
                Circle(8., 9.5, 3.6, false),
                Circle(8., 4.4, 1.7, false),
                Line(&[(8., 6.6), (8., 13.)], false),
                Line(&[(4.6, 8.), (2., 7.)], false),
                Line(&[(4.5, 11.), (2., 12.5)], false),
                Line(&[(11.4, 8.), (14., 7.)], false),
                Line(&[(11.5, 11.), (14., 12.5)], false),
            ],
            Icon::Search => &[Circle(7., 7., 4.5, false), Line(&[(10.3, 10.3), (14., 14.)], false)],
            Icon::Gear => &[
                Circle(8., 8., 2.2, false),
                Circle(8., 8., 4.9, false),
                Line(&[(8., 1.3), (8., 3.1)], false),
                Line(&[(8., 12.9), (8., 14.7)], false),
                Line(&[(1.3, 8.), (3.1, 8.)], false),
                Line(&[(12.9, 8.), (14.7, 8.)], false),
                Line(&[(3.3, 3.3), (4.5, 4.5)], false),
                Line(&[(11.5, 11.5), (12.7, 12.7)], false),
                Line(&[(3.3, 12.7), (4.5, 11.5)], false),
                Line(&[(11.5, 4.5), (12.7, 3.3)], false),
            ],
            Icon::Bell => &[
                Line(&[(2.5, 12.), (13.5, 12.)], false),
                Line(&[(4.2, 12.), (4.2, 7.5)], false),
                Line(&[(11.8, 12.), (11.8, 7.5)], false),
                Curve((4.2, 7.5), (4.2, 2.8), (8., 2.8)),
                Curve((8., 2.8), (11.8, 2.8), (11.8, 7.5)),
                Line(&[(6.5, 14.), (9.5, 14.)], false),
            ],
            Icon::Terminal => &[
                Line(&[(1.5, 2.5), (14.5, 2.5), (14.5, 13.5), (1.5, 13.5)], true),
                Line(&[(4.5, 6.), (7., 8.), (4.5, 10.)], false),
                Line(&[(8.5, 10.5), (11.5, 10.5)], false),
            ],
            Icon::Problems => {
                &[Circle(8., 8., 6., false), Line(&[(8., 4.8), (8., 8.8)], false), Circle(8., 11.2, 0.8, true)]
            }
            Icon::Output => &[
                Line(&[(2.5, 4.), (13.5, 4.)], false),
                Line(&[(2.5, 8.), (13.5, 8.)], false),
                Line(&[(2.5, 12.), (9.5, 12.)], false),
            ],
            Icon::Spark => &[
                Curve((8., 1.5), (8.6, 7.4), (14.5, 8.)),
                Curve((14.5, 8.), (8.6, 8.6), (8., 14.5)),
                Curve((8., 14.5), (7.4, 8.6), (1.5, 8.)),
                Curve((1.5, 8.), (7.4, 7.4), (8., 1.5)),
            ],
            Icon::Build => &[
                Line(&[(8., 1.5), (14., 4.8), (14., 11.2), (8., 14.5), (2., 11.2), (2., 4.8)], true),
                Line(&[(2., 4.8), (8., 8.), (14., 4.8)], false),
                Line(&[(8., 8.), (8., 14.5)], false),
            ],
            Icon::CollapseAll => {
                &[Line(&[(4., 7.), (8., 3.5), (12., 7.)], false), Line(&[(4., 12.5), (8., 9.), (12., 12.5)], false)]
            }
            Icon::Locate => &[
                Circle(8., 8., 4.5, false),
                Circle(8., 8., 1.1, true),
                Line(&[(8., 1.), (8., 3.5)], false),
                Line(&[(8., 12.5), (8., 15.)], false),
                Line(&[(1., 8.), (3.5, 8.)], false),
                Line(&[(12.5, 8.), (15., 8.)], false),
            ],
            Icon::Close => &[Line(&[(4.5, 4.5), (11.5, 11.5)], false), Line(&[(11.5, 4.5), (4.5, 11.5)], false)],
            Icon::Lock => &[
                Line(&[(3.5, 7.), (12.5, 7.), (12.5, 14.), (3.5, 14.)], true),
                Line(&[(5.5, 7.), (5.5, 5.)], false),
                Line(&[(10.5, 7.), (10.5, 5.)], false),
                Curve((5.5, 5.), (5.5, 2.5), (8., 2.5)),
                Curve((8., 2.5), (10.5, 2.5), (10.5, 5.)),
            ],
            Icon::Warning => &[
                Line(&[(8., 2.), (14.5, 13.5), (1.5, 13.5)], true),
                Line(&[(8., 6.), (8., 9.5)], false),
                Circle(8., 11.5, 0.75, true),
            ],
            Icon::Check => &[Line(&[(3.5, 8.5), (6.5, 11.5), (12.5, 4.5)], false)],
            Icon::Minus => &[Line(&[(3.5, 8.), (12.5, 8.)], false)],
            Icon::More => &[Circle(8., 3.5, 1., true), Circle(8., 8., 1., true), Circle(8., 12.5, 1., true)],
            Icon::Refresh => &[
                Curve((13., 8.), (13., 13.), (8., 13.)),
                Curve((8., 13.), (3., 13.), (3., 8.)),
                Curve((3., 8.), (3., 3.), (8., 3.)),
                Line(&[(6.5, 1.2), (8.5, 3.), (6.5, 4.8)], false),
            ],
            Icon::Fetch => &[
                Line(&[(8., 2.), (8., 11.)], false),
                Line(&[(4.5, 7.5), (8., 11.), (11.5, 7.5)], false),
                Line(&[(3., 14.), (13., 14.)], false),
            ],
            Icon::Pull => &[
                Line(&[(8., 1.5), (8., 10.5)], false),
                Line(&[(4.5, 7.), (8., 10.5), (11.5, 7.)], false),
                Line(&[(2.5, 11.), (2.5, 14.), (13.5, 14.), (13.5, 11.)], false),
            ],
            Icon::Push => &[
                Line(&[(8., 10.5), (8., 1.5)], false),
                Line(&[(4.5, 5.), (8., 1.5), (11.5, 5.)], false),
                Line(&[(2.5, 11.), (2.5, 14.), (13.5, 14.), (13.5, 11.)], false),
            ],
            Icon::Plus => &[Line(&[(8., 3.), (8., 13.)], false), Line(&[(3., 8.), (13., 8.)], false)],
            Icon::Commit => {
                &[Circle(8., 8., 3., false), Line(&[(1., 8.), (5., 8.)], false), Line(&[(11., 8.), (15., 8.)], false)]
            }
            Icon::Keyboard => &[
                Line(&[(1.5, 4.), (14.5, 4.), (14.5, 12.), (1.5, 12.)], true),
                Line(&[(4.5, 9.5), (11.5, 9.5)], false),
                Circle(4.5, 6.6, 0.6, true),
                Circle(8., 6.6, 0.6, true),
                Circle(11.5, 6.6, 0.6, true),
            ],
            Icon::Brush => &[
                Circle(8., 8., 6., false),
                Circle(5.5, 6.5, 1., true),
                Circle(9.5, 5., 1., true),
                Circle(11., 9., 1., true),
            ],
            Icon::Code => {
                &[Line(&[(5.5, 4.), (1.5, 8.), (5.5, 12.)], false), Line(&[(10.5, 4.), (14.5, 8.), (10.5, 12.)], false)]
            }
            Icon::Tools => &[
                Line(&[(2.5, 13.5), (9., 7.)], false),
                Circle(11., 5., 2.8, false),
                Line(&[(2., 3.), (5., 3.), (5., 6.), (2., 6.)], true),
            ],
            Icon::Module => &[
                Line(&[(1.5, 11.), (1.5, 3.5), (6., 3.5), (7.5, 5.5), (14.5, 5.5), (14.5, 7.5)], false),
                Line(&[(1.5, 11.), (1.5, 13.), (7.5, 13.)], false),
                Fill(&[(9.5, 9.), (15., 9.), (15., 14.5), (9.5, 14.5)]),
            ],
            Icon::SourceFolder => &[FOLDER, Circle(8., 9.3, 1.9, true)],
            Icon::TestFolder => &[FOLDER, Line(&[(5.3, 9.2), (7.2, 11.), (10.7, 7.6)], false)],
            Icon::ResourceFolder => {
                &[FOLDER, Line(&[(4.5, 8.3), (11.5, 8.3)], false), Line(&[(4.5, 10.7), (11.5, 10.7)], false)]
            }
            Icon::TestResourceFolder => &[
                FOLDER,
                Line(&[(4.5, 8.3), (8.5, 8.3)], false),
                Line(&[(4.5, 10.7), (8.5, 10.7)], false),
                Circle(11.3, 9.5, 1.3, true),
            ],
            Icon::GeneratedFolder => &[
                FOLDER,
                Curve((8., 6.6), (8.3, 9.), (10.7, 9.3)),
                Curve((10.7, 9.3), (8.3, 9.6), (8., 12.)),
                Curve((8., 12.), (7.7, 9.6), (5.3, 9.3)),
                Curve((5.3, 9.3), (7.7, 9.), (8., 6.6)),
            ],
            Icon::ExcludedFolder => &[FOLDER, Line(&[(5.3, 11.5), (10.7, 7.)], false)],
            Icon::Library => &[
                Line(&[(2., 3.), (4.8, 3.), (4.8, 13.5), (2., 13.5)], true),
                Line(&[(6.2, 4.5), (9., 4.5), (9., 13.5), (6.2, 13.5)], true),
                Line(&[(10., 4.2), (12.6, 3.4), (15., 12.6), (12.4, 13.4)], true),
            ],
            Icon::Jar => &[
                Line(&[(3.5, 5.5), (12.5, 5.5), (12.5, 14.), (3.5, 14.)], true),
                Line(&[(4.5, 2.), (11.5, 2.), (11.5, 5.5), (4.5, 5.5)], true),
                Line(&[(6., 9.), (10., 9.)], false),
            ],
            Icon::Package => &[
                Line(&[(2.5, 3.5), (13.5, 3.5), (13.5, 13.5), (2.5, 13.5)], true),
                Line(&[(2.5, 7.), (13.5, 7.)], false),
                Line(&[(6.5, 3.5), (6.5, 7.)], false),
                Line(&[(9.5, 3.5), (9.5, 7.)], false),
            ],
            Icon::RunWindow => &[Line(&[(4., 2.5), (13., 8.), (4., 13.5)], true)],
            Icon::Stop => &[Fill(&[(3.5, 3.5), (12.5, 3.5), (12.5, 12.5), (3.5, 12.5)])],
            Icon::Pause => &[
                Fill(&[(4., 3.), (6.8, 3.), (6.8, 13.), (4., 13.)]),
                Fill(&[(9.2, 3.), (12., 3.), (12., 13.), (9.2, 13.)]),
            ],
            Icon::Resume => {
                &[Fill(&[(3., 3.), (5., 3.), (5., 13.), (3., 13.)]), Fill(&[(7., 3.), (14., 8.), (7., 13.)])]
            }
            Icon::Rerun => &[
                Curve((13., 8.), (13., 13.), (8., 13.)),
                Curve((8., 13.), (3., 13.), (3., 8.)),
                Curve((3., 8.), (3., 3.), (8., 3.)),
                Line(&[(6.5, 1.2), (8.5, 3.), (6.5, 4.8)], false),
                Fill(&[(6.8, 6.), (10.5, 8.), (6.8, 10.)]),
            ],
            Icon::RerunFailed => &[
                Curve((13., 8.), (13., 13.), (8., 13.)),
                Curve((8., 13.), (3., 13.), (3., 8.)),
                Curve((3., 8.), (3., 3.), (8., 3.)),
                Line(&[(6.5, 1.2), (8.5, 3.), (6.5, 4.8)], false),
                Line(&[(6.3, 6.3), (9.7, 9.7)], false),
                Line(&[(9.7, 6.3), (6.3, 9.7)], false),
            ],
            Icon::StepOver => &[
                Curve((2.5, 9.), (8., 0.5), (13.5, 9.)),
                Line(&[(10.5, 8.), (13.5, 9.), (14.2, 6.)], false),
                Circle(8., 13., 1.6, true),
            ],
            Icon::StepInto => &[
                Line(&[(8., 1.5), (8., 8.5)], false),
                Line(&[(5., 5.5), (8., 8.5), (11., 5.5)], false),
                Circle(8., 13., 1.6, true),
            ],
            Icon::StepOut => &[
                Line(&[(8., 9.), (8., 1.8)], false),
                Line(&[(5., 4.8), (8., 1.8), (11., 4.8)], false),
                Circle(8., 13., 1.6, true),
            ],
            Icon::Hammer => &[
                Line(&[(2.5, 4.), (8.5, 2.), (11., 4.5), (9., 6.5), (6.5, 6.)], true),
                Line(&[(8., 6.2), (14., 13.)], false),
            ],
            Icon::Eye => &[
                Curve((1.5, 8.), (8., 1.5), (14.5, 8.)),
                Curve((1.5, 8.), (8., 14.5), (14.5, 8.)),
                Circle(8., 8., 2., true),
            ],
            Icon::Variable => {
                &[Line(&[(2.5, 3.5), (13.5, 3.5), (13.5, 12.5), (2.5, 12.5)], true), Circle(8., 8., 1.7, true)]
            }
            Icon::Frames => &[
                Line(&[(2.5, 3.5), (13.5, 3.5)], false),
                Line(&[(2.5, 8.), (13.5, 8.)], false),
                Line(&[(2.5, 12.5), (13.5, 12.5)], false),
            ],
            Icon::Leaf => &[
                Curve((3., 13.), (3., 3.), (13.5, 2.5)),
                Curve((13.5, 2.5), (13., 13.), (3., 13.)),
                Line(&[(3., 13.), (9.5, 6.5)], false),
                Line(&[(3., 13.), (1.5, 14.5)], false),
            ],
            Icon::Feather => &[
                Curve((4., 12.), (4., 3.), (13.5, 2.5)),
                Curve((13.5, 2.5), (12.5, 10.), (6., 10.)),
                Line(&[(2., 14.), (10., 6.)], false),
                Line(&[(7., 9.), (10., 9.)], false),
            ],
            Icon::Inject => &[
                Line(&[(1.5, 8.), (7.5, 8.)], false),
                Line(&[(5., 5.5), (7.5, 8.), (5., 10.5)], false),
                Circle(11.3, 8., 3.2, false),
                Circle(11.3, 8., 1.2, true),
            ],
            Icon::Endpoint => &[
                Circle(8., 8., 6., false),
                Curve((8., 2.), (3.6, 8.), (8., 14.)),
                Curve((8., 2.), (12.4, 8.), (8., 14.)),
                Line(&[(2., 8.), (14., 8.)], false),
            ],
            Icon::ClaudeChanges => &[
                Line(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5)], true),
                Line(&[(8., 4.5), (8., 9.5)], false),
                Line(&[(5.5, 7.), (10.5, 7.)], false),
                Line(&[(5.5, 12.), (10.5, 12.)], false),
            ],
            Icon::Sort => &[
                Line(&[(2., 4.), (9.5, 4.)], false),
                Line(&[(2., 8.), (7.5, 8.)], false),
                Line(&[(2., 12.), (5.5, 12.)], false),
                Line(&[(12.5, 3.), (12.5, 13.)], false),
                Line(&[(10.5, 11.), (12.5, 13.), (14.5, 11.)], false),
            ],
            Icon::Structure => &[
                Line(&[(2., 3.5), (14., 3.5)], false),
                Line(&[(4.5, 3.5), (4.5, 12.5)], false),
                Line(&[(4.5, 8.), (13., 8.)], false),
                Line(&[(4.5, 12.5), (13., 12.5)], false),
                Circle(2.5, 3.5, 0.9, true),
            ],
            Icon::Hierarchy => &[
                Line(&[(5.5, 1.5), (10.5, 1.5), (10.5, 5.), (5.5, 5.)], true),
                Line(&[(1.5, 11.), (6., 11.), (6., 14.5), (1.5, 14.5)], true),
                Line(&[(10., 11.), (14.5, 11.), (14.5, 14.5), (10., 14.5)], true),
                Line(&[(8., 5.), (8., 8.)], false),
                Line(&[(3.75, 11.), (3.75, 8.), (12.25, 8.), (12.25, 11.)], false),
            ],
            Icon::Copy => &[
                Line(&[(5.5, 5.5), (13.5, 5.5), (13.5, 14.), (5.5, 14.)], true),
                Line(&[(3., 11.), (2.5, 11.), (2.5, 2.5), (10.5, 2.5), (10.5, 3.)], false),
            ],
            Icon::Jdk => &[
                Line(&[(2.5, 6.5), (2.5, 11.5), (4.5, 14.), (9.5, 14.), (11.5, 11.5), (11.5, 6.5)], true),
                Curve((11.5, 7.5), (15., 8.), (11.5, 11.)),
                Curve((5., 1.5), (6.5, 3.), (5., 4.8)),
                Curve((8.5, 1.5), (10., 3.), (8.5, 4.8)),
            ],
        }
    }
}

/// Paint `icon` in `bounds` (fitted, centred) with lines of `ink`.
pub fn paint(icon: Icon, bounds: gpui::Bounds<Pixels>, ink: Hsla, window: &mut Window) {
    let side = f32::from(bounds.size.width.min(bounds.size.height));
    let scale = side / 16.;
    let c = bounds.center();
    let at = |(x, y): (f32, f32)| -> Point<Pixels> { point(c.x + px((x - 8.) * scale), c.y + px((y - 8.) * scale)) };
    let width = px((1.25 * scale).max(1.));
    for part in icon.parts() {
        let path = match part {
            Line(pts, closed) => {
                let mut b = PathBuilder::stroke(width);
                for (i, &q) in pts.iter().enumerate() {
                    if i == 0 { b.move_to(at(q)) } else { b.line_to(at(q)) }
                }
                if *closed {
                    b.close();
                }
                b.build()
            }
            Circle(x, y, r, filled) => {
                let mut b = if *filled { PathBuilder::fill() } else { PathBuilder::stroke(width) };
                let r = *r * scale;
                let radii = point(px(r), px(r));
                let (left, right) = (at((*x, *y)) - point(px(r), px(0.)), at((*x, *y)) + point(px(r), px(0.)));
                b.move_to(left);
                b.arc_to(radii, px(0.), false, true, right);
                b.arc_to(radii, px(0.), false, true, left);
                b.close();
                b.build()
            }
            Fill(pts) => {
                let mut b = PathBuilder::fill();
                for (i, &q) in pts.iter().enumerate() {
                    if i == 0 { b.move_to(at(q)) } else { b.line_to(at(q)) }
                }
                b.close();
                b.build()
            }
            Curve(from, ctrl, to) => {
                let mut b = PathBuilder::stroke(width);
                b.move_to(at(*from));
                b.curve_to(at(*to), at(*ctrl));
                b.build()
            }
        };
        if let Ok(path) = path {
            window.paint_path(path, ink);
        }
    }
}

/// `icon` as an element `size` points square.
pub fn icon(icon: Icon, size: f32, ink: Hsla) -> impl IntoElement {
    canvas(|_, _, _| {}, move |b, _, window, _| paint(icon, b, ink, window)).flex_none().size(px(size))
}

//! Flat vector art in the SnoutData house style: a round ink line over a few flat fills, never a
//! gradient, one soft green as the only hue, a couple of sparks. The drawings are written as SVG
//! path data (the website's own shapes where we share one, such as the spark and the butterfly),
//! flattened and triangulated once. Each front end paints them with whatever palette is showing.
//!
//! Adding a drawing: a soft `Blob` behind; objects in `Paper` / `Tint`; ONE `Accent`, on the
//! thing the picture is about; a couple of sparks. Spot drawings use a `0 0 240 160` box, icons
//! a `0 0 24 24` box.

use std::ops::{Add, Mul, Sub};
use std::sync::LazyLock;

/// A point (or a vector) in a drawing's own units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

pub type Vec2 = Pos2;

pub const fn pos2(x: f32, y: f32) -> Pos2 {
    Pos2 { x, y }
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Pos2 { x, y }
}

impl Pos2 {
    pub const ZERO: Self = pos2(0.0, 0.0);

    pub fn to_vec2(self) -> Vec2 {
        self
    }

    pub fn to_pos2(self) -> Pos2 {
        self
    }

    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

impl Add for Pos2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        pos2(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Pos2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        pos2(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Pos2 {
    type Output = Self;
    fn mul(self, k: f32) -> Self {
        pos2(self.x * k, self.y * k)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Fill {
    None,
    Blob,
    Paper,
    Tint,
    Accent,
    Clay,
    /// Solid ink, for dots and sparks.
    Ink,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Line {
    None,
    Ink,
    /// The faint horizon a drawing stands on.
    Ground,
}

pub struct Part {
    pub fill: Fill,
    /// Triangles, three points each.
    pub tris: Vec<Pos2>,
    /// Polylines, and whether each one closes.
    pub outlines: Vec<(Vec<Pos2>, bool)>,
    pub line: Line,
}

/// A drawing in its own coordinate box, built once and painted at any size.
pub struct Drawing {
    pub size: Vec2,
    /// Line width in drawing units.
    pub line_width: f32,
    pub parts: Vec<Part>,
    /// Sparks that twinkle: centre and scale, in drawing units. A front end draws them
    /// last, turned and shrunk by [`twinkle`] as time goes on.
    pub sparks: Vec<(Pos2, f32)>,
    xf: [f32; 6],
}

impl Drawing {
    fn new(w: f32, h: f32, line_width: f32) -> Self {
        Self { size: vec2(w, h), line_width, parts: Vec::new(), sparks: Vec::new(), xf: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }
    }

    // ----- Building ------------------------------------------------------------------

    /// Draw `f` under an extra SVG `matrix(a b c d e f)`.
    fn with(&mut self, m: [f32; 6], f: impl FnOnce(&mut Self)) {
        let saved = self.xf;
        let t = saved;
        self.xf = [
            t[0] * m[0] + t[2] * m[1],
            t[1] * m[0] + t[3] * m[1],
            t[0] * m[2] + t[2] * m[3],
            t[1] * m[2] + t[3] * m[3],
            t[0] * m[4] + t[2] * m[5] + t[4],
            t[1] * m[4] + t[3] * m[5] + t[5],
        ];
        f(self);
        self.xf = saved;
    }

    fn rotated(&mut self, deg: f32, cx: f32, cy: f32, f: impl FnOnce(&mut Self)) {
        let (s, c) = deg.to_radians().sin_cos();
        self.with([c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy], f);
    }

    fn placed(&mut self, x: f32, y: f32, scale: f32, f: impl FnOnce(&mut Self)) {
        self.with([scale, 0.0, 0.0, scale, x, y], f);
    }

    fn map(&self, p: Pos2) -> Pos2 {
        let t = self.xf;
        pos2(t[0] * p.x + t[2] * p.y + t[4], t[1] * p.x + t[3] * p.y + t[5])
    }

    fn add(&mut self, subpaths: Vec<(Vec<Pos2>, bool)>, fill: Fill, line: Line) -> &mut Self {
        let subpaths: Vec<(Vec<Pos2>, bool)> = subpaths
            .into_iter()
            .map(|(pts, closed)| (pts.into_iter().map(|p| self.map(p)).collect(), closed))
            .collect();
        let mut tris = Vec::new();
        if fill != Fill::None {
            for (pts, _) in &subpaths {
                triangulate(pts, &mut tris);
            }
        }
        self.parts.push(Part { fill, tris, outlines: subpaths, line });
        self
    }

    /// An SVG path (`M L H V C Q Z`, absolute or relative), filled and inked.
    fn path(&mut self, d: &str, fill: Fill) -> &mut Self {
        self.add(parse_path(d), fill, Line::Ink)
    }

    /// An SVG path, filled, with no ink line.
    fn shape(&mut self, d: &str, fill: Fill) -> &mut Self {
        self.add(parse_path(d), fill, Line::None)
    }

    /// An SVG path, inked only.
    fn line(&mut self, d: &str) -> &mut Self {
        self.add(parse_path(d), Fill::None, Line::Ink)
    }

    fn ground(&mut self, d: &str) -> &mut Self {
        self.add(parse_path(d), Fill::None, Line::Ground)
    }

    fn ellipse(&mut self, c: Pos2, rx: f32, ry: f32, fill: Fill, inked: bool) -> &mut Self {
        let pts = (0..48)
            .map(|i| {
                let a = i as f32 / 48.0 * std::f32::consts::TAU;
                pos2(c.x + rx * a.cos(), c.y + ry * a.sin())
            })
            .collect();
        self.add(vec![(pts, true)], fill, if inked { Line::Ink } else { Line::None })
    }

    fn circle(&mut self, x: f32, y: f32, r: f32, fill: Fill) -> &mut Self {
        self.ellipse(pos2(x, y), r, r, fill, true)
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) -> &mut Self {
        self.ellipse(pos2(x, y), r, r, Fill::Ink, false)
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, fill: Fill) -> &mut Self {
        let d = format!(
            "M{} {y}H{}Q{} {y} {} {}V{}Q{} {} {} {}H{}Q{x} {} {x} {}V{}Q{x} {y} {} {y}Z",
            x + r,
            x + w - r,
            x + w,
            x + w,
            y + r,
            y + h - r,
            x + w,
            y + h,
            x + w - r,
            y + h,
            x + r,
            y + h,
            y + h - r,
            y + r,
            x + r,
        );
        self.path(&d, fill)
    }

    /// An open arc around `(cx, cy)`, angles in degrees, clockwise on screen.
    fn arc(&mut self, cx: f32, cy: f32, r: f32, from: f32, to: f32) -> &mut Self {
        let n = ((to - from).abs() / 6.0).ceil().max(2.0) as usize;
        let pts = (0..=n)
            .map(|i| {
                let a = (from + (to - from) * i as f32 / n as f32).to_radians();
                pos2(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.add(vec![(pts, false)], Fill::None, Line::Ink)
    }

    /// The four-point spark, the house ornament.
    fn spark(&mut self, x: f32, y: f32, s: f32) -> &mut Self {
        let (a, b) = (7.0 * s, 1.4 * s);
        let d = format!(
            "M{x} {}Q{} {} {} {y}Q{} {} {x} {}Q{} {} {} {y}Q{} {} {x} {}Z",
            y - a,
            x + b,
            y - b,
            x + a,
            x + b,
            y + b,
            y + a,
            x - b,
            y + b,
            x - a,
            x - b,
            y - b,
            y - a
        );
        self.shape(&d, Fill::Ink)
    }

    /// A spark that twinkles (see [`twinkle`]).
    fn twinkling_spark(&mut self, x: f32, y: f32, s: f32) -> &mut Self {
        let c = self.map(pos2(x, y));
        self.sparks.push((c, s));
        self
    }

    /// A gear: `teeth` teeth round `(cx, cy)`, outer radius `r`, a hole in the middle.
    fn gear(&mut self, cx: f32, cy: f32, r: f32, teeth: usize, fill: Fill) -> &mut Self {
        let inner = r * 0.78;
        let mut d = String::new();
        for i in 0..teeth {
            let a0 = i as f32 / teeth as f32 * std::f32::consts::TAU;
            let step = std::f32::consts::TAU / teeth as f32;
            for (k, (frac, rad)) in [(0.0, inner), (0.18, r), (0.5, r), (0.68, inner)].into_iter().enumerate() {
                let a = a0 + step * frac;
                let (x, y) = (cx + rad * a.cos(), cy + rad * a.sin());
                d.push_str(&format!("{}{x} {y}", if i == 0 && k == 0 { "M" } else { "L" }));
            }
        }
        d.push('Z');
        self.path(&d, fill);
        self.circle(cx, cy, r * 0.32, Fill::Paper)
    }

    /// One wing pair of the brand butterfly, on the left of the body (website `Wings`).
    fn wings(&mut self) {
        self.path("M57 40C48 26 30 12 16 14C6 16 8 32 16 42C22 50 36 54 57 53Z", Fill::Tint);
        self.path("M57 56C42 56 26 62 24 74C22 86 34 92 44 86C52 81 56 72 58 64Z", Fill::Accent);
        self.line("M52 44L40 36H30").line("M50 49H34L27 43").line("M52 62L42 70").line("M58 30C54 22 50 16 44 12");
        self.dot(29.0, 36.0, 2.4).dot(26.0, 42.0, 2.4).dot(41.0, 71.0, 2.4).dot(43.0, 11.0, 2.6);
    }

    /// The brand butterfly, the art logo, in a `0 0 120 100` box.
    fn butterfly(&mut self) {
        self.wings();
        self.with([-1.0, 0.0, 0.0, 1.0, 120.0, 0.0], Self::wings);
        self.rect(56.0, 30.0, 8.0, 52.0, 4.0, Fill::Paper);
        self.line("M60 82V92");
        self.dot(60.0, 94.0, 2.6);
    }

    /// One wing pair of snout's mark: veins that are branches, each ending in a commit.
    fn git_wings(&mut self) {
        self.path("M60 44C50 28 30 12 16 14C6 16 8 32 16 42C22 50 36 54 60 53Z", Fill::Tint);
        self.path("M60 58C44 58 26 62 24 74C22 86 34 92 44 86C52 81 58 74 60 68Z", Fill::Accent);
        self.line("M60 50C50 50 46 36 33 36").line("M60 66C52 66 50 74 42 74").line("M58 30C54 22 50 16 44 12");
        self.circle(29.5, 36.0, 3.6, Fill::Paper).circle(38.5, 74.0, 3.6, Fill::Paper);
        self.dot(43.0, 11.0, 2.2);
    }

    /// snout's mark, the app icon's drawing: the brand butterfly with a commit chain for a
    /// body, its newest commit green. A `0 0 120 100` box, like the butterfly.
    fn git_butterfly(&mut self) {
        self.git_wings();
        self.with([-1.0, 0.0, 0.0, 1.0, 120.0, 0.0], Self::git_wings);
        self.line("M60 32V92");
        self.circle(60.0, 34.0, 4.6, Fill::Accent);
        for y in [50.0, 66.0, 82.0] {
            self.circle(60.0, y, 4.2, Fill::Paper);
        }
    }
}

// ----- Path data ---------------------------------------------------------------------------

/// Parse SVG path data into flattened subpaths. Supports `M L H V C Q Z` in both cases.
fn parse_path(d: &str) -> Vec<(Vec<Pos2>, bool)> {
    let mut tokens = Tokens { s: d.as_bytes(), i: 0 };
    let mut out: Vec<(Vec<Pos2>, bool)> = Vec::new();
    let mut cur: Vec<Pos2> = Vec::new();
    let mut at = Pos2::ZERO;
    let mut start = Pos2::ZERO;
    let mut cmd = b'M';
    let flush = |cur: &mut Vec<Pos2>, out: &mut Vec<(Vec<Pos2>, bool)>, closed: bool| {
        if cur.len() > 1 {
            out.push((std::mem::take(cur), closed));
        }
        cur.clear();
    };
    while let Some(c) = tokens.command() {
        if let Some(c) = c {
            cmd = c;
            if c == b'Z' || c == b'z' {
                flush(&mut cur, &mut out, true);
                at = start;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { at.to_vec2() } else { Vec2::ZERO };
        let pt = |t: &mut Tokens| Some(pos2(t.number()?, t.number()?) + base);
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let Some(p) = pt(&mut tokens) else { break };
                flush(&mut cur, &mut out, false);
                cur.push(p);
                (at, start) = (p, p);
                // Further pairs after a move are line-tos.
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let Some(p) = pt(&mut tokens) else { break };
                cur.push(p);
                at = p;
            }
            b'H' => {
                let Some(x) = tokens.number() else { break };
                at = pos2(if rel { at.x + x } else { x }, at.y);
                cur.push(at);
            }
            b'V' => {
                let Some(y) = tokens.number() else { break };
                at = pos2(at.x, if rel { at.y + y } else { y });
                cur.push(at);
            }
            b'C' => {
                let (Some(c1), Some(c2), Some(p)) = (pt(&mut tokens), pt(&mut tokens), pt(&mut tokens)) else { break };
                for k in 1..=16 {
                    let t = k as f32 / 16.0;
                    let u = 1.0 - t;
                    let v = at.to_vec2() * (u * u * u)
                        + c1.to_vec2() * (3.0 * u * u * t)
                        + c2.to_vec2() * (3.0 * u * t * t)
                        + p.to_vec2() * (t * t * t);
                    cur.push(v.to_pos2());
                }
                at = p;
            }
            b'Q' => {
                let (Some(c1), Some(p)) = (pt(&mut tokens), pt(&mut tokens)) else { break };
                for k in 1..=12 {
                    let t = k as f32 / 12.0;
                    let u = 1.0 - t;
                    let v = at.to_vec2() * (u * u) + c1.to_vec2() * (2.0 * u * t) + p.to_vec2() * (t * t);
                    cur.push(v.to_pos2());
                }
                at = p;
            }
            _ => break,
        }
        if cur.is_empty() {
            cur.push(at);
        }
    }
    flush(&mut cur, &mut out, false);
    out
}

struct Tokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl Tokens<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b',' | b'\n' | b'\t') {
            self.i += 1;
        }
    }

    /// `Some(Some(c))` for a command letter, `Some(None)` when a number follows (the previous
    /// command repeats), `None` at the end.
    fn command(&mut self) -> Option<Option<u8>> {
        self.skip();
        let c = *self.s.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(Some(c))
        } else {
            Some(None)
        }
    }

    fn number(&mut self) -> Option<f32> {
        self.skip();
        let start = self.i;
        let mut seen_dot = false;
        while let Some(&c) = self.s.get(self.i) {
            let sign = (c == b'-' || c == b'+') && self.i == start;
            let dot = c == b'.' && !seen_dot;
            if !(c.is_ascii_digit() || sign || dot) {
                break;
            }
            seen_dot |= dot;
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()
    }
}

/// Ear-clipping triangulation of a simple polygon, appended to `out` three points at a time.
fn triangulate(poly: &[Pos2], out: &mut Vec<Pos2>) {
    let mut p: Vec<Pos2> = Vec::with_capacity(poly.len());
    for &q in poly {
        if p.last().is_none_or(|l: &Pos2| l.distance(q) > 1e-3) {
            p.push(q);
        }
    }
    while p.len() > 1 && p[0].distance(*p.last().unwrap()) <= 1e-3 {
        p.pop();
    }
    if p.len() < 3 {
        return;
    }
    let area: f32 =
        (0..p.len()).map(|i| p[i].to_vec2().x * p[(i + 1) % p.len()].y - p[(i + 1) % p.len()].x * p[i].y).sum();
    if area < 0.0 {
        p.reverse();
    }
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b - a).x * (c - b).y - (b - a).y * (c - b).x;
    // A point on the triangle's edge blocks the ear too, or a vertex lying on a diagonal
    // would let two triangles overlap.
    let inside = |q: Pos2, a: Pos2, b: Pos2, c: Pos2| {
        cross(a, b, q) >= -1e-6 && cross(b, c, q) >= -1e-6 && cross(c, a, q) >= -1e-6
    };

    let mut idx: Vec<usize> = (0..p.len()).collect();
    while idx.len() > 3 {
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (p[ia], p[ib], p[ic]);
            let turn = cross(a, b, c);
            if turn.abs() < 1e-6 {
                idx.remove(i); // collinear: no triangle
                clipped = true;
                break;
            }
            if turn < 0.0 {
                continue; // reflex
            }
            if idx.iter().any(|&j| j != ia && j != ib && j != ic && inside(p[j], a, b, c)) {
                continue;
            }
            out.extend([a, b, c]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break; // not simple; fan what is left
        }
    }
    for k in 1..idx.len().saturating_sub(1) {
        out.extend([p[idx[0]], p[idx[k]], p[idx[k + 1]]]);
    }
}

// ----- Icons -------------------------------------------------------------------------------

/// Toolbar and button icons: the button's own ink plus one green fill, 24-unit box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    Open,
    Pull,
    Fetch,
    Push,
    Stage,
    Unstage,
    Discard,
    Refresh,
    Switch,
    Branch,
    Web,
    Spark,
    Back,
    Commit,
    Theme,
    Alert,
    Trash,
    Tag,
    Search,
    Plus,
    Stop,
    Close,
    Repo,
    Folder,
}

impl Glyph {
    pub const ALL: [Self; 24] = [
        Self::Open,
        Self::Pull,
        Self::Fetch,
        Self::Push,
        Self::Stage,
        Self::Unstage,
        Self::Discard,
        Self::Refresh,
        Self::Switch,
        Self::Branch,
        Self::Web,
        Self::Spark,
        Self::Back,
        Self::Commit,
        Self::Theme,
        Self::Alert,
        Self::Trash,
        Self::Tag,
        Self::Search,
        Self::Plus,
        Self::Stop,
        Self::Close,
        Self::Repo,
        Self::Folder,
    ];

    pub fn drawing(self) -> &'static Drawing {
        static ALL: LazyLock<Vec<Drawing>> = LazyLock::new(|| Glyph::ALL.iter().map(|g| g.build()).collect());
        &ALL[self as usize]
    }

    fn build(self) -> Drawing {
        let mut d = Drawing::new(24.0, 24.0, 1.8);
        let tray = "M4 14.5V18.5Q4 20 5.5 20H18.5Q20 20 20 18.5V14.5";
        match self {
            Self::Open => {
                d.path("M3 6.5Q3 5 4.5 5H9L11 7H19.5Q21 7 21 8.5V18Q21 19.5 19.5 19.5H4.5Q3 19.5 3 18Z", Fill::None);
                d.path("M3 11H21V18Q21 19.5 19.5 19.5H4.5Q3 19.5 3 18Z", Fill::Accent);
            }
            Self::Pull => {
                d.shape("M4.5 15H19.5V19.5H4.5Z", Fill::Accent);
                d.line(tray).line("M12 3.5V14").line("M7.5 9.5L12 14L16.5 9.5");
            }
            Self::Push => {
                d.shape("M4.5 15H19.5V19.5H4.5Z", Fill::Accent);
                d.line(tray).line("M12 14V3.5").line("M7.5 8L12 3.5L16.5 8");
            }
            Self::Fetch => {
                d.path(
                    "M7 15.5C3.6 15.5 3 11 6.4 10.3C6.8 6.4 12.6 5 14.8 8.6C18.2 8.1 20.6 10.6 19.6 13.4C19.3 14.7 18.4 15.5 17 15.5Z",
                    Fill::Accent,
                );
                d.line("M12 11V21").line("M9 18L12 21L15 18");
            }
            Self::Stage => {
                d.rect(4.0, 4.0, 16.0, 16.0, 3.5, Fill::Accent);
                d.line("M12 8.5V15.5").line("M8.5 12H15.5");
            }
            Self::Unstage => {
                d.rect(4.0, 4.0, 16.0, 16.0, 3.5, Fill::None);
                d.line("M8.5 12H15.5");
            }
            Self::Discard => {
                d.line("M9 4.5L4.5 9L9 13.5");
                d.line("M4.5 9H14C17 9 19.5 11.5 19.5 14.5C19.5 17.5 17 20 14 20H9");
            }
            Self::Refresh => {
                d.arc(12.0, 12.0, 7.5, 50.0, 320.0);
                d.line("M18.3 3.6L17.9 7.4L14.1 7.1");
            }
            Self::Switch => {
                d.line("M4 8H18").line("M14.5 4.5L18 8L14.5 11.5");
                d.line("M20 16H6").line("M9.5 12.5L6 16L9.5 19.5");
            }
            Self::Branch => {
                d.line("M7 7.5V16.5").line("M17 10.5C17 14.5 7 13 7 16.5");
                d.circle(7.0, 5.5, 2.2, Fill::None).circle(7.0, 18.5, 2.2, Fill::None);
                d.circle(17.0, 8.5, 2.4, Fill::Accent);
            }
            Self::Web => {
                d.ellipse(pos2(12.0, 12.0), 8.5, 8.5, Fill::Accent, true);
                d.ellipse(pos2(12.0, 12.0), 3.6, 8.5, Fill::None, true);
                d.line("M3.5 12H20.5");
            }
            Self::Spark => {
                d.path("M11 3Q12.4 9.6 19 11Q12.4 12.4 11 19Q9.6 12.4 3 11Q9.6 9.6 11 3Z", Fill::Accent);
                d.spark(19.0, 19.0, 0.5);
            }
            Self::Back => {
                d.line("M14.5 5L7.5 12L14.5 19");
            }
            Self::Commit => {
                d.line("M2.5 12H8").line("M16 12H21.5");
                d.circle(12.0, 12.0, 4.0, Fill::Accent);
            }
            Self::Theme => {
                d.ellipse(pos2(12.0, 12.0), 8.0, 8.0, Fill::None, true);
                let mut half: Vec<Pos2> = (0..=24)
                    .map(|i| {
                        let a = (90.0 + 180.0 * i as f32 / 24.0f32).to_radians();
                        pos2(12.0 + 8.0 * a.cos(), 12.0 + 8.0 * a.sin())
                    })
                    .collect();
                half.push(pos2(12.0, 4.0));
                d.add(vec![(half, true)], Fill::Accent, Line::Ink);
            }
            Self::Alert => {
                d.path("M10.3 4.4Q12 1.6 13.7 4.4L21.2 17.6Q22.6 20 19.9 20H4.1Q1.4 20 2.8 17.6Z", Fill::None);
                d.line("M12 9V13.5");
                d.dot(12.0, 16.6, 1.3);
            }
            Self::Trash => {
                d.line("M3.5 6.5H20.5").line("M9 6.5V4.5Q9 3.5 10 3.5H14Q15 3.5 15 4.5V6.5");
                d.path("M5.5 6.5L6.6 19Q6.8 20.5 8.3 20.5H15.7Q17.2 20.5 17.4 19L18.5 6.5", Fill::None);
                d.line("M10 10.5V16.5").line("M14 10.5V16.5");
            }
            Self::Tag => {
                d.path("M3.5 4.9Q3.5 3.5 4.9 3.5H11.2L20.5 12.8L12.8 20.5L3.5 11.2Z", Fill::Accent);
                d.circle(8.0, 8.0, 1.6, Fill::Paper);
            }
            Self::Plus => {
                d.line("M12 5V19").line("M5 12H19");
            }
            Self::Stop => {
                d.rect(6.0, 6.0, 12.0, 12.0, 3.0, Fill::Tint);
            }
            Self::Close => {
                d.line("M7 7L17 17").line("M17 7L7 17");
            }
            Self::Repo => {
                d.line("M5.5 18.5V5.5Q5.5 3.5 7.5 3.5H18.5V16.5H7.5Q5.5 16.5 5.5 18.5Q5.5 20.5 7.5 20.5H18.5");
                d.path("M9.5 3.5H13.5V10.5L11.5 9L9.5 10.5Z", Fill::Accent);
            }
            Self::Folder => {
                d.path("M3 6.5Q3 5 4.5 5H9.2L11.2 7H19.5Q21 7 21 8.5V17.5Q21 19 19.5 19H4.5Q3 19 3 17.5Z", Fill::None);
                d.path("M3 6.5Q3 5 4.5 5H9.2L11.2 7H19.5Q21 7 21 8.5V10H3Z", Fill::Accent);
            }
            Self::Search => {
                d.ellipse(pos2(10.5, 10.5), 6.5, 6.5, Fill::Accent, true);
                d.line("M15.3 15.3L20.5 20.5");
            }
        }
        d
    }
}

// ----- Drawings ----------------------------------------------------------------------------

/// Shared ground of a spot drawing: the website's door-card blob and its horizon.
fn spot(d: &mut Drawing) {
    d.shape(
        "M30 100C24 62 66 32 122 36C180 40 222 68 212 108C204 142 154 152 110 148C66 144 36 132 30 100Z",
        Fill::Blob,
    );
    d.ground("M44 146C84 142 160 142 200 146");
}

/// The welcome screen: a laptop whose screen shows a branch leaving and rejoining, its newest
/// commit the green one, with the butterfly over it.
pub fn welcome() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(300.0, 190.0, 2.5);
        d.shape(
            "M34 122C24 74 76 34 150 38C226 42 282 78 270 128C260 168 196 180 140 176C86 172 42 160 34 122Z",
            Fill::Blob,
        );
        d.ground("M44 176C100 171 200 171 258 176");
        d.rotated(-2.0, 150.0, 100.0, |d| {
            d.rect(78.0, 48.0, 144.0, 96.0, 8.0, Fill::Paper);
            d.path("M78 62V56Q78 48 86 48H214Q222 48 222 56V62Z", Fill::Tint);
            d.dot(88.0, 55.0, 1.8).dot(96.0, 55.0, 1.8).dot(104.0, 55.0, 1.8);
            d.line("M96 116H204");
            d.line("M118 116C130 116 130 88 144 88H170C184 88 184 116 196 116");
            d.circle(100.0, 116.0, 5.0, Fill::Paper).circle(140.0, 116.0, 5.0, Fill::Paper);
            d.circle(144.0, 88.0, 5.0, Fill::Paper).circle(170.0, 88.0, 6.0, Fill::Accent);
            d.circle(196.0, 116.0, 5.0, Fill::Paper);
        });
        d.path("M62 148H238L254 166H46Z", Fill::Paper);
        d.line("M134 157H166");
        d.rotated(14.0, 250.0, 40.0, |d| d.placed(222.0, 14.0, 0.5, Drawing::butterfly));
        d.spark(54.0, 60.0, 1.0).spark(272.0, 112.0, 0.6).spark(96.0, 26.0, 0.55);
        d
    });
    &D
}

/*---------------------------------------------------------------------------------------------
 *  Copyright (c) 2026 SNOUTDATA INC. All rights reserved.
 *  Licensed under the Apache License, Version 2.0. See LICENSE in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
// Ported from Studio's art (`Illustrations.tsx` and the `ill-twinkle` keyframes in its
// styles.css): a spark twinkles over 4.8 s, easing in and out, down to 70 % of its size and
// a quarter turn round at the middle; every other spark runs half a period behind. Reduced
// motion stops it (the front end passes no time then).

/// How the `index`th spark of a drawing stands `secs` seconds in: its scale and its turn in
/// degrees.
pub fn twinkle(index: usize, secs: f32) -> (f32, f32) {
    const PERIOD: f32 = 4.8;
    let t = (secs + if index % 2 == 1 { PERIOD / 2.0 } else { 0.0 }).rem_euclid(PERIOD) / PERIOD;
    // 0 → 1 → 0 over the period, eased in and out at both ends.
    let half = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
    let eased = half * half * (3.0 - 2.0 * half);
    (1.0 - 0.3 * eased, 45.0 * eased)
}

/// The four-point spark's outline at `(x, y)`, scale `s`, turned `deg` degrees: for a
/// front end drawing a twinkling spark.
pub fn spark_outline(x: f32, y: f32, s: f32, deg: f32) -> Vec<Pos2> {
    let (a, b) = (7.0 * s, 1.4 * s);
    let d = format!("M0 {}Q{b} {} {a} 0Q{b} {b} 0 {a}Q{} {b} {} 0Q{} {} 0 {}Z", -a, -b, -b, -a, -b, -b, -a);
    let (sin, cos) = deg.to_radians().sin_cos();
    parse_path(&d)
        .into_iter()
        .next()
        .map(|(pts, _)| pts.into_iter().map(|p| pos2(x + p.x * cos - p.y * sin, y + p.x * sin + p.y * cos)).collect())
        .unwrap_or_default()
}

/// The splash and the background tasks: SnoutCode building. A laptop with code on its
/// screen, and the gear it turns (the green, it is what the picture is about) with a smaller
/// one beside it, under twinkling sparks. A `0 0 240 160` spot box.
pub fn building() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(240.0, 160.0, 2.5);
        spot(&mut d);
        d.rotated(-2.0, 100.0, 90.0, |d| {
            d.rect(44.0, 50.0, 112.0, 74.0, 6.0, Fill::Paper);
            d.path("M44 62V56Q44 50 50 50H150Q156 50 156 56V62Z", Fill::Tint);
            d.dot(52.0, 56.0, 1.6).dot(59.0, 56.0, 1.6);
            d.line("M56 76H92").line("M64 88H124").line("M64 100H110").line("M56 112H80");
        });
        d.path("M34 126H166L178 140H22Z", Fill::Paper);
        d.line("M88 133H112");
        d.gear(178.0, 70.0, 22.0, 9, Fill::Accent);
        d.gear(204.0, 104.0, 14.0, 7, Fill::Tint);
        d.twinkling_spark(36.0, 34.0, 0.8).twinkling_spark(214.0, 30.0, 0.65).twinkling_spark(130.0, 26.0, 0.5);
        d
    });
    &D
}

/// The house spark on its own, `0 0 24 24`, for ornaments.
pub fn sparkle() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(24.0, 24.0, 1.8);
        d.spark(12.0, 12.0, 1.5);
        d
    });
    &D
}

/// snout's mark on its own, `0 0 120 100`.
pub fn mark() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(120.0, 100.0, 3.2);
        d.git_butterfly();
        d
    });
    &D
}

/// The About banner, a wide `0 0 440 124` box: the mark over what snout holds, each on its
/// own thread: your repositories, the diff, the history (the green, it is what snout draws
/// best) and the agent beside it.
pub fn about() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(440.0, 124.0, 2.5);
        d.shape(
            "M22 86C16 54 80 32 160 36C240 40 330 26 398 44C436 54 434 98 404 108C340 122 120 122 54 114C32 110 24 100 22 86Z",
            Fill::Blob,
        );
        d.ground("M40 116C140 112 300 112 400 116");
        d.ground("M204 40C160 46 104 54 76 68").ground("M212 48C196 54 178 58 164 64");
        d.ground("M228 48C246 54 270 60 290 70").ground("M236 40C290 46 352 54 380 66");
        // Your repositories.
        d.path("M46 78Q46 72 52 72H62L68 78H88Q94 78 94 84V102Q94 108 88 108H52Q46 108 46 102Z", Fill::Tint);
        d.path("M46 88Q46 84 50 84H90Q94 84 94 88V102Q94 108 88 108H52Q46 108 46 102Z", Fill::Paper);
        // The diff.
        d.rotated(-4.0, 160.0, 88.0, |d| {
            d.rect(142.0, 64.0, 36.0, 46.0, 3.0, Fill::Paper);
            d.shape("M146 80H174V88H146Z", Fill::Tint);
            d.line("M150 74H170").line("M150 84H166").line("M150 94H170").line("M150 103H162");
        });
        // The history.
        d.line("M258 102H324");
        d.line("M272 102C282 102 280 82 292 82H304C314 82 312 102 322 102");
        d.circle(262.0, 102.0, 4.5, Fill::Paper).circle(292.0, 82.0, 4.5, Fill::Paper);
        d.circle(304.0, 82.0, 5.5, Fill::Accent).circle(322.0, 102.0, 4.5, Fill::Paper);
        // The agent.
        d.rect(352.0, 68.0, 58.0, 42.0, 6.0, Fill::Paper);
        d.path("M352 78V74Q352 68 358 68H404Q410 68 410 74V78Z", Fill::Tint);
        d.line("M361 87L369 93L361 99").line("M374 99H390");
        d.placed(190.0, 0.0, 0.5, Drawing::git_butterfly);
        d.spark(118.0, 24.0, 0.7).spark(322.0, 22.0, 0.8).spark(30.0, 52.0, 0.5).spark(420.0, 34.0, 0.5);
        d
    });
    &D
}

/// Nothing to commit: a sprout in a clay pot.
pub fn clean() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(240.0, 160.0, 2.5);
        spot(&mut d);
        d.line("M120 104V70");
        d.path("M120 86C106 86 96 78 94 64C108 64 118 72 120 86Z", Fill::Accent);
        d.path("M120 76C122 60 132 52 148 52C146 68 136 76 120 76Z", Fill::Accent);
        d.path("M98 112H142L136 144H104Z", Fill::Clay);
        d.rect(92.0, 102.0, 56.0, 12.0, 4.0, Fill::Clay);
        d.spark(70.0, 64.0, 0.9).spark(174.0, 92.0, 0.6).spark(160.0, 40.0, 0.5);
        d
    });
    &D
}

/// Pick something: a page, one line marked, and the pointer reaching for it.
pub fn pick() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(240.0, 160.0, 2.5);
        spot(&mut d);
        d.rotated(-3.0, 118.0, 88.0, |d| {
            d.path("M80 30H132L154 52V140H80Z", Fill::Paper);
            d.line("M132 30V52H154");
            d.shape("M90 86H146V98H90Z", Fill::Accent);
            d.line("M94 66H126").line("M94 79H140").line("M94 92H138").line("M94 105H128").line("M94 118H118");
        });
        d.path("M150 100V132L158 125L164 138L171 135L165 122H175Z", Fill::Paper);
        d.spark(62.0, 58.0, 0.9).spark(186.0, 66.0, 0.6);
        d
    });
    &D
}

/// No history yet: a seed just in the ground, one green leaf up.
pub fn seed() -> &'static Drawing {
    static D: LazyLock<Drawing> = LazyLock::new(|| {
        let mut d = Drawing::new(240.0, 100.0, 2.5);
        d.shape("M40 70C40 44 80 30 120 32C164 34 202 48 200 70C198 92 160 96 120 96C80 96 40 92 40 70Z", Fill::Blob);
        d.path("M70 80C90 72 150 72 170 80Z", Fill::Tint);
        d.line("M120 76V54");
        d.path("M120 62C122 48 132 42 146 42C144 56 134 62 120 62Z", Fill::Accent);
        d.spark(84.0, 46.0, 0.7).spark(160.0, 64.0, 0.5);
        d
    });
    &D
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paths() {
        let p = parse_path("M10 20H30V40L10 40Z m5 5 l1 1");
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].0, [pos2(10.0, 20.0), pos2(30.0, 20.0), pos2(30.0, 40.0), pos2(10.0, 40.0)]);
        assert!(p[0].1);
        assert_eq!(p[1].0, [pos2(15.0, 25.0), pos2(16.0, 26.0)]);
        assert_eq!(parse_path("M0 0Q5 5 10 0")[0].0.len(), 13);
    }

    #[test]
    fn triangulates_concave_polygons() {
        // An L shape: area 3, concave at (1, 1).
        let l = [pos2(0.0, 0.0), pos2(2.0, 0.0), pos2(2.0, 1.0), pos2(1.0, 1.0), pos2(1.0, 2.0), pos2(0.0, 2.0)];
        let mut tris = Vec::new();
        triangulate(&l, &mut tris);
        let area: f32 = tris
            .chunks(3)
            .map(|t| ((t[1] - t[0]).x * (t[2] - t[0]).y - (t[1] - t[0]).y * (t[2] - t[0]).x).abs() / 2.0)
            .sum();
        assert!((area - 3.0).abs() < 1e-4, "area {area}");
    }

    #[test]
    fn sparks_twinkle_half_a_period_apart() {
        assert_eq!(twinkle(0, 0.0), (1.0, 0.0));
        let (scale, turn) = twinkle(0, 2.4);
        assert!((scale - 0.7).abs() < 1e-4 && (turn - 45.0).abs() < 1e-3, "{scale} {turn}");
        assert_eq!(twinkle(1, 0.0), twinkle(0, 2.4));
        assert_eq!(twinkle(0, 4.8), twinkle(0, 0.0));
        assert_eq!(spark_outline(10.0, 10.0, 1.0, 0.0).first().copied(), Some(pos2(10.0, 3.0)));
        assert_eq!(building().sparks.len(), 3);
    }

    #[test]
    fn every_drawing_builds() {
        for g in Glyph::ALL {
            assert!(!g.drawing().parts.is_empty(), "{g:?}");
        }
        for d in [welcome(), mark(), about(), clean(), pick(), seed(), sparkle(), building()] {
            assert!(d.parts.iter().any(|p| !p.tris.is_empty()));
        }
    }
}

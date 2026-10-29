//! The spot drawings of the dialogs and the empty panels, in the house style of the rest of
//! `art`: a `0 0 240 160` box, the blob and its horizon, objects in `Paper` and `Tint`, ONE
//! `Accent` on the thing the picture is about (or the `Clay` of trouble, the one place a
//! drawing borrows the error's colour) and a few twinkling sparks.

use std::sync::LazyLock;

use super::{Drawing, Fill, Line, Pos2, parse_path, pos2, spot};

// ----- Parts ------------------------------------------------------------------------------

/// A page with a folded corner and a few lines, its top-left at (x, y).
fn sheet(d: &mut Drawing, x: f32, y: f32, w: f32, h: f32, fill: Fill, lines: usize) {
    let f = (w / 4.0).min(10.0);
    d.path(&format!("M{x} {y}H{}L{} {}V{}H{x}Z", x + w - f, x + w, y + f, y + h), fill);
    d.line(&format!("M{} {y}V{}H{}", x + w - f, y + f, x + w));
    for i in 0..lines {
        let end = x + w - if i % 2 == 1 { 16.0 } else { 10.0 };
        d.line(&format!("M{} {}H{end}", x + 8.0, y + 18.0 + i as f32 * 9.0));
    }
}

/// A laptop: the screen at (x, y), `w` by `h`, its base under it.
fn laptop(d: &mut Drawing, x: f32, y: f32, w: f32, h: f32) {
    d.rect(x, y, w, h, 6.0, Fill::Paper);
    d.path(
        &format!("M{} {}H{}L{} {}H{}Z", x - 10.0, y + h, x + w + 10.0, x + w + 20.0, y + h + 12.0, x - 20.0),
        Fill::Tint,
    );
}

/// A server: three units stacked from (x, y), each with its light and a slot.
fn server(d: &mut Drawing, x: f32, y: f32, w: f32) {
    for k in 0..3 {
        let top = y + k as f32 * 30.0;
        d.rect(x, top, w, 26.0, 5.0, if k == 1 { Fill::Tint } else { Fill::Paper });
        d.dot(x + 11.0, top + 13.0, 2.2);
        d.line(&format!("M{} {}H{}", x + 22.0, top + 13.0, x + w - 10.0));
    }
}

/// The website's cloud, moved by (dx, dy) and scaled by `s`.
fn cloud(d: &mut Drawing, dx: f32, dy: f32, s: f32, fill: Fill) {
    d.with([s, 0.0, 0.0, s, dx, dy], |d| {
        d.path(
            "M140 66C128 66 121 57 124 48C126 40 135 36 143 39C146 27 159 20 172 23C184 25 192 34 194 43C204 40 216 46 217 57C218 65 211 70 202 70Z",
            fill,
        );
    });
}

/// A round badge at (cx, cy) with a tick in it.
fn tick_badge(d: &mut Drawing, cx: f32, cy: f32, r: f32, fill: Fill) {
    d.circle(cx, cy, r, fill);
    let k = r / 12.0;
    d.line(&format!("M{} {}L{} {}L{} {}", cx - 6.0 * k, cy, cx - 2.0 * k, cy + 4.0 * k, cx + 6.0 * k, cy - 5.0 * k));
}

/// A round badge at (cx, cy) with a plus in it.
fn plus_badge(d: &mut Drawing, cx: f32, cy: f32, r: f32) {
    d.circle(cx, cy, r, Fill::Accent);
    let k = r * 0.5;
    d.line(&format!("M{cx} {}V{}M{} {cy}H{}", cy - k, cy + k, cx - k, cx + k));
}

/// A dotted thread along the path `p`: short dashes with round ends, every 8 units.
fn thread(d: &mut Drawing, p: &str) {
    for (pts, _) in parse_path(p) {
        let mut dashes: Vec<(Vec<Pos2>, bool)> = Vec::new();
        let mut walked = 0.0_f32;
        let mut next = 0.0_f32;
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.distance(b);
            while next <= walked + len && len > 0.0 {
                let t = (next - walked) / len;
                let at = a + (b - a) * t;
                let dir = (b - a) * (1.0 / len);
                dashes.push((vec![at, at + dir * 1.5], false));
                next += 8.0;
            }
            walked += len;
        }
        d.add(dashes, Fill::None, Line::Ink);
    }
}

/// A magnifying glass: the lens at (cx, cy), radius `r`, its handle down to the right.
fn lens(d: &mut Drawing, cx: f32, cy: f32, r: f32, glass: Fill, handle: Fill) {
    let k = std::f32::consts::FRAC_1_SQRT_2;
    let (hx, hy) = (cx + r * k, cy + r * k);
    d.rotated(45.0, hx, hy, |d| {
        d.rect(hx + 2.0, hy - 4.0, 24.0, 8.0, 4.0, handle);
    });
    d.circle(cx, cy, r, glass);
    d.arc(cx, cy, r - 6.0, 200.0, 250.0);
}

/// A jolt: a short zigzag from (x, y), down and to the right.
fn jolt(d: &mut Drawing, x: f32, y: f32) {
    d.line(&format!("M{x} {y}L{} {}L{x} {}L{} {}", x + 6.0, y + 5.0, y + 9.0, x + 6.0, y + 14.0));
}

/// Two little z's for something asleep, the big one at (x, y).
fn asleep(d: &mut Drawing, x: f32, y: f32) {
    d.line(&format!("M{x} {y}H{}L{x} {}H{}", x + 10.0, y + 10.0, x + 10.0));
    d.line(&format!("M{} {}H{}L{} {}H{}", x + 16.0, y - 12.0, x + 22.0, x + 16.0, y - 6.0, x + 22.0));
}

/// A drawing built once by `f` on the spot ground.
fn spot_drawing(f: impl FnOnce(&mut Drawing)) -> Drawing {
    let mut d = Drawing::new(240.0, 160.0, 2.5);
    spot(&mut d);
    f(&mut d);
    d
}

macro_rules! drawing {
    ($(#[$doc:meta])* $name:ident, $build:expr) => {
        $(#[$doc])*
        pub fn $name() -> &'static Drawing {
            static D: LazyLock<Drawing> = LazyLock::new(|| spot_drawing($build));
            &D
        }
    };
}

// ----- Dialogs -----------------------------------------------------------------------------

drawing!(
    /// Discard or delete: a bin with its lid lifted, a page of changes on its way in (its
    /// changed lines the clay of what is lost), and the undo arrow it cannot take back.
    discard,
    |d| {
        d.rotated(16.0, 152.0, 52.0, |d| {
            sheet(d, 130.0, 20.0, 44.0, 56.0, Fill::Paper, 0);
            d.shape("M136 44H166V54H136Z", Fill::Clay);
            d.line("M138 38H160").line("M138 49H164").line("M138 60H156");
        });
        d.path("M84 80H156L148 142H92Z", Fill::Tint);
        d.line("M108 94V130").line("M120 94V130").line("M132 94V130");
        d.rotated(-26.0, 82.0, 78.0, |d| {
            d.rect(78.0, 68.0, 84.0, 11.0, 4.0, Fill::Paper);
            d.line("M108 68V63Q108 59 112 59H128Q132 59 132 63V68");
        });
        d.line("M30 44H58C68 44 74 50 74 58C74 66 68 72 58 72H48").line("M38 36L30 44L38 52");
        d.twinkling_spark(196.0, 112.0, 0.7).twinkling_spark(44.0, 110.0, 0.55).twinkling_spark(206.0, 28.0, 0.5);
    }
);

drawing!(
    /// A push the remote turned back: your newest commit, the green, sent up towards the
    /// remote's cloud and stopped by the clay barrier under it.
    push_rejected,
    |d| {
        laptop(d, 30.0, 86.0, 70.0, 44.0);
        d.line("M42 110H86");
        d.circle(46.0, 110.0, 4.0, Fill::Paper).circle(64.0, 110.0, 4.0, Fill::Paper);
        d.circle(84.0, 110.0, 5.5, Fill::Accent);
        cloud(d, 6.0, -10.0, 1.0, Fill::Paper);
        d.line("M100 106L136 88").line("M127.4 85.5L136 88L132.1 96.1");
        d.rect(120.0, 66.0, 60.0, 11.0, 3.0, Fill::Clay);
        d.line("M134 66L128 77").line("M148 66L142 77").line("M162 66L156 77").line("M176 66L170 77");
        d.line("M146 84L150 92").line("M156 82L164 88").line("M114 80L120 84");
        d.twinkling_spark(36.0, 46.0, 0.7).twinkling_spark(212.0, 112.0, 0.6).twinkling_spark(112.0, 28.0, 0.5);
    }
);

drawing!(
    /// A merge that conflicts: two lines of history meeting in a clay knot, a jolt over it,
    /// and the file both sides changed.
    merge_conflict,
    |d| {
        d.rotated(6.0, 196.0, 84.0, |d| sheet(d, 172.0, 54.0, 44.0, 60.0, Fill::Paper, 4));
        d.line("M30 114H104C122 114 128 90 146 90");
        d.line("M52 114C66 114 64 66 82 66H104C122 66 128 90 146 90");
        d.circle(34.0, 114.0, 4.5, Fill::Paper).circle(78.0, 114.0, 5.0, Fill::Paper);
        d.circle(92.0, 66.0, 5.0, Fill::Paper);
        d.circle(150.0, 90.0, 10.0, Fill::Clay);
        d.line("M146 86L154 94").line("M154 86L146 94");
        d.line("M156 52L148 66H158L150 78");
        d.twinkling_spark(40.0, 52.0, 0.7).twinkling_spark(120.0, 26.0, 0.5).twinkling_spark(206.0, 34.0, 0.55);
    }
);

drawing!(
    /// Clone: the remote's cloud, and a dotted thread down to a folder on this computer with
    /// the copy, the green, settling into it; the butterfly flies along.
    clone_repo,
    |d| {
        cloud(d, -96.0, -2.0, 1.0, Fill::Paper);
        d.line("M50 52H76").line("M58 60H70");
        thread(d, "M84 70C92 98 112 108 136 106");
        d.line("M130 100L137 106L130 112");
        d.path("M144 84H166L174 92H206Q212 92 212 98V136H138V90Q138 84 144 84Z", Fill::Tint);
        d.rect(156.0, 70.0, 32.0, 42.0, 4.0, Fill::Accent);
        d.line("M164 70V112");
        d.path("M134 106H212Q218 106 216 112L210 136H140L130 112Q128 106 134 106Z", Fill::Paper);
        d.rotated(14.0, 196.0, 34.0, |d| d.placed(176.0, 16.0, 0.34, Drawing::butterfly));
        d.twinkling_spark(30.0, 104.0, 0.7).twinkling_spark(120.0, 28.0, 0.5).twinkling_spark(98.0, 132.0, 0.5);
    }
);

drawing!(
    /// A new branch: the history running along, and a branch leaving it for a new tip, the
    /// green, with a leaf just sprouting from it.
    new_branch,
    |d| {
        d.line("M28 116H212");
        d.line("M92 116C114 116 112 70 136 70H164");
        d.circle(52.0, 116.0, 5.0, Fill::Paper).circle(92.0, 116.0, 5.0, Fill::Paper);
        d.circle(136.0, 116.0, 5.0, Fill::Paper).circle(180.0, 116.0, 5.0, Fill::Paper);
        d.path("M180 60C184 44 196 38 210 40C208 56 196 62 180 60Z", Fill::Tint);
        d.line("M182 58L204 44");
        d.circle(172.0, 70.0, 10.0, Fill::Accent);
        d.line("M172 65V75").line("M167 70H177");
        d.twinkling_spark(44.0, 52.0, 0.7).twinkling_spark(118.0, 34.0, 0.5).twinkling_spark(210.0, 96.0, 0.55);
    }
);

drawing!(
    /// Stash: the changes put aside in a box for later, the newest page, the green, going in.
    stash,
    |d| {
        d.rotated(-9.0, 98.0, 56.0, |d| sheet(d, 78.0, 30.0, 40.0, 54.0, Fill::Paper, 3));
        d.rotated(8.0, 142.0, 50.0, |d| sheet(d, 122.0, 22.0, 40.0, 54.0, Fill::Accent, 3));
        d.rect(62.0, 86.0, 116.0, 54.0, 6.0, Fill::Paper);
        d.rect(56.0, 78.0, 128.0, 13.0, 4.0, Fill::Tint);
        d.rect(102.0, 104.0, 36.0, 15.0, 3.0, Fill::Tint);
        d.line("M200 34V66").line("M192 58L200 66L208 58");
        d.twinkling_spark(36.0, 54.0, 0.7).twinkling_spark(206.0, 110.0, 0.55).twinkling_spark(176.0, 22.0, 0.5);
    }
);

drawing!(
    /// Connect via SSH: this computer's terminal wired to a server, the key on the wire the
    /// green.
    ssh,
    |d| {
        laptop(d, 26.0, 72.0, 76.0, 50.0);
        d.line("M38 90L46 96L38 102").line("M52 102H68");
        server(d, 158.0, 42.0, 56.0);
        d.line("M104 102C128 102 132 86 158 86");
        d.circle(122.0, 62.0, 9.0, Fill::Accent);
        d.circle(122.0, 62.0, 3.0, Fill::Paper);
        d.path("M131 60H150V65H146V70H142V65H131Z", Fill::Accent);
        d.twinkling_spark(40.0, 44.0, 0.7).twinkling_spark(146.0, 26.0, 0.5).twinkling_spark(222.0, 136.0, 0.5);
    }
);

drawing!(
    /// A JDK on its way: the cup of Java, its coffee the green, and the download coming
    /// down to it from the cloud.
    jdk,
    |d| {
        d.ellipse(pos2(112.0, 140.0), 56.0, 6.0, Fill::Tint, true);
        d.path("M76 84H148V120Q148 138 130 138H94Q76 138 76 120Z", Fill::Paper);
        d.line("M148 94C166 94 166 122 146 122");
        d.ellipse(pos2(112.0, 84.0), 36.0, 6.0, Fill::Accent, true);
        d.line("M98 70C94 62 102 58 98 50").line("M112 68C108 60 116 56 112 48").line("M126 70C122 62 130 58 126 50");
        cloud(d, 108.0, 2.0, 0.5, Fill::Paper);
        d.line("M190 42V78").line("M183 71L190 78L197 71");
        d.twinkling_spark(40.0, 60.0, 0.7).twinkling_spark(206.0, 114.0, 0.55).twinkling_spark(150.0, 30.0, 0.45);
    }
);

drawing!(
    /// Claude reviews: the conversation, with the spark, beside the page of changes it
    /// proposes, and the green tick of a review done.
    claude_review,
    |d| {
        d.rotated(-3.0, 66.0, 62.0, |d| {
            d.path(
                "M36 32H96Q108 32 108 44V70Q108 82 96 82H62L46 96L50 82H36Q24 82 24 70V44Q24 32 36 32Z",
                Fill::Paper,
            );
            d.spark(42.0, 50.0, 0.75);
            d.line("M54 50H96").line("M38 62H92").line("M38 72H72");
        });
        d.rotated(4.0, 158.0, 92.0, |d| {
            sheet(d, 124.0, 46.0, 68.0, 90.0, Fill::Paper, 0);
            d.shape("M128 72H186V82H128Z", Fill::Tint);
            d.line("M134 62H176").line("M134 77H170").line("M134 92H180").line("M134 107H164").line("M134 122H174");
        });
        tick_badge(d, 192.0, 46.0, 12.0, Fill::Accent);
        d.twinkling_spark(36.0, 118.0, 0.65).twinkling_spark(118.0, 24.0, 0.5).twinkling_spark(214.0, 128.0, 0.5);
    }
);

drawing!(
    /// Something to look at twice: a page with the clay warning beside it.
    warning,
    |d| {
        d.rotated(-5.0, 108.0, 84.0, |d| sheet(d, 74.0, 36.0, 64.0, 84.0, Fill::Paper, 4));
        d.path("M160 52L190 106H130Z", Fill::Clay);
        d.line("M160 70V88");
        d.dot(160.0, 97.0, 2.6);
        d.twinkling_spark(40.0, 50.0, 0.7).twinkling_spark(206.0, 34.0, 0.6).twinkling_spark(210.0, 128.0, 0.5);
    }
);

drawing!(
    /// Something to know: a page, and the round green badge with its i.
    info,
    |d| {
        d.rotated(-5.0, 100.0, 84.0, |d| sheet(d, 66.0, 36.0, 64.0, 84.0, Fill::Paper, 4));
        d.circle(162.0, 72.0, 22.0, Fill::Accent);
        d.dot(162.0, 61.0, 2.8);
        d.line("M162 70V84");
        d.twinkling_spark(40.0, 50.0, 0.7).twinkling_spark(206.0, 116.0, 0.6).twinkling_spark(196.0, 32.0, 0.5);
    }
);

drawing!(
    /// Done: a clipboard of ticked lines, the green tick badge, and the butterfly.
    success,
    |d| {
        d.rect(70.0, 36.0, 74.0, 100.0, 6.0, Fill::Paper);
        d.rect(88.0, 28.0, 38.0, 14.0, 4.0, Fill::Tint);
        for y in [62.0, 84.0, 106.0] {
            d.line(&format!("M82 {y}L86 {}L92 {}", y + 4.0, y - 4.0));
            d.line(&format!("M100 {y}H{}", if y == 84.0 { 124.0 } else { 132.0 }));
        }
        tick_badge(d, 158.0, 112.0, 16.0, Fill::Accent);
        d.rotated(12.0, 186.0, 40.0, |d| d.placed(166.0, 20.0, 0.36, Drawing::butterfly));
        d.twinkling_spark(44.0, 56.0, 0.7).twinkling_spark(204.0, 96.0, 0.5).twinkling_spark(44.0, 122.0, 0.5);
    }
);

// ----- Empty panels -------------------------------------------------------------------------

drawing!(
    /// No project open: an open folder with its pages fanned up out of it, the green plus of
    /// the next one, and the butterfly over it.
    no_project,
    |d| {
        d.path("M58 64H96L104 72H170Q176 72 176 78V134H52V70Q52 64 58 64Z", Fill::Tint);
        d.rotated(-10.0, 92.0, 70.0, |d| sheet(d, 74.0, 44.0, 40.0, 52.0, Fill::Paper, 3));
        d.rotated(6.0, 130.0, 66.0, |d| sheet(d, 112.0, 40.0, 40.0, 52.0, Fill::Paper, 3));
        d.path("M46 92H178Q184 92 184 98L178 134H52L40 98Q40 92 46 92Z", Fill::Paper);
        d.line("M100 112H130");
        plus_badge(d, 198.0, 74.0, 12.0);
        d.rotated(-14.0, 50.0, 40.0, |d| d.placed(26.0, 18.0, 0.36, Drawing::butterfly));
        d.twinkling_spark(150.0, 20.0, 0.5).twinkling_spark(214.0, 124.0, 0.6).twinkling_spark(98.0, 24.0, 0.45);
    }
);

drawing!(
    /// Nothing staged: an empty tray, a dotted page above it where a change would be, and
    /// the green plus that stages one.
    nothing_staged,
    |d| {
        thread(d, "M98 36H132L144 48V92H98Z");
        thread(d, "M132 36V48H144");
        d.path("M56 108H92L100 120H140L148 108H184V134Q184 140 178 140H62Q56 140 56 134Z", Fill::Tint);
        plus_badge(d, 196.0, 42.0, 12.0);
        d.line("M196 58C196 84 182 98 160 102").line("M167 96L160 102L168 107");
        d.twinkling_spark(44.0, 56.0, 0.7).twinkling_spark(70.0, 92.0, 0.45).twinkling_spark(212.0, 128.0, 0.5);
    }
);

drawing!(
    /// No tests yet: a checklist with nothing ticked, and the flask beside it, its liquid
    /// the green, a bubble or two rising.
    no_tests,
    |d| {
        d.rotated(-6.0, 60.0, 92.0, |d| {
            d.rect(34.0, 58.0, 50.0, 70.0, 5.0, Fill::Paper);
            for y in [72.0, 90.0, 108.0] {
                d.rect(42.0, y, 9.0, 9.0, 2.0, Fill::None);
                d.line(&format!("M58 {}H{}", y + 4.5, if y == 90.0 { 70.0 } else { 76.0 }));
            }
        });
        d.path("M118 36V70L94 120Q89 134 102 134H158Q171 134 166 120L142 70V36Z", Fill::Paper);
        d.path("M106 102H154L166 120Q171 134 158 134H102Q89 134 94 120Z", Fill::Accent);
        d.line("M110 36H150");
        d.circle(128.0, 88.0, 3.5, Fill::Paper).circle(136.0, 74.0, 2.5, Fill::Paper);
        d.twinkling_spark(196.0, 50.0, 0.75).twinkling_spark(98.0, 24.0, 0.5).twinkling_spark(206.0, 120.0, 0.5);
    }
);

drawing!(
    /// No problems: a page under the lens, nothing found on it, and the green tick.
    no_problems,
    |d| {
        d.rotated(-4.0, 104.0, 78.0, |d| sheet(d, 70.0, 30.0, 68.0, 94.0, Fill::Paper, 5));
        lens(d, 150.0, 98.0, 24.0, Fill::Tint, Fill::Paper);
        tick_badge(d, 176.0, 50.0, 14.0, Fill::Accent);
        d.twinkling_spark(42.0, 52.0, 0.7).twinkling_spark(212.0, 104.0, 0.5).twinkling_spark(52.0, 124.0, 0.5);
    }
);

drawing!(
    /// No processes: a quiet window, its pulse a flat line, asleep, with the green power
    /// button that starts something.
    no_processes,
    |d| {
        d.rect(50.0, 40.0, 122.0, 80.0, 8.0, Fill::Paper);
        d.path("M50 54V48Q50 40 58 40H164Q172 40 172 48V54Z", Fill::Tint);
        d.dot(59.0, 47.0, 1.6).dot(66.0, 47.0, 1.6);
        d.line("M62 92H90L95 86L100 98L105 92H160");
        d.line("M111 120V132").line("M92 132H130");
        d.circle(190.0, 112.0, 13.0, Fill::Accent);
        d.arc(190.0, 113.0, 6.0, -55.0, 235.0);
        d.line("M190 104V111");
        asleep(d, 184.0, 40.0);
        d.twinkling_spark(32.0, 62.0, 0.65).twinkling_spark(120.0, 24.0, 0.45).twinkling_spark(36.0, 128.0, 0.45);
    }
);

drawing!(
    /// No search results: the lens over a page whose lines are only dots, a question in the
    /// glass, its handle the green.
    no_results,
    |d| {
        d.rotated(-4.0, 104.0, 74.0, |d| {
            d.path("M72 30H130L140 40V118H72Z", Fill::Paper);
            d.line("M130 30V40H140");
            for (y, end) in [(50.0, 124.0), (62.0, 116.0), (74.0, 128.0), (86.0, 110.0)] {
                thread(d, &format!("M82 {y}H{end}"));
            }
        });
        lens(d, 152.0, 98.0, 24.0, Fill::Tint, Fill::Accent);
        d.line("M146 92C146 84 158 84 158 92C158 97 152 97 152 103");
        d.dot(152.0, 109.0, 1.9);
        d.twinkling_spark(40.0, 56.0, 0.7).twinkling_spark(206.0, 46.0, 0.55).twinkling_spark(214.0, 128.0, 0.45);
    }
);

drawing!(
    /// Claude has changed nothing yet: the conversation, its spark on the green, and a
    /// dotted page where its first edit will land.
    claude_idle,
    |d| {
        d.rotated(-3.0, 76.0, 70.0, |d| {
            d.path(
                "M44 38H104Q116 38 116 50V76Q116 88 104 88H70L54 102L58 88H44Q32 88 32 76V50Q32 38 44 38Z",
                Fill::Paper,
            );
            d.circle(52.0, 56.0, 9.0, Fill::Accent);
            d.spark(52.0, 56.0, 0.7);
            d.line("M66 56H104").line("M46 70H100");
        });
        thread(d, "M150 50H186L198 62V128H150Z");
        thread(d, "M186 50V62H198");
        thread(d, "M116 92C130 104 136 96 148 92");
        d.twinkling_spark(32.0, 120.0, 0.6).twinkling_spark(134.0, 26.0, 0.5).twinkling_spark(214.0, 36.0, 0.5);
    }
);

drawing!(
    /// Connecting to a remote machine: this computer and the server, a dotted thread between
    /// them with the green on its way along it.
    remote_connecting,
    |d| {
        laptop(d, 26.0, 72.0, 76.0, 50.0);
        d.line("M38 90L46 96L38 102").line("M52 102H68");
        server(d, 158.0, 42.0, 56.0);
        thread(d, "M106 100C130 100 134 86 156 86");
        d.circle(130.0, 94.0, 5.5, Fill::Accent);
        d.rotated(-10.0, 120.0, 40.0, |d| d.placed(104.0, 22.0, 0.3, Drawing::butterfly));
        d.twinkling_spark(40.0, 44.0, 0.7).twinkling_spark(146.0, 128.0, 0.5).twinkling_spark(222.0, 136.0, 0.5);
    }
);

drawing!(
    /// The connection is lost: the server, and the cable's plug, the clay, pulled out of its
    /// socket, with the jolt where it came out.
    remote_lost,
    |d| {
        laptop(d, 26.0, 72.0, 76.0, 50.0);
        d.line("M38 90L46 96L38 102").line("M52 102H68");
        server(d, 166.0, 42.0, 50.0);
        d.rect(154.0, 78.0, 13.0, 22.0, 3.0, Fill::Paper);
        d.dot(160.5, 84.0, 1.7).dot(160.5, 94.0, 1.7);
        d.line("M104 104C118 106 118 122 124 124");
        d.rotated(-24.0, 132.0, 122.0, |d| {
            d.rect(122.0, 114.0, 20.0, 16.0, 4.0, Fill::Clay);
            d.line("M142 118H150").line("M142 126H150");
        });
        jolt(d, 144.0, 84.0);
        d.line("M136 104L144 108L140 114");
        d.twinkling_spark(40.0, 44.0, 0.7).twinkling_spark(122.0, 30.0, 0.5).twinkling_spark(222.0, 136.0, 0.5);
    }
);

drawing!(
    /// The debugger with nothing to do: the bug, asleep on a green leaf.
    debugger_idle,
    |d| {
        d.path("M54 134C78 110 160 110 188 130C160 148 82 150 54 134Z", Fill::Accent);
        d.line("M64 134C104 128 148 128 182 131");
        d.line("M98 84L84 76").line("M96 98L80 98").line("M98 110L86 120");
        d.line("M142 84L156 76").line("M144 98L160 98").line("M142 110L154 120");
        d.line("M114 58C111 50 105 48 100 48").line("M126 58C129 50 135 48 140 48");
        d.circle(120.0, 66.0, 10.0, Fill::Tint);
        d.ellipse(pos2(120.0, 98.0), 24.0, 28.0, Fill::Paper, true);
        d.line("M120 76V124");
        d.dot(110.0, 92.0, 2.6).dot(130.0, 106.0, 2.6).dot(128.0, 88.0, 2.0);
        asleep(d, 162.0, 46.0);
        d.twinkling_spark(42.0, 52.0, 0.7).twinkling_spark(206.0, 108.0, 0.5).twinkling_spark(78.0, 26.0, 0.45);
    }
);

drawing!(
    /// No beans: an empty jar with Spring's green leaf on its label, and one bean left
    /// outside it.
    no_beans,
    |d| {
        d.path("M84 56H156V62Q164 66 164 76V128Q164 140 152 140H88Q76 140 76 128V76Q76 66 84 62Z", Fill::Paper);
        d.rect(80.0, 42.0, 80.0, 16.0, 4.0, Fill::Tint);
        d.rect(92.0, 82.0, 56.0, 36.0, 4.0, Fill::Tint);
        d.path("M106 112C106 98 116 90 134 90C134 104 124 112 106 112Z", Fill::Accent);
        d.line("M108 110L128 94");
        d.ellipse(pos2(192.0, 130.0), 13.0, 8.0, Fill::Tint, true);
        d.line("M182 130C188 125 194 135 202 129");
        d.twinkling_spark(44.0, 56.0, 0.7).twinkling_spark(196.0, 46.0, 0.6).twinkling_spark(52.0, 124.0, 0.45);
    }
);

drawing!(
    /// A Gradle sync that failed: the build's blocks, the top one knocked off in clay, and
    /// the sync arrow that will try again.
    gradle_failed,
    |d| {
        d.rect(66.0, 108.0, 44.0, 30.0, 4.0, Fill::Tint);
        d.rect(114.0, 108.0, 44.0, 30.0, 4.0, Fill::Tint);
        d.rect(90.0, 76.0, 44.0, 30.0, 4.0, Fill::Paper);
        d.rotated(24.0, 172.0, 70.0, |d| {
            d.rect(152.0, 56.0, 40.0, 28.0, 4.0, Fill::Clay);
        });
        jolt(d, 142.0, 56.0);
        d.arc(58.0, 58.0, 16.0, 30.0, 300.0);
        d.line("M61.2 36.6L66 44.1L56.1 45.5");
        d.twinkling_spark(206.0, 120.0, 0.6).twinkling_spark(118.0, 30.0, 0.5).twinkling_spark(36.0, 112.0, 0.5);
    }
);

drawing!(
    /// No notifications: a quiet bell, and the green tick beside it for all clear.
    quiet_bell,
    |d| {
        d.dot(118.0, 32.0, 4.0);
        d.path("M118 36C98 36 88 52 88 72V96L76 110H160L148 96V72C148 52 138 36 118 36Z", Fill::Paper);
        d.line("M100 74C100 62 106 54 114 52");
        d.circle(118.0, 118.0, 8.0, Fill::Tint);
        tick_badge(d, 164.0, 52.0, 13.0, Fill::Accent);
        d.twinkling_spark(60.0, 50.0, 0.8).twinkling_spark(192.0, 100.0, 0.6).twinkling_spark(44.0, 112.0, 0.5);
    }
);

drawing!(
    /// Settings the same in every project: a panel of sliders, its knobs set (one the green),
    /// handing the same values along dotted threads to two project folders.
    shared_settings,
    |d| {
        d.rect(30.0, 30.0, 92.0, 100.0, 8.0, Fill::Paper);
        d.line("M44 56H108").line("M44 80H108").line("M44 104H108");
        d.circle(66.0, 56.0, 7.0, Fill::Tint);
        d.circle(94.0, 80.0, 7.0, Fill::Accent);
        d.circle(56.0, 104.0, 7.0, Fill::Tint);
        thread(d, "M126 64C146 64 146 48 162 48");
        thread(d, "M126 96C146 96 146 112 162 112");
        for (x, y) in [(162.0, 32.0), (162.0, 96.0)] {
            d.path(
                &format!(
                    "M{x} {}Q{x} {y} {} {y}H{}L{} {}H{}Q{} {} {} {}V{}H{x}Z",
                    y + 4.0,
                    x + 4.0,
                    x + 16.0,
                    x + 21.0,
                    y + 5.0,
                    x + 44.0,
                    x + 48.0,
                    y + 5.0,
                    x + 48.0,
                    y + 9.0,
                    y + 34.0
                ),
                Fill::Tint,
            );
            d.line(&format!("M{} {}H{}", x + 10.0, y + 21.0, x + 38.0));
            d.circle(x + 30.0, y + 21.0, 4.0, Fill::Paper);
        }
        d.twinkling_spark(206.0, 82.0, 0.6).twinkling_spark(140.0, 24.0, 0.5).twinkling_spark(24.0, 140.0, 0.55);
    }
);

// ----- Settings and Project Structure pages -----------------------------------------------------

drawing!(
    /// Appearance: the painter's palette, its paints (one the green), the brush leaning on
    /// it, and the moon of dark mode over it.
    appearance,
    |d| {
        d.path(
            "M56 96C48 70 70 44 106 40C144 36 176 52 178 80C180 100 162 106 148 100C136 95 128 104 134 116C140 128 124 140 102 136C78 132 62 116 56 96Z",
            Fill::Paper,
        );
        d.circle(158.0, 78.0, 6.0, Fill::Blob);
        d.circle(84.0, 70.0, 9.0, Fill::Tint).circle(110.0, 58.0, 9.0, Fill::Accent);
        d.circle(138.0, 60.0, 8.0, Fill::Tint).circle(80.0, 100.0, 8.0, Fill::Tint);
        d.rotated(38.0, 178.0, 112.0, |d| {
            d.rect(173.0, 96.0, 10.0, 46.0, 4.0, Fill::Tint);
            d.rect(172.0, 86.0, 12.0, 12.0, 2.0, Fill::Paper);
            d.path("M172 86C172 76 176 70 178 64C180 70 184 76 184 86Z", Fill::Paper);
        });
        d.path("M208 22C196 24 190 36 194 46C198 56 210 60 218 54C206 54 198 44 202 34C203 30 205 26 208 22Z", Fill::Tint);
        d.twinkling_spark(36.0, 50.0, 0.7).twinkling_spark(196.0, 76.0, 0.5).twinkling_spark(40.0, 130.0, 0.5);
    }
);

drawing!(
    /// Keymap: the keyboard, and the one key lifted out of it to take new keys, the green.
    keymap,
    |d| {
        d.rect(32.0, 78.0, 168.0, 58.0, 8.0, Fill::Paper);
        for (row, y) in [88.0, 104.0].into_iter().enumerate() {
            for k in 0..7 {
                let x = 42.0 + row as f32 * 6.0 + k as f32 * 21.0;
                if !(row == 0 && k == 4) {
                    d.rect(x, y, 16.0, 11.0, 3.0, Fill::Tint);
                }
            }
        }
        d.rect(78.0, 120.0, 76.0, 9.0, 3.0, Fill::Tint);
        thread(d, "M134 86C134 72 140 64 146 58");
        d.rotated(-12.0, 160.0, 40.0, |d| {
            d.rect(142.0, 20.0, 36.0, 34.0, 7.0, Fill::Accent);
            d.rect(148.0, 25.0, 24.0, 20.0, 4.0, Fill::None);
            d.circle(156.0, 31.0, 2.5, Fill::None).circle(164.0, 31.0, 2.5, Fill::None);
            d.circle(156.0, 39.0, 2.5, Fill::None).circle(164.0, 39.0, 2.5, Fill::None);
        });
        d.twinkling_spark(44.0, 46.0, 0.7).twinkling_spark(212.0, 70.0, 0.55).twinkling_spark(104.0, 34.0, 0.45);
    }
);

drawing!(
    /// Editor: a page of code in its window, indented, the line with the caret lit green,
    /// and the tab's arrow at the margin.
    editor,
    |d| {
        d.rotated(-2.0, 120.0, 84.0, |d| {
            d.rect(46.0, 30.0, 148.0, 104.0, 8.0, Fill::Paper);
            d.path("M46 44V38Q46 30 54 30H186Q194 30 194 38V44Z", Fill::Tint);
            d.dot(56.0, 37.0, 1.6).dot(63.0, 37.0, 1.6).dot(70.0, 37.0, 1.6);
            d.shape("M50 84H190V96H50Z", Fill::Accent);
            d.line("M60 58H112").line("M76 70H150").line("M92 90H138").line("M76 110H124").line("M60 122H90");
            d.line("M141 84V97");
            d.line("M62 90H80").line("M75 86L80 90L75 94");
        });
        d.line("M214 62L224 72L214 82").line("M30 92L20 102L30 112");
        d.twinkling_spark(34.0, 40.0, 0.7).twinkling_spark(214.0, 122.0, 0.55).twinkling_spark(206.0, 24.0, 0.45);
    }
);

drawing!(
    /// Version Control: the remote's cloud, fetched on the clock, the new commit (the green)
    /// coming down a dotted thread to the history.
    version_control,
    |d| {
        cloud(d, 6.0, -8.0, 0.9, Fill::Paper);
        d.line("M28 124H200");
        d.line("M76 124C96 124 96 96 116 96H150");
        d.circle(44.0, 124.0, 5.0, Fill::Paper).circle(76.0, 124.0, 5.0, Fill::Paper);
        d.circle(116.0, 96.0, 5.0, Fill::Paper).circle(140.0, 124.0, 5.0, Fill::Paper);
        d.circle(180.0, 124.0, 5.0, Fill::Paper);
        thread(d, "M176 64C176 76 168 86 158 92");
        d.circle(154.0, 96.0, 7.0, Fill::Accent);
        d.circle(58.0, 58.0, 18.0, Fill::Paper);
        d.line("M58 46V58L66 64");
        d.twinkling_spark(98.0, 30.0, 0.6).twinkling_spark(218.0, 96.0, 0.5).twinkling_spark(28.0, 92.0, 0.5);
    }
);

drawing!(
    /// Gradle: the build's blocks stacked, the top one set in green, and the gear that runs
    /// the daemons beside them.
    gradle,
    |d| {
        d.rect(54.0, 108.0, 44.0, 30.0, 4.0, Fill::Tint);
        d.rect(102.0, 108.0, 44.0, 30.0, 4.0, Fill::Tint);
        d.rect(78.0, 76.0, 44.0, 30.0, 4.0, Fill::Paper);
        d.rect(78.0, 44.0, 44.0, 30.0, 4.0, Fill::Accent);
        d.line("M90 59L97 66L110 52");
        d.gear(176.0, 98.0, 22.0, 9, Fill::Paper);
        d.gear(196.0, 56.0, 12.0, 7, Fill::Tint);
        d.twinkling_spark(40.0, 54.0, 0.7).twinkling_spark(144.0, 30.0, 0.5).twinkling_spark(218.0, 128.0, 0.45);
    }
);

drawing!(
    /// Spring: a seedling in its pot, its new leaf the green, and the profile tags tied to
    /// it.
    spring,
    |d| {
        d.path("M86 104H154L146 140H94Z", Fill::Tint);
        d.rect(80.0, 96.0, 80.0, 12.0, 4.0, Fill::Paper);
        d.line("M120 96C120 78 116 66 120 50");
        d.path("M120 72C104 74 90 66 86 52C102 50 116 56 120 72Z", Fill::Paper);
        d.line("M118 70L96 56");
        d.path("M120 54C124 36 140 26 160 28C158 46 142 56 120 54Z", Fill::Accent);
        d.line("M122 52L150 34");
        thread(d, "M160 100C172 100 176 92 182 86");
        d.rotated(12.0, 196.0, 84.0, |d| {
            d.path("M180 76H206Q212 76 212 82V90Q212 96 206 96H180L172 86Z", Fill::Paper);
            d.circle(181.0, 86.0, 2.5, Fill::None);
            d.line("M188 86H204");
        });
        d.twinkling_spark(46.0, 56.0, 0.7).twinkling_spark(198.0, 130.0, 0.5).twinkling_spark(184.0, 30.0, 0.45);
    }
);

drawing!(
    /// Terminal: the terminal's window with its prompt and output, the cursor the green.
    terminal,
    |d| {
        d.rotated(-3.0, 120.0, 84.0, |d| {
            d.rect(44.0, 32.0, 152.0, 100.0, 8.0, Fill::Paper);
            d.path("M44 46V40Q44 32 52 32H188Q196 32 196 40V46Z", Fill::Tint);
            d.dot(54.0, 39.0, 1.6).dot(61.0, 39.0, 1.6).dot(68.0, 39.0, 1.6);
            d.line("M58 60L66 66L58 72").line("M74 72H124");
            d.line("M58 86H150").line("M58 98H118");
            d.line("M58 112L66 118L58 124");
            d.rect(74.0, 110.0, 10.0, 16.0, 2.0, Fill::Accent);
        });
        d.rotated(14.0, 204.0, 36.0, |d| d.placed(184.0, 18.0, 0.32, Drawing::butterfly));
        d.twinkling_spark(32.0, 48.0, 0.7).twinkling_spark(214.0, 112.0, 0.55).twinkling_spark(120.0, 18.0, 0.45);
    }
);

drawing!(
    /// Modules: the build's projects as parcels, stacked and side by side, the one in view
    /// the green.
    modules,
    |d| {
        for (x, y, fill) in [(52.0, 98.0, Fill::Tint), (104.0, 98.0, Fill::Paper), (156.0, 98.0, Fill::Tint)] {
            d.rect(x, y, 44.0, 38.0, 4.0, fill);
            d.line(&format!("M{} {y}V{}", x + 22.0, y + 12.0));
        }
        d.rect(78.0, 56.0, 44.0, 38.0, 4.0, Fill::Paper);
        d.line("M100 56V68");
        d.rect(130.0, 56.0, 44.0, 38.0, 4.0, Fill::Accent);
        d.line("M152 56V68");
        thread(d, "M100 30C100 40 112 44 126 44");
        d.twinkling_spark(42.0, 56.0, 0.7).twinkling_spark(206.0, 46.0, 0.55).twinkling_spark(80.0, 26.0, 0.45);
    }
);

drawing!(
    /// Dependencies: a parcel and what it brings in, hanging from it, the one Gradle chose
    /// the green.
    dependencies,
    |d| {
        d.rect(96.0, 26.0, 48.0, 36.0, 4.0, Fill::Paper);
        d.line("M120 26V38");
        d.line("M120 62V78").line("M64 78H176").line("M64 78V92").line("M120 78V92").line("M176 78V92");
        d.rect(44.0, 92.0, 40.0, 32.0, 4.0, Fill::Tint);
        d.rect(100.0, 92.0, 40.0, 32.0, 4.0, Fill::Accent);
        d.rect(156.0, 92.0, 40.0, 32.0, 4.0, Fill::Tint);
        d.line("M110 108L117 115L130 101");
        d.twinkling_spark(46.0, 50.0, 0.7).twinkling_spark(196.0, 46.0, 0.55).twinkling_spark(214.0, 134.0, 0.45);
    }
);

drawing!(
    /// Facets: the frameworks and tools a project has, as cards fanned out: Spring's leaf,
    /// the green, on the one in front, Claude's spark on the one behind.
    facets,
    |d| {
        d.rotated(-10.0, 96.0, 88.0, |d| {
            d.rect(64.0, 44.0, 64.0, 84.0, 6.0, Fill::Tint);
            d.spark(96.0, 70.0, 1.3);
            d.line("M78 102H114").line("M78 112H104");
        });
        d.rotated(8.0, 148.0, 88.0, |d| {
            d.rect(116.0, 40.0, 64.0, 88.0, 6.0, Fill::Paper);
            d.path("M132 82C134 64 148 56 166 58C164 76 150 84 132 82Z", Fill::Accent);
            d.line("M134 80L158 64");
            d.line("M130 102H166").line("M130 112H156");
        });
        d.twinkling_spark(40.0, 52.0, 0.7).twinkling_spark(206.0, 40.0, 0.55).twinkling_spark(210.0, 126.0, 0.45);
    }
);

/// Every spot drawing of this module, by name: for tests and the gallery.
pub fn all() -> [(&'static str, &'static Drawing); 36] {
    [
        ("discard", discard()),
        ("push_rejected", push_rejected()),
        ("merge_conflict", merge_conflict()),
        ("clone_repo", clone_repo()),
        ("new_branch", new_branch()),
        ("stash", stash()),
        ("ssh", ssh()),
        ("jdk", jdk()),
        ("claude_review", claude_review()),
        ("warning", warning()),
        ("info", info()),
        ("success", success()),
        ("no_project", no_project()),
        ("nothing_staged", nothing_staged()),
        ("no_tests", no_tests()),
        ("no_problems", no_problems()),
        ("no_processes", no_processes()),
        ("no_results", no_results()),
        ("claude_idle", claude_idle()),
        ("remote_connecting", remote_connecting()),
        ("remote_lost", remote_lost()),
        ("debugger_idle", debugger_idle()),
        ("no_beans", no_beans()),
        ("gradle_failed", gradle_failed()),
        ("quiet_bell", quiet_bell()),
        ("shared_settings", shared_settings()),
        ("appearance", appearance()),
        ("keymap", keymap()),
        ("editor", editor()),
        ("version_control", version_control()),
        ("gradle", gradle()),
        ("spring", spring()),
        ("terminal", terminal()),
        ("modules", modules()),
        ("dependencies", dependencies()),
        ("facets", facets()),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_spot_builds_with_fills_and_sparks() {
        for (name, d) in super::all() {
            assert!(d.parts.iter().any(|p| !p.tris.is_empty()), "{name} has no fill");
            assert_eq!(d.sparks.len(), 3, "{name} twinkles three sparks");
            assert_eq!((d.size.x, d.size.y), (240.0, 160.0), "{name} is a spot box");
        }
    }
}

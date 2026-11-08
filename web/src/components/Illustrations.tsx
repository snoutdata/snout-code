// Flat vector art in the house style of the Snout sites: a round ink line over a few flat
// fills, drawn from the --ill-* tokens (styles.css) so every drawing follows the theme. One
// green (--ill-accent) on the thing a picture is about; the warm tones (ochre, clay, sand) only
// where warmth is the point. Every drawing is decorative and aria-hidden. See web/README.md.

import type { ReactNode } from 'react'

// ---------------------------------------------------------------------------------------------
// Shared parts
// ---------------------------------------------------------------------------------------------

// The four-point sparkle, the house ornament.
export function Spark({ x, y, s = 1 }: { x: number, y: number, s?: number }) {
	return (
		<path
			className="ill-spark"
			d={`M${x} ${y - 7 * s} Q${x + 1.4 * s} ${y - 1.4 * s} ${x + 7 * s} ${y} Q${x + 1.4 * s} ${y + 1.4 * s} ${x} ${y + 7 * s} Q${x - 1.4 * s} ${y + 1.4 * s} ${x - 7 * s} ${y} Q${x - 1.4 * s} ${y - 1.4 * s} ${x} ${y - 7 * s} Z`}
		/>
	)
}

// A curly brace, the coder's ornament: `{` at (x, y) top, h tall; `flip` draws `}`.
function Brace({ x, y, h = 30, flip = false, className = '' }: { x: number, y: number, h?: number, flip?: boolean, className?: string }) {
	const k = flip ? -1 : 1
	const m = h / 2
	const w = h / 6
	return (
		<path
			className={className}
			d={`M${x + 2 * w * k} ${y}C${x + w * k} ${y} ${x + w * k} ${y + 2} ${x + w * k} ${y + w * 1.5}V${y + m - w}C${x + w * k} ${y + m - 2} ${x} ${y + m} ${x - w * 0.4 * k} ${y + m}C${x} ${y + m} ${x + w * k} ${y + m + 2} ${x + w * k} ${y + m + w}V${y + h - w * 1.5}C${x + w * k} ${y + h - 2} ${x + w * k} ${y + h} ${x + 2 * w * k} ${y + h}`}
		/>
	)
}

// Lines of code: short strokes at an indent, the way a page of code reads from afar.
function CodeLines({ x, y, lines, gap = 10, accent = -1, warm = -1 }: { x: number, y: number, lines: [number, number][], gap?: number, accent?: number, warm?: number }) {
	return (
		<g>
			{lines.map(([indent, len], i) => (
				<path
					key={i}
					className={i === accent ? 'ill-code-accent' : i === warm ? 'ill-code-warm' : 'ill-code'}
					d={`M${x + indent} ${y + i * gap}H${x + indent + len}`}
				/>
			))}
		</g>
	)
}

// An app window: a body, a title bar with three dots.
function Win({ x, y, w, h, bar = 'ill-tint', fill = 'ill-paper' }: { x: number, y: number, w: number, h: number, bar?: string, fill?: string }) {
	return (
		<g>
			<rect className={fill} x={x} y={y} width={w} height={h} rx="8" />
			<path className={bar} d={`M${x + 8} ${y}H${x + w - 8}A8 8 0 0 1 ${x + w} ${y + 8}V${y + 14}H${x}V${y + 8}A8 8 0 0 1 ${x + 8} ${y}Z`} />
			<circle className="ill-ink-dot" cx={x + 9} cy={y + 7} r="1.8" />
			<circle className="ill-ink-dot" cx={x + 16} cy={y + 7} r="1.8" />
			<circle className="ill-ink-dot" cx={x + 23} cy={y + 7} r="1.8" />
		</g>
	)
}

function Laptop({ x, y, w, h }: { x: number, y: number, w: number, h: number }) {
	return (
		<g>
			<rect className="ill-paper" x={x} y={y} width={w} height={h} rx="7" />
			<path className="ill-tint" d={`M${x - 14} ${y + h}H${x + w + 14}L${x + w + 4} ${y + h + 12}H${x - 4}Z`} />
		</g>
	)
}

function SkyCloud({ x, y, s = 1, className = '' }: { x: number, y: number, s?: number, className?: string }) {
	return (
		<g className={className}>
			<path
				className="ill-paper"
				transform={`translate(${x} ${y}) scale(${s})`}
				d="M8 30C-2 30 -4 16 6 14C8 4 22 0 30 8C36 -2 54 0 56 14C66 14 68 30 58 30Z"
			/>
		</g>
	)
}

function Pine({ x, y, s = 1 }: { x: number, y: number, s?: number }) {
	return (
		<g transform={`translate(${x} ${y}) scale(${s})`}>
			<path d="M0 0V-12" />
			<path className="ill-paper" d="M0 -78L18 -40H9L22 -12H-22L-9 -40H-18Z" />
		</g>
	)
}

function RoundTree({ x, y, s = 1 }: { x: number, y: number, s?: number }) {
	return (
		<g transform={`translate(${x} ${y}) scale(${s})`}>
			<path d="M0 0V-34" />
			<path className="ill-tint" d="M0 -30C-22 -30 -26 -52 -14 -62C-12 -80 12 -80 14 -62C26 -52 22 -30 0 -30Z" />
			<path d="M0 -44L-8 -52M0 -40L7 -47" />
		</g>
	)
}

// A tree whose branches are a Git history: the trunk is main, each bough a branch, and the
// fruit its commits, the newest one green.
function GitTree({ x, y, s = 1 }: { x: number, y: number, s?: number }) {
	return (
		<g transform={`translate(${x} ${y}) scale(${s})`}>
			<path className="ill-tint" d="M-58 -78C-74 -112 -40 -150 -2 -146C40 -150 76 -118 62 -80C56 -60 30 -52 0 -54C-30 -52 -52 -62 -58 -78Z" />
			<path d="M0 0V-96" />
			<path d="M0 -40C-20 -46 -30 -60 -32 -80" />
			<path d="M0 -60C18 -66 28 -82 30 -104" />
			<path d="M0 -86C-8 -96 -14 -108 -14 -122" />
			<circle className="ill-paper" cx="-32" cy="-84" r="6" />
			<circle className="ill-paper" cx="30" cy="-108" r="6" />
			<circle className="ill-paper" cx="-14" cy="-126" r="6" />
			<circle className="ill-accent" cx="0" cy="-100" r="7" />
			<path d="M-14 0C-8 -4 8 -4 14 0" />
		</g>
	)
}

// A small friendly robot: the coding agent. (x, y) is the middle of its feet.
function Bot({ x, y, fill = 'ill-paper' }: { x: number, y: number, fill?: string }) {
	return (
		<g transform={`translate(${x} ${y})`}>
			<path d="M0 -64V-74" />
			<circle className="ill-accent" cx="0" cy="-77" r="4" />
			<rect className={fill} x="-20" y="-64" width="40" height="28" rx="9" />
			<circle className="ill-ink-dot" cx="-8" cy="-50" r="3" />
			<circle className="ill-ink-dot" cx="8" cy="-50" r="3" />
			<path d="M-5 -42C-2 -40 2 -40 5 -42" />
			<rect className="ill-tint" x="-15" y="-32" width="30" height="26" rx="7" />
			<path d="M-15 -24L-26 -14M15 -24L26 -16" />
			<path d="M-7 -6V0M7 -6V0" />
		</g>
	)
}

// A crate: a module of a build.
function Crate({ x, y, w = 56, h = 34, fill = 'ill-paper' }: { x: number, y: number, w?: number, h?: number, fill?: string }) {
	return (
		<g>
			<rect className={fill} x={x} y={y} width={w} height={h} rx="5" />
			<path d={`M${x} ${y + 10}H${x + w}`} />
			<path className="ill-code" d={`M${x + 10} ${y + 21}H${x + w * 0.55}`} />
		</g>
	)
}

// ---------------------------------------------------------------------------------------------
// The butterfly
// ---------------------------------------------------------------------------------------------

// One wing pair of the SnoutCode butterfly, drawn on the left of the body; the right side is
// the same drawing mirrored. It is the app icon's (assets/icon/snout.svg): the veins are
// branches, each ending in a commit.
function Wings() {
	return (
		<g>
			<path className="ill-tint" d="M60 44C50 28 30 12 16 14C6 16 8 32 16 42C22 50 36 54 60 53Z" />
			<path className="ill-accent" d="M60 58C44 58 26 62 24 74C22 86 34 92 44 86C52 81 58 74 60 68Z" />
			<path d="M60 50C50 50 46 36 33 36" />
			<path d="M60 66C52 66 50 74 42 74" />
			<circle className="ill-paper" cx="29.5" cy="36" r="3.6" />
			<circle className="ill-paper" cx="38.5" cy="74" r="3.6" />
		</g>
	)
}

// The body is a commit chain, the top commit green. `glasses` puts the coder's round
// spectacles on it, for the drawings big enough to show them.
function FlyBody({ glasses = false }: { glasses?: boolean }) {
	return (
		<>
			<g className="ill-fly-wing"><Wings /></g>
			<g className="ill-fly-wing"><g transform="matrix(-1 0 0 1 120 0)"><Wings /></g></g>
			<path d="M58 30C54 22 50 16 44 12M62 30C66 22 70 16 76 12" />
			<circle className="ill-ink-dot" cx="43" cy="11" r="2.6" />
			<circle className="ill-ink-dot" cx="77" cy="11" r="2.6" />
			<path d="M60 34V90" />
			<circle className="ill-accent" cx="60" cy="34" r="4.6" />
			<circle className="ill-paper" cx="60" cy="50" r="4.2" />
			<circle className="ill-paper" cx="60" cy="66" r="4.2" />
			<circle className="ill-paper" cx="60" cy="82" r="4.2" />
			{glasses && (
				<g className="ill-glasses">
					<circle className="ill-glass" cx="52" cy="26" r="6" />
					<circle className="ill-glass" cx="68" cy="26" r="6" />
					<path d="M58 25C59 23.6 61 23.6 62 25" />
				</g>
			)}
		</>
	)
}

// The art butterfly on its own. The app icon stays the logo (Brand.tsx); this is for the places
// that want a drawing: the hero, a page heading, an empty state.
export function Butterfly({ className = '', glasses = false }: { className?: string, glasses?: boolean }) {
	return (
		<svg className={`ill ill-fly ${className}`} viewBox="0 0 120 100" aria-hidden="true" focusable="false">
			<FlyBody glasses={glasses} />
		</svg>
	)
}

// The butterfly nested in a bigger drawing, top left at (x, y). A nested <svg>, never a
// <g transform>, because the flap scales the wings around the butterfly's own middle.
// Its line is thickened as it shrinks, so it matches the drawing around it.
function Fly({ x, y, w = 60, tilt = -10, glasses = false }: { x: number, y: number, w?: number, tilt?: number, glasses?: boolean }) {
	return (
		<svg className="ill-fly" style={{ strokeWidth: flyStroke(w) }} x={x} y={y} width={w} height={w * 0.83} viewBox="0 0 120 100" overflow="visible">
			<g transform={`rotate(${tilt} 60 50)`}><FlyBody glasses={glasses} /></g>
		</svg>
	)
}

function flyStroke(w: number): number {
	return Math.max(3, Math.round((2.6 * 120 / w) * 10) / 10)
}

// The drawn stroke under a lockup's key word; it draws itself in once on load.
function Swash() {
	return (
		<svg className="mark-swash" viewBox="0 0 100 14" preserveAspectRatio="none" aria-hidden="true" focusable="false">
			<path d="M3 9C24 4 52 3 97 6" pathLength={1} />
		</svg>
	)
}

// The coder's mark after a word: a text caret, blinking.
function Caret() {
	return <span className="mark-caret" aria-hidden="true" />
}

// The hero's name as a drawing: the butterfly beside the wordmark (real text, so the heading
// still reads "SnoutCode" to a crawler), a drawn stroke under "Code" and a caret after it.
export function HeroMark() {
	return (
		<span className="hero-mark">
			<Butterfly className="hero-mark-fly" glasses />
			<span className="hero-mark-word">
				Snout<span className="mark-key">Code<Swash /></span><Caret />
			</span>
		</span>
	)
}

// A page heading as a drawing: a small scene beside the heading's own text, the stroke under
// its last word. Goes INSIDE the <h1>, so the heading text is unchanged for crawlers.
export function PageMark({ art, text }: { art: keyof typeof MARK_ART, text: string }) {
	const Art = MARK_ART[art]
	const cut = text.lastIndexOf(' ')
	const lead = cut < 0 ? '' : text.slice(0, cut + 1)
	const key = cut < 0 ? text : text.slice(cut + 1)
	return (
		<span className="page-mark">
			<svg className="ill ill-mark" viewBox="0 0 120 100" aria-hidden="true" focusable="false"><Art /></svg>
			<span className="page-mark-word">
				{lead}<span className="mark-key">{key}<Swash /></span>
			</span>
		</span>
	)
}

const MARK_BLOB = 'M10 60C6 32 36 14 66 16C96 18 116 36 112 62C108 86 80 94 56 92C30 90 13 82 10 60Z'

const MARK_ART = {
	// An open box, a green arrow going in.
	download: () => (
		<>
			<path className="ill-blob" d={MARK_BLOB} />
			<path className="ill-sand" d="M28 54H92V86H28Z" />
			<path className="ill-paper" d="M28 54L18 42H52L60 54ZM92 54L102 42H68L60 54Z" />
			<path className="ill-accent" d="M54 14H66V32H76L60 48L44 32H54Z" />
			<path d="M40 70H60" />
		</>
	),
	// The butterfly, lost, beside a signpost.
	lost: () => (
		<>
			<path className="ill-blob" d={MARK_BLOB} />
			<path d="M40 90V30" />
			<path className="ill-paper" d="M22 32H58L66 40L58 48H22Z" />
			<path className="ill-tint" d="M58 54H22L14 62L22 70H58Z" />
			<Fly x={62} y={14} w={50} tilt={12} />
		</>
	)
}

// ---------------------------------------------------------------------------------------------
// The hero's drawing: the butterfly perched on the top edge of the app window
// ---------------------------------------------------------------------------------------------

export function PerchArt() {
	return (
		<svg className="ill ill-perch" viewBox="0 0 240 150" aria-hidden="true" focusable="false">
			<path className="ill-thread" d="M8 128C34 76 70 132 100 96C120 72 128 70 142 74" />
			<Brace x={22} y={44} h={34} className="ill-float" />
			<Brace x={58} y={44} h={34} flip className="ill-float ill-float-late" />
			<Fly x={128} y={18} w={104} tilt={-6} glasses />
			<Spark x={112} y={30} s={0.9} />
			<Spark x={226} y={92} s={0.6} />
		</svg>
	)
}

// ---------------------------------------------------------------------------------------------
// Section drawings (0 0 360 140, on the wide blob), one per homepage section
// ---------------------------------------------------------------------------------------------

const WIDE_BLOB = 'M22 84C18 46 64 22 120 26C160 29 190 40 226 34C282 25 342 44 338 88C334 124 280 128 232 120C196 114 170 122 126 124C70 127 26 116 22 84Z'

const SECTION_ART = {
	// A steaming mug of Java, and an editor offering a completion.
	java: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<path className="ill-steam" d="M64 40C58 30 70 26 64 14M82 40C76 30 88 26 82 14" />
			<path className="ill-clay" d="M46 48H100V102C100 112 92 118 82 118H64C54 118 46 112 46 102Z" />
			<path d="M100 60C116 60 116 88 100 88" />
			<Brace x={60} y={64} h={30} className="ill-on-clay" />
			<Brace x={86} y={64} h={30} flip className="ill-on-clay" />
			<path className="ill-ground" d="M36 124H112" />
			<Win x={136} y={18} w={186} h={104} />
			<CodeLines x={150} y={44} lines={[[0, 54], [10, 80], [10, 40], [20, 64], [0, 12]]} gap={13} />
			<rect className="ill-paper" x={214} y={66} width={96} height={46} rx="6" />
			<path className="ill-accent-soft" d="M214 80H310V95H214Z" />
			<path className="ill-code" d="M224 74H272M224 88H286M224 104H262" />
			<path d="M206 66V80" className="ill-caret-line" />
			<Spark x={330} y={22} s={0.7} />
			<Spark x={28} y={46} s={0.55} />
		</>
	),
	// A build's modules as crates on threads, one of them green.
	build: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<Crate x={150} y={14} w={64} h={36} fill="ill-tint" />
			<path className="ill-thread" d="M182 50V66M182 66H86V80M182 66V80M182 66H278V80" />
			<Crate x={58} y={80} />
			<Crate x={154} y={80} fill="ill-accent" />
			<Crate x={250} y={80} />
			<path className="ill-thread" d="M278 114V124H310" />
			<rect className="ill-sand" x={310} y={110} width={30} height={22} rx="4" />
			<g className="ill-spin-slow">
				<circle className="ill-paper" cx="40" cy="44" r="12" />
				<circle cx="40" cy="44" r="4" />
				<path d="M40 26V30M40 58V62M22 44H26M54 44H58M27 31L30 34M50 54L53 57M27 57L30 54M50 34L53 31" />
			</g>
			<Spark x={328} y={34} s={0.7} />
		</>
	),
	// Spring: a sprout out of a pot of beans, threads from the beans to where they go.
	spring: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<path d="M180 92V40" />
			<path className="ill-accent" d="M180 62C160 64 148 52 150 36C166 34 180 44 180 62Z" />
			<path className="ill-accent" d="M180 50C196 52 210 40 208 22C192 22 180 32 180 50Z" />
			<path className="ill-clay" d="M150 90H210L204 124H156Z" />
			<path d="M146 90H214" />
			{[[74, 52, -20], [92, 92, 15], [272, 50, 25], [288, 96, -10]].map(([cx, cy, r], i) => (
				<g key={i} transform={`rotate(${r} ${cx} ${cy})`}>
					<ellipse className="ill-warm" cx={cx} cy={cy} rx="13" ry="17" />
					<path d={`M${cx} ${cy - 13}C${cx - 5} ${cy - 4} ${cx + 5} ${cy + 4} ${cx} ${cy + 13}`} />
				</g>
			))}
			<path className="ill-thread" d="M88 58C116 66 126 86 148 96M106 96C122 102 132 104 152 106M258 56C232 62 224 80 210 96M274 98C250 104 232 106 208 106" />
			<Spark x={36} y={36} s={0.6} />
			<Spark x={324} y={30} s={0.7} />
		</>
	),
	// The agent at work: a robot at a laptop, its line in clay with its caret, the butterfly
	// watching from the lid.
	agents: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<Bot x={74} y={124} />
			<path className="ill-thread" d="M100 104C120 100 132 98 148 94" />
			<Laptop x={150} y={24} w={150} h={86} />
			<CodeLines x={166} y={42} lines={[[0, 60], [10, 90], [10, 70], [20, 50], [10, 76]]} gap={13} warm={2} />
			<path className="ill-caret-clay" d="M248 60V74" />
			<rect className="ill-clay" x={240} y={46} width={30} height={10} rx="3" />
			<Fly x={262} y={-10} w={54} tilt={10} />
			<Spark x={30} y={50} s={0.6} />
			<Spark x={330} y={100} s={0.65} />
		</>
	),
	// The agent drives the window: the robot's wire runs into the app, where a run plays, the
	// tests come back two green and one to look at, and a small card waits for your answer
	// before anything changes.
	harness: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<Bot x={62} y={126} />
			<path className="ill-wire" d="M88 110C118 112 120 74 150 74" />
			<path className="ill-wire-flow" d="M88 110C118 112 120 74 150 74" />
			<Win x={150} y={22} w={176} h={104} />
			<rect className="ill-tint" x={158} y={44} width={34} height={74} rx="4" />
			<path className="ill-code" d="M165 56H184M165 68H180M165 80H186" />
			<CodeLines x={204} y={48} lines={[[0, 58], [10, 80], [10, 64]]} gap={12} accent={1} />
			<circle className="ill-accent" cx="214" cy="104" r="9" />
			<path className="ill-on-accent" d="M211 99V109L219 104Z" />
			<circle className="ill-accent" cx="244" cy="104" r="7" />
			<path d="M241 104L243.5 106.5L248 101.5" />
			<circle className="ill-accent" cx="266" cy="104" r="7" />
			<path d="M263 104L265.5 106.5L270 101.5" />
			<circle className="ill-clay" cx="288" cy="104" r="7" />
			<path d="M285.5 101.5L290.5 106.5M290.5 101.5L285.5 106.5" />
			<g className="ill-float">
				<rect className="ill-paper" x={262} y={2} width={86} height={40} rx="7" />
				<path className="ill-code" d="M272 14H322" />
				<rect className="ill-accent" x={272} y={24} width={30} height={10} rx="4" />
				<rect className="ill-tint" x={308} y={24} width={30} height={10} rx="4" />
			</g>
			<Spark x={28} y={44} s={0.6} />
			<Spark x={136} y={12} s={0.55} />
		</>
	),
	// Playwright: a browser being recorded (the red dot), a pointer clicking its button, and the
	// steps flowing into the spec beside it, the newest line in green.
	playwright: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<Win x={40} y={22} w={150} h={100} />
			<circle className="ill-clay ill-blink" cx="178" cy="29" r="4" />
			<rect className="ill-tint" x={56} y={50} width={84} height={10} rx="3" />
			<rect className="ill-paper" x={56} y={68} width={118} height={12} rx="4" />
			<rect className="ill-accent" x={56} y={92} width={58} height={18} rx="9" />
			<path className="ill-paper" d="M104 100L104 124L110 118L115 129L120 127L115 116L123 116Z" />
			<path className="ill-thread" d="M194 72C210 72 212 60 226 60" />
			<rect className="ill-paper" x={228} y={30} width={104} height={92} rx="7" />
			<CodeLines x={240} y={46} lines={[[0, 54], [8, 70], [8, 62], [8, 48], [0, 10]]} gap={14} accent={3} />
			<Spark x={30} y={30} s={0.6} />
			<Spark x={340} y={22} s={0.7} />
		</>
	),
	// Node: a package with its Node version on a tag, a script running in the terminal, and the
	// tests ticked green beside it.
	node: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<path className="ill-sand" d="M40 58L84 42L128 58V102L84 118L40 102Z" />
			<path d="M40 58L84 74L128 58M84 74V118" />
			<path className="ill-paper" d="M54 66L74 73V92L54 85Z" />
			<path d="M112 44V30" />
			<rect className="ill-accent" x={98} y={14} width={34} height={18} rx="5" />
			<text className="ill-label" x="115" y="27" textAnchor="middle">22</text>
			<Win x={146} y={30} w={116} h={78} fill="ill-ink-panel" />
			<path className="ill-on-ink" d="M158 60L165 65L158 70" />
			<path className="ill-on-ink" d="M171 70H226" />
			<path className="ill-on-ink ill-faint" d="M158 88H240" />
			<rect className="ill-accent ill-blink" x={230} y={62} width={8} height={11} rx="1.5" />
			<circle className="ill-accent" cx="296" cy="52" r="10" />
			<path d="M291.5 52L294.5 55L300.5 48.5" />
			<circle className="ill-accent" cx="296" cy="80" r="10" />
			<path d="M291.5 80L294.5 83L300.5 76.5" />
			<circle className="ill-clay" cx="296" cy="108" r="10" />
			<path d="M292.5 104.5L299.5 111.5M299.5 104.5L292.5 111.5" />
			<Spark x={28} y={40} s={0.6} />
			<Spark x={334} y={26} s={0.7} />
		</>
	),
	// Tests: a clipboard of results, two ticks and one to look at, before three browsers.
	tests: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<Win x={196} y={22} w={110} h={66} />
			<Win x={214} y={40} w={110} h={66} />
			<Win x={232} y={58} w={100} h={64} />
			<path className="ill-code" d="M246 86H300M246 98H284M246 110H310" />
			<rect className="ill-tint" x={60} y={18} width={110} height={112} rx="10" />
			<rect className="ill-paper" x={72} y={34} width={86} height={88} rx="4" />
			<rect className="ill-paper" x={96} y={12} width={38} height={16} rx="5" />
			<circle className="ill-accent" cx="88" cy="54" r="7" />
			<path d="M85 54L87.5 56.5L92 51.5" />
			<path className="ill-code" d="M102 54H146" />
			<circle className="ill-accent" cx="88" cy="78" r="7" />
			<path d="M85 78L87.5 80.5L92 75.5" />
			<path className="ill-code" d="M102 78H140" />
			<circle className="ill-clay" cx="88" cy="102" r="7" />
			<path d="M85.5 99.5L90.5 104.5M90.5 99.5L85.5 104.5" />
			<path className="ill-code" d="M102 102H134" />
			<Spark x={34} y={40} s={0.6} />
			<Spark x={334} y={30} s={0.7} />
		</>
	),
	// Git: main runs along, a branch leaves and comes back, the merge in green.
	git: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<path d="M40 96H320" />
			<path d="M100 96C120 96 124 52 150 52H210C236 52 240 96 260 96" />
			<path d="M170 52C186 52 190 26 210 26H236" />
			{[[60, 96], [100, 96], [150, 52], [180, 52], [210, 52], [210, 26], [236, 26], [300, 96]].map(([cx, cy], i) => (
				<circle key={i} className="ill-paper" cx={cx} cy={cy} r="7" />
			))}
			<circle className="ill-accent" cx="260" cy="96" r="9" />
			<path className="ill-clay" d="M236 26V4L256 9L236 14" />
			<Fly x={258} y={30} w={44} tilt={-14} />
			<Spark x={40} y={50} s={0.6} />
		</>
	),
	// Remote: the window on your desk, a wire over the hill to the machine that does the work.
	remote: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<path className="ill-tint" d="M120 124C150 88 196 80 236 124Z" />
			<Laptop x={36} y={56} w={80} h={52} />
			<CodeLines x={46} y={70} lines={[[0, 34], [8, 48], [8, 26]]} gap={10} accent={1} />
			<path className="ill-wire" d="M112 70C150 10 220 10 262 52" />
			<path className="ill-wire-flow" d="M112 70C150 10 220 10 262 52" />
			<path className="ill-paper" d="M246 62L282 34L318 62V122H246Z" />
			<rect className="ill-tint" x={258} y={70} width={48} height={44} rx="4" />
			<path d="M264 82H300M264 94H300M264 106H300" />
			<circle className="ill-accent ill-blink" cx="294" cy="82" r="2.4" />
			<circle className="ill-accent ill-blink ill-blink-late" cx="294" cy="94" r="2.4" />
			<SkyCloud x={150} y={60} s={0.7} className="ill-drift" />
			<Spark x={334} y={34} s={0.6} />
		</>
	),
	// Open source: an open box, the code inside, the butterfly flying out of it.
	open: () => (
		<>
			<path className="ill-blob" d={WIDE_BLOB} />
			<path className="ill-thread" d="M180 64C190 40 226 50 236 26" />
			<rect className="ill-paper" x={150} y={36} width={60} height={58} rx="4" />
			<CodeLines x={160} y={50} lines={[[0, 26], [6, 34], [6, 20], [0, 12]]} gap={10} accent={1} />
			<path className="ill-sand" d="M130 76H230V124H130Z" />
			<path className="ill-paper" d="M130 76L104 58L118 52L150 74ZM230 76L256 58L242 52L210 74Z" />
			<path d="M164 100H196" />
			<Fly x={226} y={-6} w={64} tilt={14} glasses />
			<path d="M78 96V66" />
			<path className="ill-paper" d="M60 50H96V70H60Z" />
			<text className="ill-label" x="78" y="64" textAnchor="middle">MIT</text>
			<Spark x={44} y={30} s={0.6} />
			<Spark x={318} y={96} s={0.7} />
		</>
	)
}

export type SectionArtKind = keyof typeof SECTION_ART

export function SectionArt({ art }: { art: SectionArtKind }) {
	const Art = SECTION_ART[art]
	return (
		<svg className={`ill ill-section ill-section-${art}`} viewBox="0 0 360 140" aria-hidden="true" focusable="false">
			<Art />
		</svg>
	)
}

// ---------------------------------------------------------------------------------------------
// Card drawings (0 0 200 100): the feature grid and the download cards
// ---------------------------------------------------------------------------------------------

const CARD_ART = {
	// The debugger: a page of code stopped at a red dot, the line held in warm light.
	debug: () => (
		<>
			<rect className="ill-paper" x={46} y={12} width={108} height={78} rx="7" />
			<path className="ill-warm" d="M46 46H154V58H46Z" />
			<circle className="ill-clay" cx="58" cy="52" r="4.5" />
			<CodeLines x={70} y={26} lines={[[0, 46], [8, 60], [8, 52], [16, 40], [8, 28], [0, 14]]} gap={10} />
			<circle className="ill-accent" cx="156" cy="80" r="14" />
			<path d="M151 74V86M161 74V86" />
		</>
	),
	// The terminal: a prompt and a green block cursor.
	terminal: () => (
		<>
			<Win x={40} y={14} w={120} h={74} bar="ill-tint" fill="ill-ink-panel" />
			<path className="ill-on-ink" d="M54 44L62 50L54 56" />
			<path className="ill-on-ink" d="M68 56H96" />
			<rect className="ill-accent ill-blink" x={100} y={46} width={9} height={12} rx="1.5" />
			<path className="ill-on-ink ill-faint" d="M54 72H120" />
		</>
	),
	// Search everywhere: a magnifier over the pages, the match lit.
	search: () => (
		<>
			<rect className="ill-paper" x={40} y={22} width={64} height={70} rx="5" />
			<rect className="ill-paper" x={56} y={10} width={64} height={70} rx="5" />
			<CodeLines x={66} y={26} lines={[[0, 40], [0, 30], [0, 44], [0, 24]]} gap={11} warm={2} />
			<circle className="ill-glass" cx="130" cy="56" r="22" />
			<path d="M146 72L166 92" className="ill-thick" />
			<path className="ill-accent" d="M118 52H140V60H118Z" />
		</>
	),
	// Node scripts and tests: a package with a green play button on its label.
	node: () => (
		<>
			<path className="ill-sand" d="M52 40L100 22L148 40V80L100 96L52 80Z" />
			<path d="M52 40L100 58L148 40M100 58V96" />
			<path className="ill-paper" d="M66 46L92 56V78L66 68Z" />
			<circle className="ill-accent" cx="124" cy="68" r="11" />
			<path className="ill-on-accent" d="M121 62.5V73.5L130 68Z" />
		</>
	),
	// Settings as text: a gear beside a page of JSON.
	settings: () => (
		<>
			<rect className="ill-paper" x={96} y={14} width={64} height={76} rx="6" />
			<Brace x={106} y={26} h={52} />
			<Brace x={150} y={26} h={52} flip />
			<path className="ill-code" d="M120 38H138M124 52H142M124 66H134" />
			<g className="ill-spin-slow">
				<circle className="ill-accent" cx="64" cy="54" r="20" />
				<circle className="ill-paper" cx="64" cy="54" r="7" />
				<path d="M64 26V32M64 76V82M36 54H42M86 54H92M44 34L48 38M80 70L84 74M44 74L48 70M80 38L84 34" />
			</g>
		</>
	),
	// The keymap: two keys, the command key and K.
	keymap: () => (
		<>
			<rect className="ill-tint" x={30} y={28} width={140} height={56} rx="10" />
			<rect className="ill-paper" x={50} y={36} width={44} height={38} rx="7" />
			<rect className="ill-accent" x={106} y={36} width={44} height={38} rx="7" />
			<text className="ill-key" x="72" y="62" textAnchor="middle">⌘</text>
			<text className="ill-key" x="128" y="62" textAnchor="middle">K</text>
		</>
	),
	// macOS: a laptop, the butterfly on its screen.
	mac: () => (
		<>
			<Laptop x={52} y={16} w={96} h={62} />
			<Fly x={78} y={24} w={44} tilt={0} />
		</>
	),
	// Linux: two packages ready to install, the green one on top.
	linux: () => (
		<>
			<path className="ill-sand" d="M40 58L72 46L104 58V86L72 96L40 86Z" />
			<path d="M40 58L72 70L104 58M72 70V96" />
			<path className="ill-accent" d="M96 36L128 24L160 36V64L128 74L96 64Z" />
			<path d="M96 36L128 48L160 36M128 48V74" />
			<path className="ill-code" d="M50 76L62 80" />
		</>
	),
	// Windows: a monitor with an installer's progress bar.
	windows: () => (
		<>
			<rect className="ill-paper" x={46} y={10} width={108} height={70} rx="6" />
			<path d="M100 80V90M78 92H122" />
			<rect className="ill-tint" x={64} y={26} width={72} height={38} rx="4" />
			<rect className="ill-paper" x={72} y={46} width={56} height={8} rx="3" />
			<rect className="ill-accent" x={72} y={46} width={36} height={8} rx="3" />
			<path className="ill-code" d="M72 36H110" />
		</>
	),
	// The agent on this computer: a laptop, the agent's line typed in, the run going green.
	here: () => (
		<>
			<Laptop x={52} y={14} w={96} h={62} />
			<CodeLines x={64} y={30} lines={[[0, 40], [8, 52], [8, 34]]} gap={11} warm={1} />
			<circle className="ill-accent" cx="128" cy="60" r="8" />
			<path className="ill-on-accent" d="M126 56V64L132 60Z" />
		</>
	),
	// Over SSH: the window on the desk, a wire to the machine where the agent runs.
	ssh: () => (
		<>
			<Laptop x={22} y={34} w={64} h={42} />
			<CodeLines x={32} y={46} lines={[[0, 28], [6, 36]]} gap={10} accent={1} />
			<path className="ill-wire" d="M84 48C110 12 136 12 150 38" />
			<path className="ill-wire-flow" d="M84 48C110 12 136 12 150 38" />
			<path className="ill-paper" d="M132 46L156 28L180 46V90H132Z" />
			<rect className="ill-tint" x={142} y={54} width={28} height={30} rx="3" />
			<path d="M147 63H165M147 73H165" />
			<circle className="ill-accent ill-blink" cx="162" cy="63" r="2" />
		</>
	),
	// WSL: a Windows monitor with a Linux shell inside it, where the agent works.
	wsl: () => (
		<>
			<rect className="ill-paper" x={46} y={10} width={108} height={70} rx="6" />
			<path d="M100 80V90M78 92H122" />
			<rect className="ill-ink-panel" x={60} y={22} width={80} height={46} rx="4" />
			<path className="ill-on-ink" d="M70 38L77 43L70 48" />
			<path className="ill-on-ink" d="M82 48H110" />
			<rect className="ill-accent ill-blink" x={114} y={39} width={7} height={10} rx="1.5" />
			<path className="ill-on-ink ill-faint" d="M70 58H128" />
		</>
	),
	// Building from source: a terminal, a gear turning beside it.
	source: () => (
		<>
			<Win x={34} y={16} w={104} h={70} fill="ill-ink-panel" />
			<path className="ill-on-ink" d="M46 46L54 52L46 58" />
			<path className="ill-on-ink" d="M60 58H100" />
			<g className="ill-spin-slow">
				<circle className="ill-accent" cx="150" cy="70" r="16" />
				<circle className="ill-paper" cx="150" cy="70" r="5.5" />
				<path d="M150 48V53M150 87V92M128 70H133M167 70H172M134 54L138 58M162 82L166 86M134 86L138 82M162 58L166 54" />
			</g>
		</>
	)
}

export type CardArtKind = keyof typeof CARD_ART

export function CardArt({ art }: { art: CardArtKind }) {
	const Art = CARD_ART[art]
	return (
		<svg className="ill ill-card" viewBox="0 0 200 100" aria-hidden="true" focusable="false">
			<Art />
		</svg>
	)
}

// ---------------------------------------------------------------------------------------------
// Flow drawings (0 0 120 80): the steps from what you ask to the window doing it
// ---------------------------------------------------------------------------------------------

const FLOW_ART = {
	// The agent in the console.
	agent: () => <Bot x={60} y={78} />,
	// The skill: a page the agent keeps a bookmark in, read when it needs it.
	skill: () => (
		<>
			<rect className="ill-paper" x={34} y={6} width={52} height={68} rx="5" />
			<CodeLines x={44} y={24} lines={[[0, 26], [0, 32], [0, 20], [0, 28]]} gap={11} />
			<path className="ill-accent" d="M70 6V30L76 25L82 30V6Z" />
		</>
	),
	// The API: a call in braces, sent on.
	api: () => (
		<>
			<Brace x={26} y={14} h={52} />
			<Brace x={78} y={14} h={52} flip />
			<CodeLines x={40} y={28} lines={[[0, 30], [4, 26], [0, 20]]} gap={12} accent={1} />
			<path d="M94 40H112M106 34L112 40L106 46" />
		</>
	),
	// The window doing it: a run started, the tests back.
	window: () => (
		<>
			<Win x={14} y={8} w={92} h={64} />
			<CodeLines x={26} y={32} lines={[[0, 34], [6, 44]]} gap={11} />
			<circle className="ill-accent" cx="88" cy="56" r="8" />
			<path className="ill-on-accent" d="M86 52V60L92 56Z" />
			<circle className="ill-clay" cx="30" cy="58" r="4" />
			<circle className="ill-accent" cx="42" cy="58" r="4" />
			<circle className="ill-accent" cx="54" cy="58" r="4" />
		</>
	)
}

export type FlowArtKind = keyof typeof FLOW_ART

export function FlowArt({ art }: { art: FlowArtKind }) {
	const Art = FLOW_ART[art]
	return (
		<svg className="ill ill-flow" viewBox="0 0 120 80" aria-hidden="true" focusable="false">
			<Art />
		</svg>
	)
}

// ---------------------------------------------------------------------------------------------
// Scenes (0 0 1200 340): each ends a page, edge to edge like a horizon
// ---------------------------------------------------------------------------------------------

function Scene({ className = '', children }: { className?: string, children: ReactNode }) {
	return (
		<svg className={`ill page-scene ${className}`} viewBox="0 0 1200 340" preserveAspectRatio="xMidYMax slice" aria-hidden="true" focusable="false">
			{children}
		</svg>
	)
}

// The homepage's horizon at the end of the day: a workshop with its window full of code and
// its lamp lit, a mug of coffee on the porch, a Git tree bearing commits, a garden of sprouts,
// and a wire over the hills to the small server hut where the remote work runs. The butterfly
// flies over it all, spectacles on.
export function WorkshopScene() {
	return (
		<Scene>
			<circle className="ill-warm" cx="910" cy="150" r="64" />
			<path className="ill-ray" d="M910 62V74M982 86L972 96M838 86L848 96M1014 150H1000M806 150H820" />
			<SkyCloud x={330} y={88} s={1.3} className="ill-drift" />
			<SkyCloud x={1000} y={82} s={1.1} className="ill-drift ill-drift-slow" />
			<path d="M520 96L530 104L540 96M556 84L564 90L572 84" />
			<path className="ill-tint" d="M-10 214C110 152 230 174 340 144C460 110 560 178 680 154C800 130 900 190 1020 152C1100 128 1160 150 1210 146V340H-10Z" />
			<path className="ill-accent" d="M-10 262C180 214 360 226 520 250C680 274 820 220 1000 230C1100 236 1160 250 1210 246V340H-10Z" />

			{/* The server hut on the far hill, its lights awake, and the wire to it. */}
			<g transform="translate(1040 196)">
				<path className="ill-paper" d="M-26 0V-34L0 -54L26 -34V0Z" />
				<rect className="ill-tint" x="-14" y="-30" width="28" height="26" rx="3" />
				<path d="M-10 -22H10M-10 -14H10" />
				<circle className="ill-accent ill-blink" cx="7" cy="-22" r="2" />
				<circle className="ill-accent ill-blink ill-blink-late" cx="7" cy="-14" r="2" />
			</g>
			<path d="M820 236V176M812 182H828M960 214V160M952 166H968" />
			<path className="ill-wire" d="M574 186C660 200 750 196 820 180C880 172 920 172 960 164C990 160 1010 170 1026 178" />
			<path className="ill-wire-flow" d="M574 186C660 200 750 196 820 180C880 172 920 172 960 164C990 160 1010 170 1026 178" />

			<GitTree x={200} y={252} s={1} />
			<Pine x={96} y={256} s={0.85} />
			<Pine x={128} y={252} s={0.65} />

			{/* The workshop. */}
			<g transform="translate(380 252)">
				<path className="ill-steam" d="M150 -112C144 -122 156 -128 150 -140C146 -148 154 -154 152 -160" />
				<path className="ill-paper" d="M142 -104H158V-80H142Z" />
				<path className="ill-paper" d="M0 0V-70H180V0Z" />
				<path className="ill-clay" d="M-14 -68L90 -122L194 -68Z" />
				<rect className="ill-warm" x="14" y="-56" width="92" height="44" rx="4" />
				<CodeLines x={24} y={-46} lines={[[0, 30], [8, 46], [8, 36], [16, 22], [0, 12]]} gap={7} accent={1} />
				<path className="ill-ink-dot" d="M128 -50H156V0H128Z" />
				<circle className="ill-warm" cx="150" cy="-24" r="2.4" />
				<path d="M-24 0H210" />
				{/* the porch table and its mug */}
				<path d="M212 -26H252M220 -26V0M244 -26V0" />
				<path className="ill-clay" d="M226 -42H240V-28H226Z" />
				<path d="M240 -38C246 -38 246 -32 240 -32" />
				<path className="ill-steam" d="M232 -48C229 -54 235 -56 232 -62" />
			</g>

			{/* A garden of sprouts: the Spring beds. */}
			{[680, 712, 744, 776].map((x, i) => (
				<g key={x} transform={`translate(${x} ${270 + (i % 2) * 6})`}>
					<path d="M0 0V-16" />
					<path className="ill-paper" d="M0 -10C-10 -10 -14 -18 -12 -24C-4 -24 0 -18 0 -10Z" />
					<path className="ill-paper" d="M0 -14C10 -14 14 -22 12 -28C4 -28 0 -22 0 -14Z" />
				</g>
			))}
			<path d="M664 282C700 276 760 278 796 284" />
			<RoundTree x={1110} y={250} s={1.1} />

			<path className="ill-sand" d="M-10 306C200 274 420 288 600 302C760 314 900 282 1210 294V350H-10Z" />
			<path className="ill-thread" d="M560 254C572 276 520 284 540 306" />
			<path d="M300 300C306 294 312 294 318 300M880 298C886 292 892 292 898 298" />

			<path className="ill-thread" d="M520 146C560 92 610 142 652 104" />
			<Fly x={640} y={60} w={92} tilt={-10} glasses />
			<Spark x={760} y={86} s={0.9} />
			<Spark x={120} y={136} s={0.6} />
			<Spark x={1150} y={118} s={0.7} />
		</Scene>
	)
}

// The download page's horizon: a balloon carrying the parcel over the hills to three houses, a
// window lit in each, the road between them.
export function ParcelScene() {
	return (
		<Scene>
			<circle className="ill-warm" cx="250" cy="140" r="54" />
			<SkyCloud x={900} y={92} s={1.3} className="ill-drift" />
			<SkyCloud x={120} y={84} s={1} className="ill-drift ill-drift-slow" />
			<path className="ill-tint" d="M-10 222C120 170 260 190 380 160C520 124 620 192 760 168C880 148 980 196 1100 166C1150 154 1190 160 1210 158V340H-10Z" />
			<path className="ill-accent" d="M-10 270C180 228 380 236 560 260C720 282 860 232 1040 240C1120 244 1170 252 1210 250V340H-10Z" />

			{[[300, 262, 'ill-clay'], [600, 276, 'ill-tint'], [900, 258, 'ill-clay']].map(([x, y, roof]) => (
				<g key={x as number} transform={`translate(${x} ${y})`}>
					<path className="ill-paper" d="M-40 0V-48H40V0Z" />
					<path className={roof as string} d="M-52 -46L0 -84L52 -46Z" />
					<rect className="ill-warm" x="-26" y="-38" width="20" height="18" rx="2" />
					<path d="M-16 -38V-20M-26 -29H-6" />
					<path className="ill-ink-dot" d="M8 -30H24V0H8Z" />
				</g>
			))}
			<path className="ill-sand" d="M-10 310C220 280 420 290 600 306C780 320 940 288 1210 298V350H-10Z" />
			<path className="ill-thread" d="M240 306C380 290 480 300 600 306C720 312 820 296 960 300" />

			{/* the balloon and its parcel */}
			<g className="ill-bob">
				<g transform="translate(0 44)">
				<path className="ill-warm" d="M720 24C760 24 784 52 780 84C776 110 748 128 736 142H704C692 128 664 110 660 84C656 52 680 24 720 24Z" />
				<path d="M720 24C704 44 700 108 712 142M720 24C736 44 740 108 728 142" />
				<path d="M706 142L710 164M734 142L730 164" />
				<rect className="ill-sand" x="700" y="164" width="40" height="30" rx="4" />
				<path d="M700 176H740" />
				<svg className="ill-fly" style={{ strokeWidth: flyStroke(24) }} x="708" y="178" width="24" height="20" viewBox="0 0 120 100" overflow="visible"><FlyBody /></svg>
				</g>
			</g>
			<path className="ill-thread" d="M652 168C600 196 520 186 470 214" />
			<RoundTree x={440} y={262} s={0.9} />
			<Pine x={1040} y={252} s={0.8} />
			<Pine x={1070} y={248} s={0.6} />
			<Pine x={160} y={258} s={0.7} />
			<Spark x={820} y={92} s={0.9} />
			<Spark x={180} y={180} s={0.6} />
		</Scene>
	)
}

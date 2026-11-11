// Button icons: the button's own ink (currentColor) plus one green fill, 18px, on the leading
// edge of a button.
const GLYPHS = {
	download: (
		<>
			<path className="glyph-accent" d="M4 14h16v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1z" />
			<path d="M12 3v10M7.5 8.5 12 13l4.5-4.5M4 14v5a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-5" />
		</>
	),
	book: (
		<>
			<path className="glyph-accent" d="M12 6.5C10 5 7 4.5 4 5v13c3-.5 6 0 8 1.5z" />
			<path d="M12 6.5C10 5 7 4.5 4 5v13c3-.5 6 0 8 1.5m0-13c2-1.5 5-2 8-1.5v13c-3-.5-6 0-8 1.5m0-13v13" />
		</>
	),
	terminal: (
		<>
			<rect className="glyph-accent" x="13" y="14" width="5" height="2.4" rx="1" />
			<rect x="3" y="4" width="18" height="16" rx="2.5" />
			<path d="m7 9 3 3-3 3" />
		</>
	),
	star: (
		<path className="glyph-star" d="m12 3.5 2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" />
	)
}

export type GlyphKind = keyof typeof GLYPHS

export function Glyph({ kind }: { kind: GlyphKind }) {
	return (
		<svg className={`glyph glyph-${kind}`} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
			{GLYPHS[kind]}
		</svg>
	)
}

// The GitHub mark, for the links that go to the repository.
export function GitHubMark() {
	return (
		<svg className="glyph glyph-github" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
			<path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z" />
		</svg>
	)
}

// The systems' own marks, for the download page's tabs: one ink (currentColor), so they follow
// the theme and the tab's state, except Tux, who reads only in his own colours.
export function OsGlyph({ os }: { os: 'mac' | 'windows' | 'linux' }) {
	if (os === 'windows') {
		return (
			<svg className="glyph-os" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
				<path fill="currentColor" d="M3 5.6 10.4 4.6v6.9H3V5.6Zm0 12.8 7.4 1v-6.8H3v5.8Zm8.3 1.1L21 21V12.5h-9.7v7Zm0-15.2v7H21V3l-9.7 1.3Z" />
			</svg>
		)
	}
	if (os === 'mac') {
		return (
			<svg className="glyph-os" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
				<path fill="currentColor" d="M16.4 12.6c0-2.1 1.7-3.1 1.8-3.2-1-1.4-2.5-1.6-3-1.7-1.3-.1-2.5.8-3.1.8-.6 0-1.6-.7-2.7-.7-1.4 0-2.7.8-3.4 2-1.5 2.5-.4 6.3 1 8.4.7 1 1.5 2.1 2.6 2.1 1 0 1.4-.7 2.7-.7s1.6.7 2.7.7c1.1 0 1.8-1 2.5-2 .8-1.2 1.1-2.3 1.1-2.3-.1 0-2.1-.8-2.2-3.1ZM14.3 6c.6-.7 1-1.7.9-2.7-.8 0-1.9.6-2.5 1.3-.5.6-1 1.6-.9 2.6.9.1 1.8-.5 2.5-1.2Z" />
			</svg>
		)
	}
	return (
		<svg className="glyph-os" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
			<ellipse cx="9.2" cy="20.6" rx="2.3" ry="1.1" fill="#f6a623" />
			<ellipse cx="14.8" cy="20.6" rx="2.3" ry="1.1" fill="#f6a623" />
			<path d="M12 2.4c-3.1 0-5.1 2.5-5.1 6.1 0 1.4.1 2.6.1 4.1 0 4 2.1 7 5 7s5-3 5-7c0-1.5.1-2.7.1-4.1 0-3.6-2-6.1-5.1-6.1Z" fill="#1c1c1c" />
			<path d="M12 9c-1.9 0-3 1.9-3 5 0 2.7 1.2 4.8 3 4.8s3-2.1 3-4.8c0-3.1-1.1-5-3-5Z" fill="#fff" />
			<circle cx="10.4" cy="7.3" r="1" fill="#fff" />
			<circle cx="13.6" cy="7.3" r="1" fill="#fff" />
			<circle cx="10.6" cy="7.4" r="0.45" fill="#1c1c1c" />
			<circle cx="13.4" cy="7.4" r="0.45" fill="#1c1c1c" />
			<path d="M10.7 8.6 12 10.2l1.3-1.6z" fill="#f6a623" />
		</svg>
	)
}

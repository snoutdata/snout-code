import { useEffect, useRef, useState } from 'react'
import { Brand } from './Brand'
import { GitHubMark } from './Glyph'
import { LangSwitcher } from './LangSwitcher'
import { ThemeToggle } from './ThemeToggle'
import { useT } from '../i18n'
import { DOCS_URL, GITHUB_URL } from '../site'

// The product sections of the landing page, grouped under one Features dropdown so the bar keeps
// only the destinations: features, open source, docs, download.
const FEATURES = [
	{ href: '/#features', key: 'java' },
	{ href: '/#playwright', key: 'playwright' },
	{ href: '/#node', key: 'node' },
	{ href: '/#agents', key: 'agents' },
	{ href: '/#git', key: 'git' },
	{ href: '/#remote', key: 'remote' }
]

const LINKS = [
	{ href: '/#open-source', key: 'openSource' },
	{ href: DOCS_URL, key: 'docs' },
	{ href: '/download', key: 'download' }
]

export function SiteHeader({ active }: { active?: string }) {
	const t = useT()
	const [menuOpen, setMenuOpen] = useState(false)
	const headerRef = useRef<HTMLElement>(null)

	// Close the mobile menu on outside click or Escape, like a normal popover.
	useEffect(() => {
		if (!menuOpen) {
			return
		}
		function onPointer(e: PointerEvent) {
			if (headerRef.current && !headerRef.current.contains(e.target as Node)) {
				setMenuOpen(false)
			}
		}
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				setMenuOpen(false)
			}
		}
		document.addEventListener('pointerdown', onPointer)
		document.addEventListener('keydown', onKey)
		return () => {
			document.removeEventListener('pointerdown', onPointer)
			document.removeEventListener('keydown', onKey)
		}
	}, [menuOpen])

	const links = LINKS.map((l) => (
		<a key={l.href} href={l.href} className={active === l.href ? 'active' : undefined}>{t(`nav.${l.key}`)}</a>
	))

	return (
		<header className="nav-wrap" ref={headerRef}>
			<nav className="nav" aria-label={t('nav.main')}>
				<Brand />
				<ul className="nav-links">
					<li>
						<FeaturesMenu />
					</li>
					{links.map((link) => <li key={link.key}>{link}</li>)}
				</ul>
				<div className="nav-actions">
					<a className="nav-github" href={GITHUB_URL} aria-label={t('nav.github')}>
						<GitHubMark />
						<span>GitHub</span>
					</a>
					<LangSwitcher />
					{/* Theme lives in the bar; on phones (<=620) it moves into the menu. */}
					<span className="nav-theme">
						<ThemeToggle />
					</span>
					<button
						className="icon-btn nav-hamburger"
						type="button"
						aria-label={t('nav.menu')}
						aria-expanded={menuOpen}
						aria-controls="nav-panel"
						onClick={() => setMenuOpen((v) => !v)}
					>
						{menuOpen ? <CloseIcon /> : <MenuIcon />}
					</button>
				</div>
			</nav>
			{menuOpen && (
				// The links are in-page anchors that do not reload the page, so following one closes the menu.
				<div
					className="nav-panel"
					id="nav-panel"
					onClick={(e) => {
						if ((e.target as Element).closest('a')) setMenuOpen(false)
					}}
				>
					<FeaturesMenu />
					{links}
					<div className="nav-panel-theme">
						<span>{t('theme.label')}</span>
						<ThemeToggle />
					</div>
				</div>
			)}
		</header>
	)
}

// The same <details> dropdown as snoutdata.com's: native open/close and keyboard support, closed by
// a pointer outside, by focus leaving it, by Escape, and (since these are in-page anchors that do
// not reload the page) by following one of its links.
function FeaturesMenu() {
	const t = useT()
	const ref = useRef<HTMLDetailsElement>(null)
	useEffect(() => {
		function closeOutside(event: PointerEvent) {
			if (ref.current && !ref.current.contains(event.target as Node)) ref.current.open = false
		}
		document.addEventListener('pointerdown', closeOutside)
		return () => document.removeEventListener('pointerdown', closeOutside)
	}, [])
	return (
		<details className="nav-menu-group" ref={ref}
			// Close when keyboard focus moves to something OUTSIDE. A blur with no relatedTarget is not
			// that: WebKit does not focus a link on tap, so closing on it would hide the links before
			// the tap landed. Taps outside are the pointerdown's job.
			onBlur={(event) => {
				const next = event.relatedTarget
				if (next instanceof Node && !event.currentTarget.contains(next)) event.currentTarget.open = false
			}}
			onKeyDown={(event) => {
				if (event.key === 'Escape' && ref.current?.open) {
					event.stopPropagation()
					ref.current.open = false
					ref.current.querySelector('summary')?.focus()
				}
			}}
		>
			<summary>
				{t('nav.features')}
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
			</summary>
			<div className="nav-menu-links">
				{FEATURES.map(({ href, key }) => (
					<a key={href} href={href} onClick={() => { if (ref.current) ref.current.open = false }}>{t(`nav.${key}`)}</a>
				))}
			</div>
		</details>
	)
}

function MenuIcon() {
	return (
		<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
			<path d="M3 6h18M3 12h18M3 18h18" />
		</svg>
	)
}

function CloseIcon() {
	return (
		<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
			<path d="M18 6 6 18M6 6l12 12" />
		</svg>
	)
}

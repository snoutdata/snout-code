import { Brand } from './Brand'
import { GitHubMark } from './Glyph'
import { ThemeToggle } from './ThemeToggle'
import { DOCS_URL, GITHUB_URL } from '../site'

const LINKS = [
	{ href: '/#features', label: 'Features' },
	{ href: '/#playwright', label: 'Playwright' },
	{ href: '/#node', label: 'Node' },
	{ href: '/#agents', label: 'Agents' },
	{ href: '/#open-source', label: 'Open source' },
	{ href: DOCS_URL, label: 'Docs' },
	{ href: '/download', label: 'Download' }
]

export function SiteHeader({ active }: { active?: string }) {
	return (
		<header className="nav-wrap">
			<nav className="nav" aria-label="Main">
				<Brand />
				<ul className="nav-links">
					{LINKS.map((l) => (
						<li key={l.href}>
							<a href={l.href} className={active === l.href ? 'active' : undefined}>{l.label}</a>
						</li>
					))}
				</ul>
				<div className="nav-actions">
					<a className="nav-github" href={GITHUB_URL} aria-label="SnoutCode on GitHub">
						<GitHubMark />
						<span>GitHub</span>
					</a>
					<ThemeToggle />
				</div>
			</nav>
		</header>
	)
}

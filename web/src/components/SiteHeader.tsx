import { Brand } from './Brand'
import { GitHubMark } from './Glyph'
import { LangSwitcher } from './LangSwitcher'
import { ThemeToggle } from './ThemeToggle'
import { useT } from '../i18n'
import { DOCS_URL, GITHUB_URL } from '../site'

const LINKS = [
	{ href: '/#features', key: 'features' },
	{ href: '/#playwright', key: 'playwright' },
	{ href: '/#node', key: 'node' },
	{ href: '/#agents', key: 'agents' },
	{ href: '/#open-source', key: 'openSource' },
	{ href: DOCS_URL, key: 'docs' },
	{ href: '/download', key: 'download' }
]

export function SiteHeader({ active }: { active?: string }) {
	const t = useT()
	return (
		<header className="nav-wrap">
			<nav className="nav" aria-label={t('nav.main')}>
				<Brand />
				<ul className="nav-links">
					{LINKS.map((l) => (
						<li key={l.href}>
							<a href={l.href} className={active === l.href ? 'active' : undefined}>{t(`nav.${l.key}`)}</a>
						</li>
					))}
				</ul>
				<div className="nav-actions">
					<a className="nav-github" href={GITHUB_URL} aria-label={t('nav.github')}>
						<GitHubMark />
						<span>GitHub</span>
					</a>
					<LangSwitcher />
					<ThemeToggle />
				</div>
			</nav>
		</header>
	)
}

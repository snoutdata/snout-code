import { Brand } from './Brand'
import { useT, withSlots } from '../i18n'
import { CONTRIBUTING_URL, DOCS_URL, GITHUB_URL, ISSUES_URL, LICENSE_URL, RELEASES_URL, SNOUTDATA_URL, docs } from '../site'

const COLUMNS = [
	{
		key: 'product',
		links: [
			{ href: '/#features', key: 'features' },
			{ href: '/download', key: 'download' },
			{ href: RELEASES_URL, key: 'releases' },
			{ href: docs('keyboard-shortcuts.md'), key: 'shortcuts' }
		]
	},
	{
		key: 'docs',
		links: [
			{ href: DOCS_URL, key: 'guide' },
			{ href: docs('getting-started.md'), key: 'gettingStarted' },
			{ href: docs('playwright.md'), key: 'playwright' },
			{ href: docs('node.md'), key: 'node' },
			{ href: docs('agents.md'), key: 'agents' },
			{ href: docs('remote-ssh.md'), key: 'remote' }
		]
	},
	{
		key: 'openSource',
		links: [
			{ href: GITHUB_URL, key: 'github' },
			{ href: ISSUES_URL, key: 'issues' },
			{ href: CONTRIBUTING_URL, key: 'contributing' },
			{ href: LICENSE_URL, key: 'license' }
		]
	}
]

export function SiteFooter() {
	const t = useT()
	return (
		<footer className="site-footer">
			<div className="footer-inner">
				<div className="footer-brand">
					<Brand />
					<p>{t('footer.tagline')}</p>
					<p className="footer-family">
						{withSlots(t('footer.family'), { link: <a href={SNOUTDATA_URL}>SnoutData</a> })}
					</p>
				</div>
				{COLUMNS.map((c) => (
					<div className="footer-col" key={c.key}>
						<h3>{t(`footer.${c.key}.title`)}</h3>
						<ul>
							{c.links.map((l) => (
								<li key={l.key}><a href={l.href}>{t(`footer.${c.key}.${l.key}`)}</a></li>
							))}
						</ul>
					</div>
				))}
			</div>
			<div className="footer-bottom">
				<span>{t('footer.copyright')}</span>
				<span className="footer-legal">
					<a href="/privacy">{t('footer.legal.privacy')}</a>
					<a href="/terms">{t('footer.legal.terms')}</a>
					<span>{t('footer.privacy')}</span>
				</span>
			</div>
		</footer>
	)
}

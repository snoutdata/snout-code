import { Brand } from './Brand'
import { CONTRIBUTING_URL, DOCS_URL, GITHUB_URL, ISSUES_URL, LICENSE_URL, RELEASES_URL, SNOUTDATA_URL, docs } from '../site'

const COLUMNS = [
	{
		title: 'Product',
		links: [
			{ href: '/#features', label: 'Features' },
			{ href: '/download', label: 'Download' },
			{ href: RELEASES_URL, label: 'Releases' },
			{ href: docs('keyboard-shortcuts.md'), label: 'Keyboard shortcuts' }
		]
	},
	{
		title: 'Docs',
		links: [
			{ href: DOCS_URL, label: 'User guide' },
			{ href: docs('getting-started.md'), label: 'Getting started' },
			{ href: docs('agents.md'), label: 'Coding agents' },
			{ href: docs('remote-ssh.md'), label: 'Remote SSH' }
		]
	},
	{
		title: 'Open source',
		links: [
			{ href: GITHUB_URL, label: 'GitHub' },
			{ href: ISSUES_URL, label: 'Issues' },
			{ href: CONTRIBUTING_URL, label: 'Contributing' },
			{ href: LICENSE_URL, label: 'MIT license' }
		]
	}
]

export function SiteFooter() {
	return (
		<footer className="site-footer">
			<div className="footer-inner">
				<div className="footer-brand">
					<Brand />
					<p>The native IDE for Java, Gradle and Spring. Free and open source.</p>
					<p className="footer-family">
						A <a href={SNOUTDATA_URL}>SnoutData</a> project.
					</p>
				</div>
				{COLUMNS.map((c) => (
					<div className="footer-col" key={c.title}>
						<h3>{c.title}</h3>
						<ul>
							{c.links.map((l) => (
								<li key={l.label}><a href={l.href}>{l.label}</a></li>
							))}
						</ul>
					</div>
				))}
			</div>
			<div className="footer-bottom">
				<span>Copyright © 2026 SNOUTDATA INC. Released under the MIT license.</span>
				<span>No cookies, no tracking on this site.</span>
			</div>
		</footer>
	)
}

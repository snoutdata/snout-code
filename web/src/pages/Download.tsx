import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { GitHubMark, Glyph } from '../components/Glyph'
import { CardArt, PageMark, ParcelScene, type CardArtKind } from '../components/Illustrations'
import { useScrollReveal } from '../effects'
import { useI18n } from '../i18n'
import { GITHUB_URL, MS_STORE_URL, RELEASES_URL, docs } from '../site'

// Each platform's words are in the catalogs, under download.platforms.<id> (file, points, guide).
const PLATFORMS: { id: string, art: CardArtKind, name: string, guide: string }[] = [
	{ id: 'mac', art: 'mac', name: 'macOS', guide: docs('getting-started.md') },
	{ id: 'linux', art: 'linux', name: 'Linux', guide: docs('linux.md') },
	{ id: 'windows', art: 'windows', name: 'Windows', guide: docs('windows.md') }
]

export function Download() {
	useScrollReveal()
	const { t, tList } = useI18n()

	return (
		<div className="page page-download">
			<SiteHeader active="/download" />

			<main>
				<section className="page-intro">
					<h1><PageMark art="download" text={t('common.downloadSnoutCode')} /></h1>
					<p className="lede">{t('download.lede')}</p>
				</section>

				<section className="section download-grid" aria-label={t('download.label')}>
					{PLATFORMS.map((p, i) => (
						<article className="card download-card" key={p.id} data-reveal style={{ ['--i' as string]: i }}>
							<div className={`card-art tone-${(i + 1) % 4}`}><CardArt art={p.art} /></div>
							<h2>{p.name}</h2>
							<p className="download-file">{t(`download.platforms.${p.id}.file`)}</p>
							<ul className="check-list">
								{tList(`download.platforms.${p.id}.points`).map((pt, j) => <li key={j}>{pt}</li>)}
							</ul>
							<a className="btn btn-block" href={p.id === 'windows' ? MS_STORE_URL : RELEASES_URL}>
								<Glyph kind="download" />
								{p.id === 'windows' ? t('download.store') : t('download.for', { os: p.name })}
							</a>
							<a className="more-link" href={p.guide}>{t(`download.platforms.${p.id}.guide`)} <span aria-hidden="true">&rarr;</span></a>
						</article>
					))}
				</section>

				<section className="section need" aria-labelledby="need-title">
					<div className="need-grid">
						<div data-reveal>
							<span className="eyebrow">{t('download.need.eyebrow')}</span>
							<h2 id="need-title">{t('download.need.title')}</h2>
							<ul className="check-list">
								{tList('download.need.points').map((pt, j) => <li key={j}>{pt}</li>)}
							</ul>
						</div>
						<div className="source-card" data-reveal>
							<div className="card-art tone-2"><CardArt art="source" /></div>
							<h2>{t('download.source.title')}</h2>
							<p>{t('download.source.body')}</p>
							<pre className="code-block"><code><span className="code-prompt">$</span> git clone {GITHUB_URL}.git{'\n'}<span className="code-prompt">$</span> cd snout-code{'\n'}<span className="code-prompt">$</span> ./snout.sh</code></pre>
							<div className="open-links">
								<a className="btn btn-ghost" href={GITHUB_URL}><GitHubMark />{t('download.source.repo')}</a>
								<a className="btn btn-ghost" href={`${GITHUB_URL}#building`}><Glyph kind="terminal" />{t('download.source.building')}</a>
							</div>
						</div>
					</div>
				</section>
			</main>

			<ParcelScene />
			<SiteFooter />
		</div>
	)
}

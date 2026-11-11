import { useEffect, useState } from 'react'
import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { GitHubMark, Glyph, OsGlyph } from '../components/Glyph'
import { CardArt, PageMark, ParcelScene } from '../components/Illustrations'
import { useScrollReveal } from '../effects'
import { useI18n } from '../i18n'
import { ALL_RELEASES_URL, GITHUB_URL, MS_STORE_URL, docs } from '../site'
import { detectVisitor, fallbackRelease, fetchLatestRelease, FILES, isOs, megabytes, OSES, refineArch, type Arch, type FileId, type Os, type Release } from '../download'

// The download page: the visitor's system is chosen for them, and one button gets them their
// file straight from the latest GitHub release, with the system's other files under it. Each
// system's words are in the catalogs, under download.platforms.<id> (points, guide), and each
// file's under download.files.<id>. Every file is also listed as a plain link at the bottom, for a
// reader that never picks a tab, and the release archive is a click away for any other version.

const GUIDES: Record<Os, string> = {
	mac: docs('getting-started.md'),
	windows: docs('windows.md'),
	linux: docs('linux.md')
}

const TONES: Record<Os, number> = { mac: 1, linux: 2, windows: 3 }

// The files of `os`, the one for `arch` first and, on Linux, the .deb before the Arch package.
function filesFor(os: Os, arch: Arch): FileId[] {
	const own = FILES.filter((f) => f.os === os)
	return [...own.filter((f) => f.arch === arch), ...own.filter((f) => f.arch !== arch)].map((f) => f.id)
}

export function Download() {
	useScrollReveal()
	const { t, tList } = useI18n()
	// The prerendered page shows macOS; the visitor's own system is put in front once it loads.
	const [os, setOs] = useState<Os>('mac')
	const [arch, setArch] = useState<Arch>('x64')
	const [release, setRelease] = useState<Release>(fallbackRelease)

	useEffect(() => {
		let live = true
		const asked = window.location.hash.slice(1)
		const guess = detectVisitor()
		setOs(isOs(asked) ? asked : guess.os)
		setArch(guess.arch)
		refineArch(guess).then((v) => live && setArch(v.arch))
		fetchLatestRelease().then((r) => live && setRelease(r))
		return () => {
			live = false
		}
	}, [])

	function choose(next: Os) {
		setOs(next)
		history.replaceState(null, '', `#${next}`)
	}

	const name = OSES.find((o) => o.id === os)?.name ?? ''
	const [first, ...rest] = filesFor(os, arch)
	// Windows installs from the Store, so SmartScreen never stops it; its installers are the others.
	const primary = os === 'windows' ? null : first
	const others = os === 'windows' ? [first, ...rest] : rest
	const file = primary ? release.files[primary] : null
	const meta = os === 'windows'
		? [t('download.storeMeta')]
		: [t(`download.files.${primary}`), megabytes(file?.size ?? null), release.version && t('download.version', { v: release.version })]

	return (
		<div className="page page-download">
			<SiteHeader active="/download" />

			<main>
				<section className="page-intro">
					<h1><PageMark art="download" text={t('common.downloadSnoutCode')} /></h1>
					<p className="lede">{t('download.lede')}</p>
				</section>

				<section className="section download-pick" aria-label={t('download.label')}>
					<div className="download-tabs" role="tablist" aria-label={t('download.choose')}>
						{OSES.map((o) => (
							<button
								key={o.id}
								type="button"
								role="tab"
								aria-selected={o.id === os}
								className={`download-tab${o.id === os ? ' active' : ''}`}
								onClick={() => choose(o.id)}
							>
								<OsGlyph os={o.id} />
								{o.name}
							</button>
						))}
					</div>

					<article className="card download-card" role="tabpanel">
						<div className={`card-art tone-${TONES[os]}`}><CardArt art={os} /></div>
						<h2>{t('download.title', { os: name })}</h2>
						<a className="btn btn-lg download-btn" href={os === 'windows' ? MS_STORE_URL : file?.url}>
							<Glyph kind="download" />
							{os === 'windows' ? t('download.store') : t('download.for', { os: name })}
						</a>
						<p className="download-meta">{meta.filter(Boolean).join(' · ')}</p>
						{others.length > 0 && (
							<div className="download-also">
								<span>{os === 'windows' ? t('download.installer') : t('download.also', { os: name })}</span>
								<ul>
									{others.map((id) => (
										<li key={id}>
											<a href={release.files[id].url}>{t(`download.files.${id}`)}</a>
											{release.files[id].size !== null && <span className="download-size"> {megabytes(release.files[id].size)}</span>}
										</li>
									))}
								</ul>
							</div>
						)}
						<ul className="check-list">
							{tList(`download.platforms.${os}.points`).map((pt, j) => <li key={j}>{pt}</li>)}
						</ul>
						<a className="more-link" href={GUIDES[os]}>{t(`download.platforms.${os}.guide`)} <span aria-hidden="true">&rarr;</span></a>
					</article>

					<div className="download-more">
						<p>
							{release.version ? t('download.allOf', { v: release.version }) : t('download.all')}{' '}
							{OSES.map((o, i) => (
								<span key={o.id}>
									{i > 0 && '; '}
									{o.name}:{' '}
									{(o.id === 'windows' ? ['store' as const, ...filesFor(o.id, 'x64')] : filesFor(o.id, 'x64')).map((id, j) => (
										<span key={id}>
											{j > 0 && ', '}
											{id === 'store'
												? <a href={MS_STORE_URL}>{t('download.storeShort')}</a>
												: <a href={release.files[id].url}>{t(`download.files.${id}`)}</a>}
										</span>
									))}
								</span>
							))}
						</p>
						<a className="btn btn-ghost" href={ALL_RELEASES_URL}><GitHubMark />{t('download.other')}</a>
					</div>
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

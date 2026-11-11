import { Fragment } from 'react'
import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { GitHubMark, Glyph } from '../components/Glyph'
import { CardArt, HeroMark, PerchArt, SectionArt, WorkshopScene, type CardArtKind } from '../components/Illustrations'
import { HarnessSection } from '../components/Harness'
import { SectionHead, Showcase, type ShotSpec } from '../components/Sections'
import { Shot } from '../components/Shot'
import { useScrollReveal } from '../effects'
import { useI18n, withSlots } from '../i18n'
import { CONTRIBUTING_URL, GITHUB_URL, ISSUES_URL, docs } from '../site'

const MADE_FOR = ['Java', 'Gradle', 'Maven', 'Spring Boot', 'TypeScript', 'JavaScript', 'Node', 'Playwright', 'Vitest', 'Jest', 'Git']

// The agents the console runs, each with the mark the app's gutter gives its lines, in its app
// colour (styles.css, --agent-*). How SnoutCode hears its edits (docs/agents.md) is in the
// catalogs, under home.agents.roster.<id>.
const AGENTS: { id: string, name?: string, mark: string }[] = [
	{ id: 'claude', name: 'Claude Code', mark: '✳' },
	{ id: 'codex', name: 'Codex', mark: '⬢' },
	{ id: 'opencode', name: 'opencode', mark: '■' },
	{ id: 'amp', name: 'Amp', mark: '▲' },
	{ id: 'custom', mark: '◆' }
]

// Each showcase row: where it links and which screenshot it shows. Its words are in the catalogs,
// under home.showcase.<id> (title, body, points, link, alt).
type ShowcaseSpec = { id: string, href: string, shot: Omit<ShotSpec, 'alt'>, flip?: boolean }

const SHOWCASES: Record<string, ShowcaseSpec[]> = {
	features: [
		{ id: 'java', href: docs('java.md'), shot: { src: 'java-completion.png' } },
		{ id: 'gradle', href: docs('gradle.md'), shot: { src: 'gradle-window.png' }, flip: true },
		{ id: 'spring', href: docs('spring.md'), shot: { src: 'spring-injections.png' } },
		{ id: 'run', href: docs('run-and-debug.md'), shot: { src: 'run-and-debug-debugger.png', dark: 'run-and-debug-debugger-dark.png' }, flip: true }
	],
	playwright: [
		{ id: 'pwWrite', href: docs('playwright.md#writing-tests'), shot: { src: 'playwright-recording.png', w: 1600, h: 514 } },
		{ id: 'pwRun', href: docs('playwright.md#running-tests'), shot: { src: 'playwright-tests-window.png', w: 1600, h: 891 }, flip: true },
		{ id: 'pwDebug', href: docs('playwright.md#debugging-tests'), shot: { src: 'playwright-debugging.png', w: 1600, h: 812 } }
	],
	node: [
		{ id: 'nodeScripts', href: docs('node.md'), shot: { src: 'node-scripts.png', w: 1600, h: 1028 }, flip: true },
		{ id: 'nodeTests', href: docs('node.md#unit-tests-vitest-and-jest'), shot: { src: 'node-vitest.png', w: 1600, h: 1028 } }
	],
	agents: [
		{ id: 'agentsEdit', href: docs('agents.md'), shot: { src: 'agents-presence-codex.png', dark: 'agents-presence-codex-dark.png' } },
		{ id: 'agentsReview', href: docs('agents.md#the-agent-changes-window'), shot: { src: 'agents-changes.png' }, flip: true }
	],
	git: [{ id: 'git', href: docs('git.md'), shot: { src: 'git-view.png' }, flip: true }],
	remote: [{ id: 'remote', href: docs('remote-ssh.md'), shot: { src: 'remote-ssh-project.png' } }]
}

const MORE: { id: string, art: CardArtKind, href: string }[] = [
	{ id: 'search', art: 'search', href: docs('editor.md') },
	{ id: 'terminal', art: 'terminal', href: docs('terminal.md') },
	{ id: 'processes', art: 'source', href: docs('processes.md') },
	{ id: 'settings', art: 'settings', href: docs('settings-and-keymap.md') },
	{ id: 'keymap', art: 'keymap', href: docs('keyboard-shortcuts.md') },
	{ id: 'native', art: 'debug', href: docs('versions-and-performance.md') }
]

const PLATFORMS: { id: string, art: CardArtKind, name: string }[] = [
	{ id: 'mac', art: 'mac', name: 'macOS' },
	{ id: 'linux', art: 'linux', name: 'Linux' },
	{ id: 'windows', art: 'windows', name: 'Windows' }
]

// The three short cards under a section, by their ids in home.<section>.trio.
function Trio({ section, ids }: { section: string, ids: string[] }) {
	const { t } = useI18n()
	return (
		<div className="trio">
			{ids.map((id) => (
				<article className="trio-card" data-reveal key={id}>
					<h3>{t(`home.${section}.trio.${id}.title`)}</h3>
					<p>{t(`home.${section}.trio.${id}.body`)}</p>
				</article>
			))}
		</div>
	)
}

function Showcases({ list }: { list: ShowcaseSpec[] }) {
	const { t, tList } = useI18n()
	return (
		<div className="showcase-list">
			{list.map((s) => {
				const k = `home.showcase.${s.id}`
				return (
					<Fragment key={s.id}>
						<Showcase
							flip={s.flip}
							title={t(`${k}.title`)}
							body={t(`${k}.body`)}
							points={tList(`${k}.points`)}
							link={{ href: s.href, label: t(`${k}.link`) }}
							shot={{ ...s.shot, alt: t(`${k}.alt`) }}
						/>
						{s.id === 'java' && <SectionArtRow />}
					</Fragment>
				)
			})}
		</div>
	)
}

export function Home() {
	useScrollReveal()
	const { t } = useI18n()

	return (
		<div className="page page-home">
			<SiteHeader />

			<main>
				<section className="hero">
					<div className="hero-copy">
						<span className="eyebrow">{t('home.hero.eyebrow')}</span>
						<h1>
							<span className="hero-name"><HeroMark /></span>
							{t('home.hero.title')}
						</h1>
						<p className="lede">{t('home.hero.lede')}</p>
						<div className="hero-cta">
							<a className="btn btn-lg" href="/download">
								<Glyph kind="download" />
								{t('common.downloadSnoutCode')}
							</a>
							<a className="btn btn-ghost btn-lg" href={GITHUB_URL}>
								<GitHubMark />
								{t('home.hero.github')}
							</a>
						</div>
						<p className="hero-proof">{t('home.hero.proof')}</p>
					</div>
					<div className="hero-visual">
						<PerchArt />
						<Shot eager src="spring-beans.png" dark="run-and-debug-debugger-dark.png" alt={t('home.hero.shotAlt')} />
					</div>
				</section>

				<section className="made-for" aria-label={t('home.madeFor')}>
					<span className="made-for-label">{t('home.madeFor')}</span>
					<ul>
						{MADE_FOR.map((m) => <li key={m}>{m}</li>)}
					</ul>
				</section>

				<section className="section" id="features" aria-labelledby="features-title">
					<SectionHead id="features-title" art="java" eyebrow={t('home.features.eyebrow')} title={t('home.features.title')}>
						{t('home.features.sub')}
					</SectionHead>
					<Showcases list={SHOWCASES.features} />
				</section>

				<section className="section section-tint" id="playwright" aria-labelledby="pw-title">
					<SectionHead id="pw-title" art="playwright" eyebrow={t('home.playwright.eyebrow')} title={t('home.playwright.title')} flip>
						{t('home.playwright.sub')}
					</SectionHead>
					<Showcases list={SHOWCASES.playwright} />
					<Trio section="playwright" ids={['ready', 'environments', 'history']} />
				</section>

				<section className="section" id="node" aria-labelledby="node-title">
					<SectionHead id="node-title" art="node" eyebrow={t('home.node.eyebrow')} title={t('home.node.title')}>
						{t('home.node.sub')}
					</SectionHead>
					<Showcases list={SHOWCASES.node} />
					<Trio section="node" ids={['editing', 'version', 'beside']} />
					<p className="center-link" data-reveal>
						<a className="more-link" href={docs('typescript.md')}>{t('home.node.guide')} <span aria-hidden="true">&rarr;</span></a>
					</p>
				</section>

				<section className="section section-tint" id="agents" aria-labelledby="agents-title">
					<SectionHead id="agents-title" art="agents" eyebrow={t('home.agents.eyebrow')} title={t('home.agents.title')} flip>
						{t('home.agents.sub')}
					</SectionHead>
					<ul className="agent-roster" data-reveal aria-label={t('home.agents.rosterLabel')}>
						{AGENTS.map((a) => (
							<li key={a.id} className={`agent-${a.id}`}>
								<span className="agent-name"><span className="agent-mark" aria-hidden="true">{a.mark}</span>{a.name ?? t(`home.agents.roster.${a.id}.name`)}</span>
								<p className="agent-how">{t(`home.agents.roster.${a.id}.how`)}</p>
							</li>
						))}
					</ul>
					<Showcases list={SHOWCASES.agents} />
					<div className="trio">
						{['setup', 'ask'].map((id) => (
							<article className="trio-card" data-reveal key={id}>
								<h3>{t(`home.agents.trio.${id}.title`)}</h3>
								<p>{t(`home.agents.trio.${id}.body`)}</p>
							</article>
						))}
						<article className="trio-card" data-reveal>
							<h3>{t('home.agents.trio.drives.title')}</h3>
							<p>{withSlots(t('home.agents.trio.drives.body'), { link: <a className="inline-link" href="#harness">{t('home.agents.trio.drives.link')}</a> })}</p>
						</article>
					</div>
				</section>

				<HarnessSection />

				<section className="section" id="git" aria-labelledby="git-title">
					<SectionHead id="git-title" art="git" eyebrow={t('home.git.eyebrow')} title={t('home.git.title')}>
						{t('home.git.sub')}
					</SectionHead>
					<Showcases list={SHOWCASES.git} />
				</section>

				<section className="section section-tint" id="remote" aria-labelledby="remote-title">
					<SectionHead id="remote-title" art="remote" eyebrow={t('home.remote.eyebrow')} title={t('home.remote.title')} flip>
						{t('home.remote.sub')}
					</SectionHead>
					<Showcases list={SHOWCASES.remote} />
				</section>

				<section className="section" aria-labelledby="more-title">
					<div className="section-title" data-reveal>
						<span className="eyebrow">{t('home.more.eyebrow')}</span>
						<h2 id="more-title">{t('home.more.title')}</h2>
					</div>
					<div className="card-grid">
						{MORE.map((m, i) => (
							<a className="card" href={m.href} key={m.id} data-reveal style={{ ['--i' as string]: i }}>
								<div className={`card-art tone-${i % 4}`}><CardArt art={m.art} /></div>
								<h3>{t(`home.more.${m.id}.title`)}</h3>
								<p>{t(`home.more.${m.id}.body`)}</p>
							</a>
						))}
					</div>
				</section>

				<section className="section" id="open-source" aria-labelledby="open-title">
					<div className="open-band" data-reveal>
						<div className="open-copy">
							<span className="eyebrow">{t('home.open.eyebrow')}</span>
							<h2 id="open-title">{t('home.open.title')}</h2>
							<p>{t('home.open.body')}</p>
							<pre className="code-block"><code><span className="code-prompt">$</span> git clone {GITHUB_URL}.git{'\n'}<span className="code-prompt">$</span> cd snout-code && ./snout.sh</code></pre>
							<div className="open-links">
								<a className="btn" href={GITHUB_URL}><Glyph kind="star" />{t('home.open.star')}</a>
								<a className="btn btn-ghost" href={ISSUES_URL}>{t('home.open.issues')}</a>
								<a className="btn btn-ghost" href={CONTRIBUTING_URL}>{t('home.open.contributing')}</a>
							</div>
						</div>
						<div className="open-art"><SectionArt art="open" /></div>
					</div>
				</section>

				<section className="section" aria-labelledby="platforms-title">
					<div className="section-title" data-reveal>
						<span className="eyebrow">{t('home.platforms.eyebrow')}</span>
						<h2 id="platforms-title">{t('home.platforms.title')}</h2>
						<p className="section-sub">{t('home.platforms.sub')}</p>
					</div>
					<div className="platform-grid">
						{PLATFORMS.map((p, i) => (
							<a className="card platform" href="/download" key={p.id} data-reveal style={{ ['--i' as string]: i }}>
								<div className={`card-art tone-${(i + 1) % 4}`}><CardArt art={p.art} /></div>
								<h3>{p.name}</h3>
								<p>{t(`home.platforms.${p.id}`)}</p>
								<span className="more-link">{t('nav.download')} <span aria-hidden="true">&rarr;</span></span>
							</a>
						))}
					</div>
				</section>

				<section className="cta-band" data-reveal>
					<h2>{t('home.cta.title')}</h2>
					<p>{t('home.cta.sub')}</p>
					<div className="hero-cta">
						<a className="btn btn-lg" href="/download"><Glyph kind="download" />{t('common.downloadSnoutCode')}</a>
						<a className="btn btn-ghost btn-lg" href={docs('getting-started.md')}><Glyph kind="book" />{t('common.gettingStarted')}</a>
					</div>
				</section>
			</main>

			<WorkshopScene />
			<SiteFooter />
		</div>
	)
}

// A breath between two showcase rows: the build drawing, on its own.
function SectionArtRow() {
	return (
		<div className="art-row" data-reveal aria-hidden="true">
			<SectionArt art="build" />
		</div>
	)
}

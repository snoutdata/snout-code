// The homepage's section on the API for agents (docs/editor-api.md): what one sentence turns into,
// the calls the agent makes, the card that asks before a change, the tools by domain, what it
// costs in tokens, and the machines it works on. Every tool name and number here is the app's;
// keep them in step with docs/editor-api.md.

import { CardArt, FlowArt, type CardArtKind, type FlowArtKind } from './Illustrations'
import { SectionHead } from './Sections'
import { useI18n } from '../i18n'
import { docs } from '../site'

// The steps a request goes through; their words are in the catalogs, under harness.flow.<art>.
const FLOW: FlowArtKind[] = ['agent', 'skill', 'api', 'window']

// The calls behind the sentence above, as the Agent Log lists them. The log is the app's, so it
// stays in the app's English in every language.
const CALLS: { tool: string, args: string, result: string, fail?: boolean }[] = [
	{ tool: 'switchProject', args: '~/projects/shop', result: 'opened' },
	{ tool: 'listRunConfigurations', args: '', result: 'Application, All tests' },
	{ tool: 'startRun', args: 'All tests', result: 'started, run 2' },
	{ tool: 'getTestResults', args: '2', result: '41 passed, 2 failed', fail: true },
	{ tool: 'openFile', args: 'OrderTotalTest.java:38', result: 'shown' },
	{ tool: 'showInApp', args: 'What failed, and why', result: 'in Agent View' }
]

// The tools by domain; each domain's name and line are in the catalogs, under harness.domains.<id>.
const DOMAINS: { id: string, count: number, tools: string[] }[] = [
	{ id: 'projects', count: 4, tools: ['switchProject', 'showNewProject'] },
	{ id: 'runs', count: 10, tools: ['startRun', 'getTestResults', 'getPlaywrightTrace'] },
	{ id: 'code', count: 7, tools: ['findReferences', 'renameSymbol'] },
	{ id: 'editor', count: 8, tools: ['openFile', 'getCurrentSelection'] },
	{ id: 'git', count: 11, tools: ['showDiff', 'commitChanges'] },
	{ id: 'build', count: 3, tools: ['runBuildTask'] },
	{ id: 'spring', count: 2, tools: ['listBeans', 'listEndpoints'] },
	{ id: 'changes', count: 3, tools: ['listAgentChanges'] },
	{ id: 'app', count: 9, tools: ['showToolWindow', 'showInApp'] }
]

// Measured on what SnoutCode sends (docs/editor-api.md, "Why not MCP: the tokens").
const MCP_TOKENS = 5800
const SKILL_TOKENS = 140

// Where it works; the words are in the catalogs, under harness.places.<art>.
const PLACES: CardArtKind[] = ['here', 'ssh', 'wsl']

export function HarnessSection() {
	const { t, tList, locale } = useI18n()
	const mcp = MCP_TOKENS.toLocaleString(locale)
	const skill = SKILL_TOKENS.toLocaleString(locale)
	return (
		<section className="section" id="harness" aria-labelledby="harness-title">
			<SectionHead id="harness-title" art="harness" eyebrow={t('harness.eyebrow')} title={t('harness.title')}>
				{t('harness.sub')}
			</SectionHead>

			<figure className="flow" data-reveal>
				<blockquote className="flow-ask">
					<span className="flow-ask-who">{t('harness.you')}</span>
					{t('harness.request')}
				</blockquote>
				<ol className="flow-steps">
					{FLOW.map((art) => (
						<li key={art} className="flow-step">
							<FlowArt art={art} />
							<h3>{t(`harness.flow.${art}.title`)}</h3>
							<p>{t(`harness.flow.${art}.body`)}</p>
						</li>
					))}
				</ol>
				<figcaption className="sr-only">{t('harness.flowCaption')}</figcaption>
			</figure>

			<div className="showcase-list harness-list">
				<article className="showcase" data-reveal>
					<div className="showcase-copy">
						<h3>{t('harness.calls.title')}</h3>
						<p>{t('harness.calls.body')}</p>
						<ul className="check-list">
							{tList('harness.calls.points').map((p, i) => <li key={i}>{p}</li>)}
						</ul>
						<a className="more-link" href={docs('editor-api.md')}>
							{t('harness.calls.link')} <span aria-hidden="true">&rarr;</span>
						</a>
					</div>
					<div className="agent-log" role="table" aria-label={t('harness.calls.logLabel')}>
						<div className="agent-log-bar" aria-hidden="true">
							<span /><span /><span />
							<strong>Agent Log</strong>
						</div>
						{CALLS.map((c, i) => (
							<div key={c.tool} className="agent-log-row" role="row" style={{ animationDelay: `${i * 0.12}s` }}>
								<span role="cell" className="agent-log-tool">{c.tool}</span>
								<span role="cell" className="agent-log-args">{c.args}</span>
								<span role="cell" className={`agent-log-result${c.fail ? ' fail' : ''}`}>{c.result}</span>
							</div>
						))}
					</div>
				</article>

				<article className="showcase flip" data-reveal>
					<div className="showcase-copy">
						<h3>{t('harness.ask.title')}</h3>
						<p>{t('harness.ask.body')}</p>
						<ul className="check-list">
							{tList('harness.ask.points').map((p, i) => <li key={i}>{p}</li>)}
						</ul>
					</div>
					<div className="ask-card-stage">
						{/* The app's card, in the app's English. */}
						<div className="ask-card" aria-label={t('harness.ask.cardLabel')}>
							<strong className="ask-card-title">Allow Amp to commit "Round order totals to the cent"?</strong>
							<p className="ask-card-why">Amp is waiting in the console for your answer.</p>
							<p className="ask-card-note">Allow for This Session lets it use <code>commitChanges</code> again without asking.</p>
							<div className="ask-card-buttons" aria-hidden="true">
								<span className="ask-btn primary">Allow</span>
								<span className="ask-btn">Allow for This Session</span>
								<span className="ask-btn">Don't Allow</span>
							</div>
						</div>
					</div>
				</article>
			</div>

			<div className="tools-head" data-reveal>
				<h3>{t('harness.tools.title')}</h3>
				<p>{t('harness.tools.sub')}</p>
			</div>
			<ul className="tool-grid">
				{DOMAINS.map((d) => (
					<li key={d.id} className="tool-card" data-reveal>
						<div className="tool-card-head">
							<h4>{t(`harness.domains.${d.id}.name`)}</h4>
							<span className="tool-count">{d.count}</span>
						</div>
						<p>{t(`harness.domains.${d.id}.body`)}</p>
						<div className="tool-names">
							{d.tools.map((t) => <code key={t}>{t}</code>)}
						</div>
					</li>
				))}
			</ul>

			<div className="tokens" data-reveal>
				<div className="tokens-copy">
					<h3>{t('harness.tokens.title')}</h3>
					<p>{t('harness.tokens.body', { manual: (2300).toLocaleString(locale) })}</p>
				</div>
				<figure className="tokens-chart">
					<figcaption>{t('harness.tokens.caption')}</figcaption>
					<div className="token-bar" title={t('harness.tokens.mcpTitle', { n: mcp })}>
						<span className="token-label">{t('harness.tokens.mcp')}</span>
						<span className="token-track"><span className="token-fill mcp" style={{ width: '100%' }} /></span>
						<span className="token-value">{t('harness.tokens.about', { n: mcp })}</span>
					</div>
					<div className="token-bar" title={t('harness.tokens.skillTitle', { n: skill })}>
						<span className="token-label">{t('harness.tokens.skill')}</span>
						<span className="token-track"><span className="token-fill skill" style={{ width: `${(SKILL_TOKENS / MCP_TOKENS) * 100}%` }} /></span>
						<span className="token-value">{t('harness.tokens.about', { n: skill })}</span>
					</div>
				</figure>
			</div>

			<ul className="places">
				{PLACES.map((art) => (
					<li key={art} className="place" data-reveal>
						<div className="card-art"><CardArt art={art} /></div>
						<h4>{t(`harness.places.${art}.name`)}</h4>
						<p>{t(`harness.places.${art}.body`)}</p>
					</li>
				))}
			</ul>
		</section>
	)
}

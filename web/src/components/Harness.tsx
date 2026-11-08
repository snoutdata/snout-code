// The homepage's section on the API for agents (docs/editor-api.md): what one sentence turns into,
// the calls the agent makes, the card that asks before a change, the tools by domain, what it
// costs in tokens, and the machines it works on. Every tool name and number here is the app's;
// keep them in step with docs/editor-api.md.

import { CardArt, FlowArt, type CardArtKind, type FlowArtKind } from './Illustrations'
import { SectionHead } from './Sections'
import { docs } from '../site'

const FLOW: { art: FlowArtKind, title: string, body: string }[] = [
	{ art: 'agent', title: 'Your agent', body: 'Claude Code, Codex, opencode or Amp, in the console.' },
	{ art: 'skill', title: 'An Agent Skill', body: 'One line in its context. It reads the rest only when it needs the window.' },
	{ art: 'api', title: 'snout-api', body: 'A UTCP manual and plain HTTP on a local port, with a token.' },
	{ art: 'window', title: 'SnoutCode', body: 'Does it in front of you, in the windows you already use.' }
]

// The calls behind the sentence above, as the Agent Log lists them.
const CALLS: { tool: string, args: string, result: string, fail?: boolean }[] = [
	{ tool: 'switchProject', args: '~/projects/shop', result: 'opened' },
	{ tool: 'listRunConfigurations', args: '', result: 'Application, All tests' },
	{ tool: 'startRun', args: 'All tests', result: 'started, run 2' },
	{ tool: 'getTestResults', args: '2', result: '41 passed, 2 failed', fail: true },
	{ tool: 'openFile', args: 'OrderTotalTest.java:38', result: 'shown' },
	{ tool: 'showInApp', args: 'What failed, and why', result: 'in Agent View' }
]

const DOMAINS: { name: string, count: number, body: string, tools: string[] }[] = [
	{ name: 'Projects', count: 4, body: 'Open the project it just made, switch, close, start a new one.', tools: ['switchProject', 'showNewProject'] },
	{ name: 'Runs and tests', count: 10, body: 'Start and stop runs, read their output, see which tests failed and what the Playwright trace saw.', tools: ['startRun', 'getTestResults', 'getPlaywrightTrace'] },
	{ name: 'Code', count: 7, body: 'Ask the Java and TypeScript engines: symbols, definitions, references, hierarchies, problems, renames.', tools: ['findReferences', 'renameSymbol'] },
	{ name: 'Editor', count: 8, body: 'Open a file at a line, read your selection and the open tabs, save.', tools: ['openFile', 'getCurrentSelection'] },
	{ name: 'Git', count: 11, body: 'Show diffs and commits, stage, commit, push, and write the message of the commit you draft.', tools: ['showDiff', 'commitChanges'] },
	{ name: 'Build', count: 3, body: 'Gradle tasks and Maven goals, run in the Build window.', tools: ['runBuildTask'] },
	{ name: 'Spring', count: 2, body: 'The beans and the HTTP endpoints, from the Spring engine.', tools: ['listBeans', 'listEndpoints'] },
	{ name: 'Agent changes', count: 3, body: 'Review its own edits: keep them or put the file back.', tools: ['listAgentChanges'] },
	{ name: 'The app', count: 9, body: 'Windows, notifications, settings, shortcuts, processes, and Agent View for what no tool covers.', tools: ['showToolWindow', 'showInApp'] }
]

// Measured on what SnoutCode sends (docs/editor-api.md, "Why not MCP: the tokens").
const MCP_TOKENS = 5800
const SKILL_TOKENS = 140

const PLACES: { art: CardArtKind, name: string, body: string }[] = [
	{ art: 'here', name: 'This computer', body: 'macOS, Windows or Linux: the agent runs beside the window.' },
	{ art: 'ssh', name: 'Remote SSH', body: 'The agent runs on the server with the project, and reaches your window through the connection.' },
	{ art: 'wsl', name: 'WSL', body: 'The window stays on Windows; the agent, the skill and snout-api run inside the Linux distribution.' }
]

export function HarnessSection() {
	return (
		<section className="section" id="harness" aria-labelledby="harness-title">
			<SectionHead id="harness-title" art="harness" eyebrow="The API for agents" title="Your agent drives the whole window">
				An agent in SnoutCode can do more than write files. Ask for something you would click through yourself, and it
				does it where you can see it: it opens the project it made, runs it, reads what failed and shows you.
			</SectionHead>

			<figure className="flow" data-reveal>
				<blockquote className="flow-ask">
					<span className="flow-ask-who">You</span>
					Make me a Spring project, open it, run its tests and show me what failed.
				</blockquote>
				<ol className="flow-steps">
					{FLOW.map((s) => (
						<li key={s.art} className="flow-step">
							<FlowArt art={s.art} />
							<h3>{s.title}</h3>
							<p>{s.body}</p>
						</li>
					))}
				</ol>
				<figcaption className="sr-only">How a request reaches SnoutCode: the agent, an Agent Skill, snout-api, then the window.</figcaption>
			</figure>

			<div className="showcase-list harness-list">
				<article className="showcase" data-reveal>
					<div className="showcase-copy">
						<h3>Every call, where you can see it</h3>
						<p>
							The agent writes the project with its own tools, then works the window through the API. Each call lands
							in the window as it happens, and the Agent Log, in the View menu, keeps the list: what it asked, with what, and how it went.
						</p>
						<ul className="check-list">
							<li>Runs, test results and Playwright traces, read where they ran</li>
							<li>The file and line of each failure, opened in the editor</li>
							<li>A report in Agent View, with Copy, Insert and Run in Terminal on its code</li>
						</ul>
						<a className="more-link" href={docs('editor-api.md')}>
							The API for agents <span aria-hidden="true">&rarr;</span>
						</a>
					</div>
					<div className="agent-log" role="table" aria-label="The calls an agent made, as the Agent Log lists them">
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
						<h3>Changes ask first</h3>
						<p>
							Reading and showing never ask. Anything that changes your project, from a rename to a push, waits on a
							card for your answer, and the agent hears what you chose.
						</p>
						<ul className="check-list">
							<li>Allow once, or for the rest of the session</li>
							<li>Don't Allow tells the agent, so it can change course</li>
							<li>Nothing lets an agent start a program of its choosing</li>
						</ul>
					</div>
					<div className="ask-card-stage">
						<div className="ask-card" aria-label="A change waiting for your answer">
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
				<h3>57 tools, by what they work on</h3>
				<p>The same tools for every agent, for its session only. Nothing is written into your project or your settings.</p>
			</div>
			<ul className="tool-grid">
				{DOMAINS.map((d) => (
					<li key={d.name} className="tool-card" data-reveal>
						<div className="tool-card-head">
							<h4>{d.name}</h4>
							<span className="tool-count">{d.count}</span>
						</div>
						<p>{d.body}</p>
						<div className="tool-names">
							{d.tools.map((t) => <code key={t}>{t}</code>)}
						</div>
					</li>
				))}
			</ul>

			<div className="tokens" data-reveal>
				<div className="tokens-copy">
					<h3>An API, not a token bill</h3>
					<p>
						MCP lists every tool with its schema in each request the agent makes, used or not. A skill shows one line;
						the manual of all 57 tools, about 2,300 tokens, is read only when the agent wants more than to open a file.
						Codex alone gets one MCP tool, since its sandbox keeps commands off the port.
					</p>
				</div>
				<figure className="tokens-chart">
					<figcaption>Tokens in every request of a session</figcaption>
					<div className="token-bar" title={`The 57 tools as MCP definitions: about ${MCP_TOKENS.toLocaleString('en-US')} tokens`}>
						<span className="token-label">The 57 tools as MCP definitions</span>
						<span className="token-track"><span className="token-fill mcp" style={{ width: '100%' }} /></span>
						<span className="token-value">about {MCP_TOKENS.toLocaleString('en-US')}</span>
					</div>
					<div className="token-bar" title={`SnoutCode's skill: about ${SKILL_TOKENS} tokens`}>
						<span className="token-label">SnoutCode's skill</span>
						<span className="token-track"><span className="token-fill skill" style={{ width: `${(SKILL_TOKENS / MCP_TOKENS) * 100}%` }} /></span>
						<span className="token-value">about {SKILL_TOKENS}</span>
					</div>
				</figure>
			</div>

			<ul className="places">
				{PLACES.map((p) => (
					<li key={p.name} className="place" data-reveal>
						<div className="card-art"><CardArt art={p.art} /></div>
						<h4>{p.name}</h4>
						<p>{p.body}</p>
					</li>
				))}
			</ul>
		</section>
	)
}

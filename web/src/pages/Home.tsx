import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { GitHubMark, Glyph } from '../components/Glyph'
import { CardArt, HeroMark, PerchArt, SectionArt, WorkshopScene, type CardArtKind } from '../components/Illustrations'
import { SectionHead, Showcase } from '../components/Sections'
import { Shot } from '../components/Shot'
import { useScrollReveal } from '../effects'
import { CONTRIBUTING_URL, GITHUB_URL, ISSUES_URL, docs } from '../site'

const MADE_FOR = ['Java', 'Gradle', 'Maven', 'Spring Boot', 'TypeScript', 'JavaScript', 'Node', 'Playwright', 'Vitest', 'Jest', 'Git']

// The agents the console runs, each with the mark the app's gutter gives its lines, in its app
// colour (styles.css, --agent-*), and how SnoutCode hears its edits (docs/agents.md).
const AGENTS: { id: string, name: string, mark: string, how: string }[] = [
	{ id: 'claude', name: 'Claude Code', mark: '\u2733', how: 'Connects as its IDE, and its hooks report each edit.' },
	{ id: 'codex', name: 'Codex', mark: '\u2B22', how: 'Hooks for the session report each edit.' },
	{ id: 'opencode', name: 'opencode', mark: '\u25A0', how: 'A plugin loaded for the session reports each edit.' },
	{ id: 'amp', name: 'Amp', mark: '\u25B2', how: 'A plugin of SnoutCode\'s reports each edit, shell writes too.' },
	{ id: 'custom', name: 'Your own', mark: '\u25C6', how: 'Any command line agent: what changes during its turn is put down to it.' }
]

const MORE: { art: CardArtKind, title: string, body: string, href: string }[] = [
	{
		art: 'search',
		title: 'Find anything',
		body: 'Search Everywhere, Go to File and Go to Class, Find and Replace in Files and a command palette with every action in it.',
		href: docs('editor.md')
	},
	{
		art: 'terminal',
		title: 'A real terminal',
		body: 'Your shell in the bottom dock, with scrollback, find and links, on the built-in terminal engine or Ghostty\'s.',
		href: docs('terminal.md')
	},
	{
		art: 'source',
		title: 'Know what runs',
		body: 'The Processes window lists everything SnoutCode runs on your machine, engines, builds and servers, and stops any of it.',
		href: docs('processes.md')
	},
	{
		art: 'settings',
		title: 'Settings as text',
		body: 'Edit settings in a form or as JSON with a schema and completion, and override any of them per project.',
		href: docs('settings-and-keymap.md')
	},
	{
		art: 'keymap',
		title: 'Keys you already know',
		body: 'An editable keymap where every action has a name, so a toolbar button is never the only way to reach something.',
		href: docs('keyboard-shortcuts.md')
	},
	{
		art: 'debug',
		title: 'Native, in Rust',
		body: 'No browser engine inside. The window is drawn on the GPU, so it opens quickly and stays quick on a big project.',
		href: docs('versions-and-performance.md')
	}
]

const PLATFORMS: { art: CardArtKind, name: string, body: string }[] = [
	{ art: 'mac', name: 'macOS', body: 'A disk image: open it and drag SnoutCode into Applications.' },
	{ art: 'linux', name: 'Linux', body: 'A .deb for Debian and Ubuntu and a package for Arch Linux, on X11 and Wayland.' },
	{ art: 'windows', name: 'Windows', body: 'An installer for Windows 10 and 11 that needs no administrator rights.' }
]

export function Home() {
	useScrollReveal()

	return (
		<div className="page page-home">
			<SiteHeader />

			<main>
				<section className="hero">
					<div className="hero-copy">
						<span className="eyebrow">Free and open source</span>
						<h1>
							<span className="hero-name"><HeroMark /></span>
							The native IDE for Java, Gradle and Spring.
						</h1>
						<p className="lede">
							Written in Rust, with its own Java engine, and TypeScript for Node projects and Playwright tests. Git is
							built in, Claude Code, Codex, opencode and Amp edit live in your editor, and the window can stay on your
							desk while the project lives on a Linux machine over SSH.
						</p>
						<div className="hero-cta">
							<a className="btn btn-lg" href="/download">
								<Glyph kind="download" />
								Download SnoutCode
							</a>
							<a className="btn btn-ghost btn-lg" href={GITHUB_URL}>
								<GitHubMark />
								View on GitHub
							</a>
						</div>
						<p className="hero-proof">macOS, Linux and Windows. MIT licensed. No account, no price.</p>
					</div>
					<div className="hero-visual">
						<PerchArt />
						<Shot
							eager
							src="spring-beans.png"
							dark="run-and-debug-debugger-dark.png"
							alt="SnoutCode on a Spring Boot web shop: the Project view, the editor and the Spring window listing the beans"
						/>
					</div>
				</section>

				<section className="made-for" aria-label="Made for">
					<span className="made-for-label">Made for</span>
					<ul>
						{MADE_FOR.map((m) => <li key={m}>{m}</li>)}
					</ul>
				</section>

				<section className="section" id="features" aria-labelledby="features-title">
					<SectionHead id="features-title" art="java" eyebrow="The language layer" title="An IDE that knows your build">
						SnoutCode reads a Gradle or Maven build the way the build sees itself, modules, source sets, dependencies
						and all, and gives the editor everything that follows from it.
					</SectionHead>
					<div className="showcase-list">
						<Showcase
							title="Java, all the way down"
							body="Completion, navigation and refactoring from SnoutCode's Java engine, on the classpath your build really has."
							points={[
								'Completion that adds the import, and go to definition into libraries',
								'Find references, rename, quick fixes and organize imports',
								'Extract method, variable and constant',
								'Call and type hierarchy, parameter hints, problems as you type'
							]}
							link={{ href: docs('java.md'), label: 'Java in the user guide' }}
							shot={{ src: 'java-completion.png', alt: 'Java completion in SnoutCode, suggesting members with their types' }}
						/>
						<SectionArtRow />
						<Showcase
							flip
							title="Gradle and Maven, as they are"
							body="Open a folder with one build or many side by side. The Project view shows the modules as the build sees them."
							points={[
								'Multi-project and composite builds, buildSrc, the Kotlin DSL, version catalogs',
								'Maven reactors, parent POMs, BOM imports and profiles, with no Maven install',
								'A JDK that suits each build, offered as a download when none is installed',
								'Gradle 7.0 to 9.x, Maven 3.6 to 3.9, JDK 17, 21 and 25'
							]}
							link={{ href: docs('gradle.md'), label: 'Gradle in the user guide' }}
							shot={{ src: 'gradle-window.png', alt: 'The Gradle window listing a build\'s projects and their tasks' }}
						/>
						<Showcase
							title="Spring, as Spring sees it"
							body="Beans, where each one is injected, and the endpoints of a Spring Boot application, all in one window."
							points={[
								'Every bean, and a gutter icon at each place it is injected',
								'Endpoints by path and method, a click from their handler',
								'Active profiles, evaluated by the engine',
								'Completion and checks in application.properties and application.yml'
							]}
							link={{ href: docs('spring.md'), label: 'Spring in the user guide' }}
							shot={{ src: 'spring-injections.png', alt: 'SnoutCode showing where a Spring bean is injected' }}
						/>
						<Showcase
							flip
							title="Run, test and debug"
							body="Run configurations and gutter run icons, tests in a tree with their results, and a debugger that shows values inline."
							points={[
								'Breakpoints, stepping, variables and watches',
								'Evaluate any expression while stopped',
								'Rerun the failed tests, from the gutter or the Tests window',
								'The same debugger for Java, TypeScript and Playwright specs'
							]}
							link={{ href: docs('run-and-debug.md'), label: 'Run and debug in the user guide' }}
							shot={{ src: 'run-and-debug-debugger.png', dark: 'run-and-debug-debugger-dark.png', alt: 'The debugger stopped at a breakpoint with values inline' }}
						/>
					</div>
				</section>

				<section className="section section-tint" id="playwright" aria-labelledby="pw-title">
					<SectionHead id="pw-title" art="playwright" eyebrow="Playwright for QA" title="End to end tests, as a project of their own" flip>
						A repository of Playwright tests opens as its own kind of project and tests the application from the
						outside, through its URL. It gets TypeScript editing and the Node, packages and browsers it needs, and
						never starts Java.
					</SectionHead>
					<div className="showcase-list">
						<Showcase
							title="Write tests by using the app"
							body="Act in the browser and the steps are written into your spec as you go, right where the caret is."
							points={[
								'Record at Cursor: into the test the caret is in, as a new test, or as a whole spec',
								'Pick Locator: each element you click puts its locator at the caret',
								'Page addresses written relative to the app, so a test runs against any environment',
								'Playwright\'s UI Mode and Inspector a click away'
							]}
							link={{ href: docs('playwright.md#writing-tests'), label: 'Writing tests in the user guide' }}
							shot={{ src: 'playwright-recording.png', alt: 'Record at Cursor writing the recorder\'s steps into a Playwright test', w: 1600, h: 514 }}
						/>
						<Showcase
							flip
							title="Follow every run, test by test"
							body="Run all, a project, a file or the test at the caret, and watch the Tests window fill in as the run goes."
							points={[
								'Progress, time left, each worker\'s step, a grid of tests by browser',
								'The last ten results of every test and its flaky rate',
								'A failure\'s error at its line, screenshot comparisons with Accept, steps and a trace filmstrip',
								'Rerun failed, from the Tests window, the title bar or the gutter'
							]}
							link={{ href: docs('playwright.md#running-tests'), label: 'Running tests in the user guide' }}
							shot={{ src: 'playwright-tests-window.png', alt: 'The Tests window following a Playwright run in three browsers', w: 1600, h: 891 }}
						/>
						<Showcase
							title="Debug a test where it stops"
							body="Set a breakpoint in a spec, a page object or a helper. The test stops there with its browser showing."
							points={[
								'Breakpoints in TypeScript or JavaScript',
								'The worker\'s stack, variables and watches in the Debug window',
								'Stepping as for Java, with the same keys',
								'The debugger is downloaded the first time, nothing to install'
							]}
							link={{ href: docs('playwright.md#debugging-tests'), label: 'Debugging tests in the user guide' }}
							shot={{ src: 'playwright-debugging.png', alt: 'A Playwright test stopped at a breakpoint in its spec, with the stack and variables', w: 1600, h: 812 }}
						/>
					</div>
					<div className="trio">
						<article className="trio-card" data-reveal>
							<h3>Ready to run</h3>
							<p>A bar installs the packages with npm, pnpm, Yarn or Bun, by the lockfile, and the browsers its Playwright version runs. The output shows in the Build window.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>Environments</h3>
							<p>Your machine, staging, a review app: each environment sets BASE_URL, its variables and a .env file. They stay outside the repository, so each tester has their own.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>History</h3>
							<p>Every run is kept, the latest 200 of each project, with its environment, branch and commit, and the tests that broke since the run before.</p>
						</article>
					</div>
				</section>

				<section className="section" id="node" aria-labelledby="node-title">
					<SectionHead id="node-title" art="node" eyebrow="Node and TypeScript" title="TypeScript, on the Node your project asks for">
						A package.json project opens as a Node project, without Java. Its scripts run and debug the way a Java
						project's do, and its editing comes from the project's own TypeScript.
					</SectionHead>
					<div className="showcase-list">
						<Showcase
							flip
							title="Every script, one click away"
							body="The scripts in package.json are run configurations, with a run icon beside each one."
							points={[
								'Workspaces and folders with their own package.json, each package\'s scripts named by its folder',
								'npm, pnpm, Yarn or Bun, by the lockfile',
								'Any TypeScript file runs as Current File, through tsx, ts-node or Node\'s own type stripping',
								'Debug follows every Node process a run starts, and attaches to a Node started with --inspect'
							]}
							link={{ href: docs('node.md'), label: 'Node projects in the user guide' }}
							shot={{ src: 'node-scripts.png', alt: 'A package.json with a run icon beside each script, and npm run dev in the Run window', w: 1600, h: 1028 }}
						/>
						<Showcase
							title="Vitest and Jest, live"
							body="Unit tests get run icons at each test, it and describe, and their runs fill the Tests window as they go."
							points={[
								'Each test passed, failed or skipped, its time, and a failure\'s message at its line',
								'Rerun failed, Run this test and Debug this test',
								'A switch between the packages that have tests',
								'Debug stops in the test and in the code it calls'
							]}
							link={{ href: docs('node.md#unit-tests-vitest-and-jest'), label: 'Vitest and Jest in the user guide' }}
							shot={{ src: 'node-vitest.png', alt: 'A Vitest run in the Tests window, a failed test picked, its test file above with run icons', w: 1600, h: 1028 }}
						/>
					</div>
					<div className="trio">
						<article className="trio-card" data-reveal>
							<h3>TypeScript editing</h3>
							<p>Completion with auto imports, hover, go to definition, find references, rename, quick fixes and problems, from the TypeScript your project installs.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>The right Node, found</h3>
							<p>From .nvmrc, .node-version, volta or engines, among the Nodes of nvm, Volta, fnm, asdf, mise and Homebrew. When none fits, it downloads one and checks it.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>Beside a Java build</h3>
							<p>A Spring Boot application with its front end, or with an e2e folder of Playwright tests, gets each language server for its own files.</p>
						</article>
					</div>
					<p className="center-link" data-reveal>
						<a className="more-link" href={docs('typescript.md')}>TypeScript and JavaScript in the user guide <span aria-hidden="true">&rarr;</span></a>
					</p>
				</section>

				<section className="section section-tint" id="agents" aria-labelledby="agents-title">
					<SectionHead id="agents-title" art="agents" eyebrow="Coding agents" title="Watch your agent type" flip>
						Claude Code, Codex, opencode, Amp or your own agent runs in a docked console. Each edit is typed in live, with the
						agent's own caret and colour, and every line it wrote stays marked.
					</SectionHead>
					<ul className="agent-roster" data-reveal aria-label="The agents SnoutCode follows">
						{AGENTS.map((a) => (
							<li key={a.id} className={`agent-${a.id}`}>
								<span className="agent-name"><span className="agent-mark" aria-hidden="true">{a.mark}</span>{a.name}</span>
								<p className="agent-how">{a.how}</p>
							</li>
						))}
					</ul>
					<div className="showcase-list">
						<Showcase
							title="Every edit, as it lands"
							body="Claude Code, Codex, opencode and Amp tell SnoutCode about each edit before and after it is made, so the editor plays it out in the agent's caret and colour."
							points={[
								'The caret and its name flag move as the text types itself in',
								'Follow the agent from file to file, or keep your own place',
								'Its lines keep its mark in the gutter and in Annotate with Agents',
								'Revert puts back exactly what the file held'
							]}
							link={{ href: docs('agents.md'), label: 'Coding agents in the user guide' }}
							shot={{ src: 'agents-presence-codex.png', dark: 'agents-presence-codex-dark.png', alt: 'Codex editing a Java file in SnoutCode, its caret and name flag on the line it is typing' }}
						/>
						<Showcase
							flip
							title="Reviewed by prompt"
							body="Every change the agents made, grouped by the prompt behind it and marked with the agent that made it, to keep or undo."
							points={[
								'Keep or undo a prompt, a file or one change at a time',
								'Accept All and Revert All for everything at once',
								'A change Claude Code asks to make waits at the top for your answer'
							]}
							link={{ href: docs('agents.md#the-agent-changes-window'), label: 'The Agent Changes window' }}
							shot={{ src: 'agents-changes.png', alt: 'The Agent Changes window: an opencode prompt with its files, and a change Claude Code asks to make' }}
						/>
					</div>
					<div className="trio">
						<article className="trio-card" data-reveal>
							<h3>Set up for the session</h3>
							<p>The console starts each agent with what it needs to report its edits; Codex asks you to trust its hooks once. Your own Codex and opencode settings are never changed.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>Ask it from the editor</h3>
							<p>Send the selection, or ask the agent to fix a problem, from the code itself. The file and lines arrive in its prompt.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>An editor API, not a token bill</h3>
							<p>The agent opens files, reads your selection and the problems through an Agent Skill, a UTCP manual and plain HTTP. About 80 tokens a request, where MCP tool definitions take about 800.</p>
						</article>
					</div>
				</section>

				<section className="section" aria-labelledby="git-title">
					<SectionHead id="git-title" art="git" eyebrow="Built in" title="Git, right where you work">
						Changes, diffs, commits, branches and history, in a Git view of its own. It runs the git you already have,
						so hooks, credentials and settings behave as they do in a terminal.
					</SectionHead>
					<div className="showcase-list">
						<Showcase
							flip
							title="From change to push"
							body="Stage by file, write the message, commit and push, without leaving the window you were typing in."
							points={[
								'Unified, side by side and compact diffs of the index and the working tree',
								'Branches, tags and a history you can search',
								'Pull, fetch and push with your normal credentials',
								'Blame in the gutter, the agents\' lines included'
							]}
							link={{ href: docs('git.md'), label: 'Git in the user guide' }}
							shot={{ src: 'git-view.png', alt: 'The Git view: changed files, a side by side diff, branches and the commit history' }}
						/>
					</div>
				</section>

				<section className="section section-tint" id="remote" aria-labelledby="remote-title">
					<SectionHead id="remote-title" art="remote" eyebrow="Remote SSH" title="Your window, their machine" flip>
						Keep the window on your Mac while the project, Git, the Java engine, builds, runs, terminals and your coding agent
						are on a Linux machine you reach with ssh.
					</SectionHead>
					<div className="showcase-list">
						<Showcase
							title="Work on a server as if it were here"
							body="SnoutCode installs its small server on the machine by itself, over the ssh you already use."
							points={[
								'Your ssh config, keys and agent, nothing new to set up',
								'When the link drops, the window reconnects by itself',
								'Builds, tests and the debugger run where the code is'
							]}
							link={{ href: docs('remote-ssh.md'), label: 'Remote SSH in the user guide' }}
							shot={{ src: 'remote-ssh-project.png', alt: 'A project open over Remote SSH, the host named in the title bar' }}
						/>
					</div>
				</section>

				<section className="section" aria-labelledby="more-title">
					<div className="section-title" data-reveal>
						<span className="eyebrow">And the rest</span>
						<h2 id="more-title">Everything a working day needs</h2>
					</div>
					<div className="card-grid">
						{MORE.map((m, i) => (
							<a className="card" href={m.href} key={m.title} data-reveal style={{ ['--i' as string]: i }}>
								<div className={`card-art tone-${i % 4}`}><CardArt art={m.art} /></div>
								<h3>{m.title}</h3>
								<p>{m.body}</p>
							</a>
						))}
					</div>
				</section>

				<section className="section" id="open-source" aria-labelledby="open-title">
					<div className="open-band" data-reveal>
						<div className="open-copy">
							<span className="eyebrow">Open source</span>
							<h2 id="open-title">Free, open, and yours to change</h2>
							<p>
								SnoutCode is MIT licensed, all of it: the app, the editor and the Java engine. There is no account, no
								price and no paid tier. Read the code, report what breaks, send a pull request.
							</p>
							<pre className="code-block"><code><span className="code-prompt">$</span> git clone {GITHUB_URL}.git{'\n'}<span className="code-prompt">$</span> cd snout-code && ./snout.sh</code></pre>
							<div className="open-links">
								<a className="btn" href={GITHUB_URL}><Glyph kind="star" />Star on GitHub</a>
								<a className="btn btn-ghost" href={ISSUES_URL}>Issues</a>
								<a className="btn btn-ghost" href={CONTRIBUTING_URL}>Contributing</a>
							</div>
						</div>
						<div className="open-art"><SectionArt art="open" /></div>
					</div>
				</section>

				<section className="section" aria-labelledby="platforms-title">
					<div className="section-title" data-reveal>
						<span className="eyebrow">Everywhere you code</span>
						<h2 id="platforms-title">macOS, Linux and Windows</h2>
						<p className="section-sub">Each one carries the Java engine and its own Java runtime, so there is nothing else to install first.</p>
					</div>
					<div className="platform-grid">
						{PLATFORMS.map((p, i) => (
							<a className="card platform" href="/download" key={p.name} data-reveal style={{ ['--i' as string]: i }}>
								<div className={`card-art tone-${(i + 1) % 4}`}><CardArt art={p.art} /></div>
								<h3>{p.name}</h3>
								<p>{p.body}</p>
								<span className="more-link">Download <span aria-hidden="true">&rarr;</span></span>
							</a>
						))}
					</div>
				</section>

				<section className="cta-band" data-reveal>
					<h2>Open your project in SnoutCode</h2>
					<p>Free for everyone, on every desktop.</p>
					<div className="hero-cta">
						<a className="btn btn-lg" href="/download"><Glyph kind="download" />Download SnoutCode</a>
						<a className="btn btn-ghost btn-lg" href={docs('getting-started.md')}><Glyph kind="book" />Getting started</a>
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

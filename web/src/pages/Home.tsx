import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { GitHubMark, Glyph } from '../components/Glyph'
import { CardArt, HeroMark, PerchArt, SectionArt, WorkshopScene, type CardArtKind } from '../components/Illustrations'
import { SectionHead, Showcase } from '../components/Sections'
import { Shot } from '../components/Shot'
import { useScrollReveal } from '../effects'
import { CONTRIBUTING_URL, GITHUB_URL, ISSUES_URL, docs } from '../site'

const MADE_FOR = ['Java', 'Gradle', 'Maven', 'Spring Boot', 'TypeScript', 'Node', 'Playwright', 'Git']

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
		art: 'node',
		title: 'Node, Vitest and Jest',
		body: 'Every package script is a run configuration. Tests get run icons at each test, a live Tests window and a debugger.',
		href: docs('node.md')
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
							Written in Rust, with its own Java engine. Git and Claude Code are built in, and the window can stay on
							your desk while the project lives on a Linux machine over SSH.
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

				<section className="section section-tint" id="agents" aria-labelledby="agents-title">
					<SectionHead id="agents-title" art="agents" eyebrow="Coding agents" title="Watch your agent type" flip>
						Claude Code, Codex or your own agent runs in a docked console. Each edit is typed in live, with the
						agent's own caret and colour, and every line it wrote stays marked.
					</SectionHead>
					<div className="agents-shot" data-reveal>
						<Shot src="agents-presence.png" dark="agents-presence-dark.png" alt="Claude Code editing a Java file in SnoutCode, its caret and name tag on the line it is typing" />
					</div>
					<div className="trio">
						<article className="trio-card" data-reveal>
							<h3>Live, in your editor</h3>
							<p>Follow the agent from file to file as it works, or keep your own place. Its lines carry its colour in the gutter and in blame.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>Reviewed by prompt</h3>
							<p>Every change the agent made, with the prompt behind it, to review, keep or undo: by prompt, by file or one change at a time.</p>
						</article>
						<article className="trio-card" data-reveal>
							<h3>An editor API, not a token bill</h3>
							<p>The agent opens files, reads your selection and the problems through an Agent Skill, a UTCP manual and plain HTTP. About 80 tokens a request, where MCP tool definitions take about 800.</p>
						</article>
					</div>
					<p className="center-link" data-reveal>
						<a className="more-link" href={docs('agents.md')}>Coding agents in the user guide <span aria-hidden="true">&rarr;</span></a>
					</p>
				</section>

				<section className="section" aria-labelledby="ts-title">
					<SectionHead id="ts-title" art="tests" eyebrow="Not only Java" title="TypeScript, Node and Playwright too">
						A package.json project opens as a Node project, and a repository of Playwright tests as a project of its
						own. Both get TypeScript editing and the Node they need, and never start Java.
					</SectionHead>
					<div className="showcase-list">
						<Showcase
							title="Playwright tests, end to end"
							body="Record at the cursor, pick locators, run any test in any browser and follow the run live in the Tests window."
							points={[
								'Progress, each worker\'s step, and a grid of tests by browser',
								'The last ten results of every test and its flaky rate',
								'Screenshot comparisons with Accept, steps, and a trace filmstrip',
								'Environments that set BASE_URL and variables per run'
							]}
							link={{ href: docs('playwright.md'), label: 'Playwright in the user guide' }}
							shot={{ src: 'playwright-tests-window.png', alt: 'The Tests window following a Playwright run in three browsers', w: 1600, h: 891 }}
						/>
					</div>
				</section>

				<section className="section section-tint" aria-labelledby="git-title">
					<SectionHead id="git-title" art="git" eyebrow="Built in" title="Git, right where you work" flip>
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

				<section className="section" id="remote" aria-labelledby="remote-title">
					<SectionHead id="remote-title" art="remote" eyebrow="Remote SSH" title="Your window, their machine">
						Keep the window on your Mac while the project, Git, the Java engine, builds, runs, terminals and Claude Code
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

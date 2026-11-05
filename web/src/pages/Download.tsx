import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { GitHubMark, Glyph } from '../components/Glyph'
import { CardArt, PageMark, ParcelScene, type CardArtKind } from '../components/Illustrations'
import { useScrollReveal } from '../effects'
import { GITHUB_URL, RELEASES_URL, docs } from '../site'

const PLATFORMS: { art: CardArtKind, name: string, file: string, points: string[], guide: { href: string, label: string } }[] = [
	{
		art: 'mac',
		name: 'macOS',
		file: 'SnoutCode .dmg',
		points: ['Open the disk image and drag SnoutCode into Applications', 'Apple silicon'],
		guide: { href: docs('getting-started.md'), label: 'Getting started' }
	},
	{
		art: 'linux',
		name: 'Linux',
		file: '.deb and Arch Linux package',
		points: ['Debian, Ubuntu and Arch Linux', 'x86_64 and aarch64, on X11 and Wayland'],
		guide: { href: docs('linux.md'), label: 'SnoutCode on Linux' }
	},
	{
		art: 'windows',
		name: 'Windows',
		file: 'Installer (.exe)',
		points: ['Windows 10 and 11, x64 and arm64', 'Installs for you, with no administrator rights', 'Not signed yet, so SmartScreen asks the first time'],
		guide: { href: docs('windows.md'), label: 'SnoutCode on Windows' }
	}
]

export function Download() {
	useScrollReveal()

	return (
		<div className="page page-download">
			<SiteHeader active="/download" />

			<main>
				<section className="page-intro">
					<h1><PageMark art="download" text="Download SnoutCode" /></h1>
					<p className="lede">
						Free for everyone, on macOS, Linux and Windows. Every build is published on the project's GitHub
						releases, next to the source it was built from.
					</p>
				</section>

				<section className="section download-grid" aria-label="Downloads">
					{PLATFORMS.map((p, i) => (
						<article className="card download-card" key={p.name} data-reveal style={{ ['--i' as string]: i }}>
							<div className={`card-art tone-${(i + 1) % 4}`}><CardArt art={p.art} /></div>
							<h2>{p.name}</h2>
							<p className="download-file">{p.file}</p>
							<ul className="check-list">
								{p.points.map((pt) => <li key={pt}>{pt}</li>)}
							</ul>
							<a className="btn btn-block" href={RELEASES_URL}>
								<Glyph kind="download" />
								Download for {p.name}
							</a>
							<a className="more-link" href={p.guide.href}>{p.guide.label} <span aria-hidden="true">&rarr;</span></a>
						</article>
					))}
				</section>

				<section className="section need" aria-labelledby="need-title">
					<div className="need-grid">
						<div data-reveal>
							<span className="eyebrow">What you need</span>
							<h2 id="need-title">Nothing else, to start</h2>
							<ul className="check-list">
								<li>The app carries its Java engine with its own Java runtime, so no Java install is needed.</li>
								<li>A build without a suitable JDK gets one offered as a download.</li>
								<li>Maven builds need no Maven install.</li>
								<li>The TypeScript server installs itself on first use, with the Node the project asks for.</li>
								<li>Git on your PATH, for the Git view. A coding agent (Claude Code, Codex, opencode or Amp) is optional, for the console.</li>
							</ul>
						</div>
						<div className="source-card" data-reveal>
							<div className="card-art tone-2"><CardArt art="source" /></div>
							<h2>Build it from source</h2>
							<p>You need Rust 1.95 or newer, Git and a JDK 21 (the script downloads one when it finds none).</p>
							<pre className="code-block"><code><span className="code-prompt">$</span> git clone {GITHUB_URL}.git{'\n'}<span className="code-prompt">$</span> cd snout-code{'\n'}<span className="code-prompt">$</span> ./snout.sh</code></pre>
							<div className="open-links">
								<a className="btn btn-ghost" href={GITHUB_URL}><GitHubMark />The repository</a>
								<a className="btn btn-ghost" href={`${GITHUB_URL}#building`}><Glyph kind="terminal" />Building</a>
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

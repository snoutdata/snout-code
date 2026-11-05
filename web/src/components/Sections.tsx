import type { ReactNode } from 'react'
import { SectionArt, type SectionArtKind } from './Illustrations'
import { Shot } from './Shot'

// A section heading with its drawing beside it, the drawing alternating sides down the page
// (`flip`) and stacked above the text on a phone.
export function SectionHead({ id, art, eyebrow, title, children, flip = false }: { id?: string, art: SectionArtKind, eyebrow: string, title: string, children?: ReactNode, flip?: boolean }) {
	return (
		<div className={`section-head${flip ? ' flip' : ''}`} data-reveal>
			<div className="section-head-copy">
				<span className="eyebrow">{eyebrow}</span>
				<h2 id={id}>{title}</h2>
				{children && <p className="section-sub">{children}</p>}
			</div>
			<div className="section-head-art"><SectionArt art={art} /></div>
		</div>
	)
}

export type ShotSpec = { src: string, dark?: string, alt: string, w?: number, h?: number }

// One showcase row: what a feature does, as a list, beside the real app doing it.
export function Showcase({ title, body, points, link, shot, flip = false }: { title: string, body: string, points: string[], link?: { href: string, label: string }, shot: ShotSpec, flip?: boolean }) {
	return (
		<article className={`showcase${flip ? ' flip' : ''}`} data-reveal>
			<div className="showcase-copy">
				<h3>{title}</h3>
				<p>{body}</p>
				<ul className="check-list">
					{points.map((p) => <li key={p}>{p}</li>)}
				</ul>
				{link && (
					<a className="more-link" href={link.href}>
						{link.label} <span aria-hidden="true">&rarr;</span>
					</a>
				)}
			</div>
			<Shot {...shot} />
		</article>
	)
}

import type { ReactNode } from 'react'
import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { useI18n, withSlots } from '../i18n'
import { GITHUB_URL, LICENSE_URL, NOTICE_URL, docs } from '../site'

const CONTACT_EMAIL = 'support@snoutdata.com'

// The words of each page are in the catalogs, under legal.<page>.sections.<id>: a title, then
// optional paragraphs (body), bullets (points) and closing paragraphs (after).
const SECTIONS = {
	privacy: [
		'short',
		'local',
		'telemetry',
		'updates',
		'downloads',
		'dependencies',
		'spring',
		'git',
		'agents',
		'remote',
		'website',
		'messages',
		'choices',
		'changes',
		'contact'
	],
	terms: ['software', 'warranty', 'site', 'thirdParty', 'updates', 'changes', 'law', 'contact']
} as const

type Doc = keyof typeof SECTIONS

export function Privacy() {
	return <LegalPage doc="privacy" />
}

export function Terms() {
	return <LegalPage doc="terms" />
}

function LegalPage({ doc }: { doc: Doc }) {
	const { t, tList } = useI18n()

	// {name} in a sentence becomes one of these links; `text` in backticks becomes code.
	const slots: Record<string, ReactNode> = {
		email: <a className="inline-link" href={`mailto:${CONTACT_EMAIL}`}>{CONTACT_EMAIL}</a>,
		license: <a className="inline-link" href={LICENSE_URL}>{t('legal.links.license')}</a>,
		notice: <a className="inline-link" href={NOTICE_URL}>{t('legal.links.notice')}</a>,
		source: <a className="inline-link" href={GITHUB_URL}>{t('legal.links.source')}</a>,
		updates: <a className="inline-link" href={docs('updates.md')}>{t('legal.links.updates')}</a>,
		privacy: <a className="inline-link" href="/privacy">{t('legal.privacy.title')}</a>
	}
	const rich = (text: string): ReactNode =>
		text.split(/(`[^`]+`)/).map((part, i) =>
			part.startsWith('`') ? <code key={i}>{part.slice(1, -1)}</code> : <span key={i}>{withSlots(part, slots)}</span>
		)

	return (
		<div className={`page page-legal page-${doc}`}>
			<SiteHeader />
			<main className="legal">
				<span className="eyebrow">{t('legal.eyebrow')}</span>
				<h1>{t(`legal.${doc}.title`)}</h1>
				<p className="legal-updated">{t('legal.updated')}</p>
				{tList(`legal.${doc}.intro`).map((p, i) => <p key={i}>{rich(p)}</p>)}
				{SECTIONS[doc].map((id, i) => {
					const key = `legal.${doc}.sections.${id}`
					const points = tList(`${key}.points`)
					return (
						<section key={id} aria-labelledby={`legal-${id}`}>
							<h2 id={`legal-${id}`}>{i + 1}. {t(`${key}.title`)}</h2>
							{tList(`${key}.body`).map((p, j) => <p key={j}>{rich(p)}</p>)}
							{points.length > 0 && <ul>{points.map((p, j) => <li key={j}>{rich(p)}</li>)}</ul>}
							{tList(`${key}.after`).map((p, j) => <p key={j}>{rich(p)}</p>)}
						</section>
					)
				})}
			</main>
			<SiteFooter />
		</div>
	)
}

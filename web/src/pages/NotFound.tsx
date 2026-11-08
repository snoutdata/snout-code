import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { PageMark, WorkshopScene } from '../components/Illustrations'
import { useT } from '../i18n'
import { DOCS_URL } from '../site'

export function NotFound() {
	const t = useT()
	return (
		<div className="page page-404">
			<SiteHeader />
			<main>
				<section className="page-intro">
					<h1><PageMark art="lost" text={t('notFound.title')} /></h1>
					<p className="lede">{t('notFound.lede')}</p>
					<div className="hero-cta center">
						<a className="btn btn-lg" href="/">{t('notFound.home')}</a>
						<a className="btn btn-ghost btn-lg" href={DOCS_URL}>{t('notFound.guide')}</a>
					</div>
				</section>
			</main>
			<WorkshopScene />
			<SiteFooter />
		</div>
	)
}

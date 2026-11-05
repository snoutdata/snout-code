import { SiteHeader } from '../components/SiteHeader'
import { SiteFooter } from '../components/SiteFooter'
import { PageMark, WorkshopScene } from '../components/Illustrations'
import { DOCS_URL } from '../site'

export function NotFound() {
	return (
		<div className="page page-404">
			<SiteHeader />
			<main>
				<section className="page-intro">
					<h1><PageMark art="lost" text="Page not found" /></h1>
					<p className="lede">The butterfly looked everywhere, and this page is not here.</p>
					<div className="hero-cta center">
						<a className="btn btn-lg" href="/">The homepage</a>
						<a className="btn btn-ghost btn-lg" href={DOCS_URL}>The user guide</a>
					</div>
				</section>
			</main>
			<WorkshopScene />
			<SiteFooter />
		</div>
	)
}

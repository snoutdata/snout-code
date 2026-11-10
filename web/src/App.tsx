import { Home } from './pages/Home'
import { Download } from './pages/Download'
import { NotFound } from './pages/NotFound'
import { Privacy, Terms } from './pages/Legal'
import { I18nProvider } from './i18n'
import { routeFor } from './routes'

export function App({ path }: { path: string }) {
	return <I18nProvider><Page path={path} /></I18nProvider>
}

function Page({ path }: { path: string }) {
	const route = routeFor(path)
	if (route.path === '/') {
		return <Home />
	}
	if (route.path === '/download') {
		return <Download />
	}
	if (route.path === '/privacy') {
		return <Privacy />
	}
	if (route.path === '/terms') {
		return <Terms />
	}
	return <NotFound />
}

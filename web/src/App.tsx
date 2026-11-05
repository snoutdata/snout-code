import { Home } from './pages/Home'
import { Download } from './pages/Download'
import { NotFound } from './pages/NotFound'
import { routeFor } from './routes'

export function App({ path }: { path: string }) {
	const route = routeFor(path)
	if (route.path === '/') {
		return <Home />
	}
	if (route.path === '/download') {
		return <Download />
	}
	return <NotFound />
}

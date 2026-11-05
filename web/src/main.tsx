import { StrictMode } from 'react'
import { createRoot, hydrateRoot } from 'react-dom/client'
import '@fontsource/inter/400.css'
import '@fontsource/inter/500.css'
import '@fontsource/inter/600.css'
import '@fontsource/inter/700.css'
import '@fontsource/inter/800.css'
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/500.css'
import './styles.css'
import { App } from './App'

const root = document.getElementById('root')!
const app = (
	<StrictMode>
		<App path={window.location.pathname} />
	</StrictMode>
)
// A built page arrives prerendered (scripts/prerender.mjs) and is hydrated; the dev server
// sends an empty root and renders it.
if (root.firstElementChild) {
	hydrateRoot(root, app)
} else {
	createRoot(root).render(app)
}

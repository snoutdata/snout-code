import { StrictMode } from 'react'
import { renderToString } from 'react-dom/server'
import { App } from './App'

export { ROUTES } from './routes'

export function render(path: string): string {
	return renderToString(
		<StrictMode>
			<App path={path} />
		</StrictMode>
	)
}

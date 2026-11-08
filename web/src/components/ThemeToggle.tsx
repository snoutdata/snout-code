import { useEffect, useState } from 'react'
import { useT } from '../i18n'

type Theme = 'light' | 'dark'

// Light is the default for everyone; dark is opt-in and remembered (index.html applies it
// before the first paint). The page renders the same on the server and the client, and the
// button learns the theme after hydration.
export function ThemeToggle() {
	const t = useT()
	const [theme, setTheme] = useState<Theme>('light')

	useEffect(() => {
		setTheme(document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light')
	}, [])

	function toggle() {
		const next: Theme = theme === 'dark' ? 'light' : 'dark'
		document.documentElement.dataset.theme = next
		try {
			localStorage.setItem('theme', next)
		} catch {
			// Private windows may refuse storage; the choice then lasts for this page only.
		}
		setTheme(next)
	}

	const label = t(theme === 'dark' ? 'theme.toLight' : 'theme.toDark')
	return (
		<button className="icon-btn" type="button" onClick={toggle} aria-label={label} title={label}>
			<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" className="theme-icon">
				<g className="theme-sun">
					<circle cx="12" cy="12" r="4.2" />
					<path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.3 5.3l1.6 1.6M17.1 17.1l1.6 1.6M5.3 18.7l1.6-1.6M17.1 6.9l1.6-1.6" />
				</g>
				<path className="theme-moon" d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z" />
			</svg>
		</button>
	)
}

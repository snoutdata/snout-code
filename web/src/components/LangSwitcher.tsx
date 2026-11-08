import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import { LOCALES, useI18n, type Locale } from '../i18n'

// The language switcher in the header: a globe and the language's own name, opening a list of the
// site's languages. A choice is remembered (setLocale), so the next visit opens in it. Outside
// click and Escape close the list; the arrow keys move through it. Below 620px only the globe shows.
export function LangSwitcher() {
	const { locale, setLocale, t } = useI18n()
	const [open, setOpen] = useState(false)
	const ref = useRef<HTMLDivElement>(null)
	const listRef = useRef<HTMLDivElement>(null)
	const current = LOCALES.find((l) => l.code === locale) ?? LOCALES[0]

	useEffect(() => {
		if (!open) {
			return
		}
		function onPointer(e: MouseEvent) {
			if (ref.current && !ref.current.contains(e.target as Node)) {
				setOpen(false)
			}
		}
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				setOpen(false)
			}
		}
		document.addEventListener('mousedown', onPointer)
		document.addEventListener('keydown', onKey)
		return () => {
			document.removeEventListener('mousedown', onPointer)
			document.removeEventListener('keydown', onKey)
		}
	}, [open])

	// On open, focus lands on the current language.
	useEffect(() => {
		if (open) {
			listRef.current?.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus()
		}
	}, [open])

	function choose(code: Locale) {
		setLocale(code)
		setOpen(false)
	}

	function onListKeyDown(e: ReactKeyboardEvent<HTMLDivElement>) {
		if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') {
			return
		}
		e.preventDefault()
		const items = Array.from(listRef.current?.querySelectorAll<HTMLButtonElement>('.lang-option') ?? [])
		const idx = items.indexOf(document.activeElement as HTMLButtonElement)
		const next = e.key === 'ArrowDown' ? idx + 1 : idx - 1
		items[(next + items.length) % items.length]?.focus()
	}

	return (
		<div className="lang-switcher" ref={ref}>
			<button
				type="button"
				className="lang-trigger"
				aria-haspopup="listbox"
				aria-expanded={open}
				aria-label={t('lang.label')}
				title={t('lang.label')}
				onClick={() => setOpen((v) => !v)}
			>
				<svg className="glyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
					<circle cx="12" cy="12" r="9" />
					<path d="M3 12h18M12 3a13.5 13.5 0 0 1 3.6 9 13.5 13.5 0 0 1-3.6 9 13.5 13.5 0 0 1-3.6-9A13.5 13.5 0 0 1 12 3z" />
				</svg>
				<span className="lang-name">{current.native}</span>
			</button>
			{open && (
				<div className="lang-pop" role="listbox" aria-label={t('lang.label')} ref={listRef} onKeyDown={onListKeyDown}>
					{LOCALES.map((l) => (
						<button
							key={l.code}
							type="button"
							className="lang-option"
							role="option"
							lang={l.code}
							aria-selected={l.code === locale}
							onClick={() => choose(l.code)}
						>
							<span>{l.native}</span>
							{l.code === locale && (
								<svg className="lang-check" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
									<path d="M20 6 9 17l-5-5" />
								</svg>
							)}
						</button>
					))}
				</div>
			)}
		</div>
	)
}

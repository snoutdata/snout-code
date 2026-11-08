import { createContext, Fragment, useCallback, useContext, useEffect, useLayoutEffect, useState, type ReactNode } from 'react'
import en from './en.json'
import fr from './fr.json'
import es from './es.json'
import pt from './pt.json'

// Tiny, dependency-free i18n, the same as snoutdata.com's. Strings live in per-locale JSON
// catalogs (en is the canonical source); components read them with useT()/t("dot.path"). The
// locale is the visitor's saved choice (localStorage `lang`), else their browser language, else
// English. It is not in the URL: every page is prerendered in English, hydrated in English, and
// switched to the visitor's language before the first paint (index.html hides the page until
// then, see the `i18n-wait` class). See README.md, "Translations".
//
// Translations are context-aware, not literal: each catalog says the thing the way a developer
// in that language would, keeping product terms (Java, Gradle, Spring, Git, SSH) and the app's own
// English UI names (the Tests window, Agent Log, Allow for This Session) as they are.

export type Locale = 'en' | 'fr' | 'es' | 'pt'

// Order here is the order shown in the language switcher. `native` is the label a speaker of that
// language recognises; `label` is the English name.
export const LOCALES: { code: Locale, label: string, native: string }[] = [
	{ code: 'en', label: 'English', native: 'English' },
	{ code: 'fr', label: 'French', native: 'Français' },
	{ code: 'es', label: 'Spanish', native: 'Español' },
	{ code: 'pt', label: 'Portuguese', native: 'Português' }
]

type Catalog = Record<string, unknown>
const CATALOGS: Record<Locale, Catalog> = { en, fr, es, pt }
const STORAGE_KEY = 'lang'

const useIsoLayoutEffect = typeof window === 'undefined' ? useEffect : useLayoutEffect

function isLocale(value: string): value is Locale {
	return value === 'en' || value === 'fr' || value === 'es' || value === 'pt'
}

// The locale to show: a saved manual choice wins; otherwise the first browser language we
// support; otherwise English. index.html runs the same logic before the first paint: change both.
export function detectLocale(): Locale {
	try {
		const saved = localStorage.getItem(STORAGE_KEY)
		if (saved && isLocale(saved)) {
			return saved
		}
	} catch {
		// localStorage can be unavailable (private mode); fall through to browser detection.
	}
	const browser = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language]
	for (const tag of browser) {
		const base = (tag || '').toLowerCase().split('-')[0]
		if (isLocale(base)) {
			return base
		}
	}
	return 'en'
}

type Vars = Record<string, string | number>

// Walk a "dot.path" through a nested catalog to its leaf (string, array, object), or undefined.
function resolveNode(catalog: Catalog, key: string): unknown {
	let node: unknown = catalog
	for (const part of key.split('.')) {
		if (node && typeof node === 'object' && part in (node as Record<string, unknown>)) {
			node = (node as Record<string, unknown>)[part]
		} else {
			return undefined
		}
	}
	return node
}

// A list of strings (feature bullets), with the same English fallback as translate().
export function translateList(locale: Locale, key: string): string[] {
	let node = resolveNode(CATALOGS[locale], key)
	if (node === undefined && locale !== 'en') {
		node = resolveNode(CATALOGS.en, key)
	}
	return Array.isArray(node) ? node.filter((x): x is string => typeof x === 'string') : []
}

// A key in a locale, falling back to English, then to the key itself (a missing translation reads
// as English, never blank). {placeholders} are filled from vars.
export function translate(locale: Locale, key: string, vars?: Vars): string {
	let node = resolveNode(CATALOGS[locale], key)
	if (typeof node !== 'string' && locale !== 'en') {
		node = resolveNode(CATALOGS.en, key)
	}
	let value = typeof node === 'string' ? node : key
	if (vars) {
		for (const [name, replacement] of Object.entries(vars)) {
			value = value.replace(new RegExp('\\{' + name + '\\}', 'g'), String(replacement))
		}
	}
	return value
}

// A translated string with elements in it: each {name} in the text becomes slots[name], so a link
// sits where the language puts it ("A {link} project.").
export function withSlots(text: string, slots: Record<string, ReactNode>): ReactNode {
	return text.split(/(\{\w+\})/).map((part, i) => {
		const name = /^\{(\w+)\}$/.exec(part)?.[1]
		return <Fragment key={i}>{name && name in slots ? slots[name] : part}</Fragment>
	})
}

type I18nValue = {
	locale: Locale
	setLocale: (locale: Locale) => void
	t: (key: string, vars?: Vars) => string
	tList: (key: string) => string[]
}

const I18nContext = createContext<I18nValue | null>(null)

export function I18nProvider({ children }: { children: ReactNode }) {
	// English on the server and at hydration, so the client's first render matches the HTML.
	const [locale, setLocaleState] = useState<Locale>('en')

	// Then the visitor's language, before the browser paints, and the page is shown.
	useIsoLayoutEffect(() => {
		const detected = detectLocale()
		if (detected !== 'en') {
			setLocaleState(detected)
		}
		document.documentElement.lang = detected
		document.documentElement.classList.remove('i18n-wait')
	}, [])

	const setLocale = useCallback((next: Locale) => {
		try {
			localStorage.setItem(STORAGE_KEY, next)
		} catch {
			// The choice then lasts for this page only.
		}
		document.documentElement.lang = next
		setLocaleState(next)
	}, [])

	const t = useCallback((key: string, vars?: Vars) => translate(locale, key, vars), [locale])
	const tList = useCallback((key: string) => translateList(locale, key), [locale])

	return <I18nContext.Provider value={{ locale, setLocale, t, tList }}>{children}</I18nContext.Provider>
}

export function useI18n(): I18nValue {
	const ctx = useContext(I18nContext)
	if (!ctx) {
		throw new Error('useI18n must be used within <I18nProvider>')
	}
	return ctx
}

// The common case: just the t() function.
export function useT(): (key: string, vars?: Vars) => string {
	return useI18n().t
}

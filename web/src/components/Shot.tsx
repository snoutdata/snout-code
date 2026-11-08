import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useT } from '../i18n'

// A product shot: an app window standing on a tinted stage. With `dark`, the window follows the
// site's theme (both images are in the page; CSS shows one, and the hidden one is lazy, so it is
// never fetched). A click opens it full size in an overlay (Lightbox).
export function Shot({ src, dark, alt, w = 1600, h = 1000, eager = false, className = '' }: { src: string, dark?: string, alt: string, w?: number, h?: number, eager?: boolean, className?: string }) {
	const t = useT()
	const [open, setOpen] = useState<string | null>(null)
	const loading = eager ? 'eager' : 'lazy'

	function show() {
		const isDark = document.documentElement.dataset.theme === 'dark'
		setOpen(`/shots/${isDark && dark ? dark : src}`)
	}

	return (
		<figure className={`shot ${className}`}>
			<button className="shot-window" type="button" onClick={show} aria-label={t('shot.enlarge', { alt })}>
				<img className={dark ? 'shot-light' : undefined} src={`/shots/${src}`} alt={alt} width={w} height={h} loading={loading} decoding="async" />
				{dark && <img className="shot-dark" src={`/shots/${dark}`} alt={alt} width={w} height={h} loading="lazy" decoding="async" />}
				<span className="shot-zoom" aria-hidden="true">
					<svg viewBox="0 0 24 24"><path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" /></svg>
				</span>
			</button>
			{open && <Lightbox src={open} alt={alt} w={w} h={h} onClose={() => setOpen(null)} />}
		</figure>
	)
}

// The overlay: the screenshot at its full size (or as large as the window allows), over a
// blurred page. Escape, a click outside the image or the close button ends it, and focus goes
// back to the shot. Portalled to <body>, since a revealed section's translate would otherwise
// become the containing block of a fixed element.
function Lightbox({ src, alt, w, h, onClose }: { src: string, alt: string, w: number, h: number, onClose: () => void }) {
	const t = useT()
	const close = useRef<HTMLButtonElement>(null)

	useEffect(() => {
		const before = document.activeElement as HTMLElement | null
		const overflow = document.body.style.overflow
		document.body.style.overflow = 'hidden'
		close.current?.focus()
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				onClose()
			} else if (e.key === 'Tab') {
				// One control in the dialog: keep focus on it.
				e.preventDefault()
				close.current?.focus()
			}
		}
		document.addEventListener('keydown', onKey)
		return () => {
			document.removeEventListener('keydown', onKey)
			document.body.style.overflow = overflow
			before?.focus()
		}
	}, [onClose])

	return createPortal(
		<div className="lightbox" role="dialog" aria-modal="true" aria-label={alt} onClick={onClose}>
			<button ref={close} className="lightbox-close" type="button" aria-label={t('shot.close')} onClick={onClose}>
				<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
			</button>
			<figure className="lightbox-figure" onClick={(e) => e.stopPropagation()}>
				<img src={src} alt={alt} width={w} height={h} />
				<figcaption>{alt}</figcaption>
			</figure>
		</div>,
		document.body
	)
}

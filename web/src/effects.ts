import { useEffect, useLayoutEffect } from 'react'

// Reveal [data-reveal] elements as they scroll into view (adds .revealed once). Anything
// already on screen at mount is revealed before the first paint with no transition: the first
// screenful is the page, not an entrance. Gated by prefers-reduced-motion in styles.css.
const useIsoLayoutEffect = typeof window === 'undefined' ? useEffect : useLayoutEffect

export function useScrollReveal() {
	useIsoLayoutEffect(() => {
		document.querySelectorAll<HTMLElement>('[data-reveal]').forEach((el) => {
			if (el.getBoundingClientRect().top < window.innerHeight) {
				el.classList.add('revealed', 'reveal-instant')
			}
		})
	}, [])

	useEffect(() => {
		const els = Array.from(document.querySelectorAll<HTMLElement>('[data-reveal]')).filter(
			(el) => !el.classList.contains('revealed')
		)
		if (els.length === 0) {
			return
		}
		const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
		if (reduce || !('IntersectionObserver' in window)) {
			els.forEach((el) => el.classList.add('revealed'))
			return
		}
		const io = new IntersectionObserver(
			(entries) => {
				entries.forEach((entry) => {
					if (entry.isIntersecting) {
						entry.target.classList.add('revealed')
						io.unobserve(entry.target)
					}
				})
			},
			{ rootMargin: '0px 0px 10% 0px', threshold: 0.01 }
		)
		els.forEach((el) => io.observe(el))
		return () => io.disconnect()
	}, [])
}

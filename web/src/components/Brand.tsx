// The logo: the app icon (assets/icon, copied to /favicon.svg) beside the wordmark. The drawn
// butterfly (Illustrations.tsx) is the ART logo, for the hero and the large moments.
export function Brand({ size = 28 }: { size?: number }) {
	return (
		<a className="brand" href="/" aria-label="SnoutCode home">
			<img src="/favicon.svg" width={size} height={size} alt="" />
			<span className="brand-word">Snout<span>Code</span></span>
		</a>
	)
}

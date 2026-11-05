// A product shot: an app window standing on a tinted stage. With `dark`, the window follows the
// site's theme (both images are in the page; CSS shows one, and the hidden one is lazy, so it is
// never fetched).
export function Shot({ src, dark, alt, w = 1600, h = 1000, eager = false, className = '' }: { src: string, dark?: string, alt: string, w?: number, h?: number, eager?: boolean, className?: string }) {
	const loading = eager ? 'eager' : 'lazy'
	return (
		<figure className={`shot ${className}`}>
			<div className="shot-window">
				<img className={dark ? 'shot-light' : undefined} src={`/shots/${src}`} alt={alt} width={w} height={h} loading={loading} decoding="async" />
				{dark && <img className="shot-dark" src={`/shots/${dark}`} alt={alt} width={w} height={h} loading="lazy" decoding="async" />}
			</div>
		</figure>
	)
}

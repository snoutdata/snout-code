// Every page of the site, with what goes in its <head>. scripts/prerender.mjs writes one HTML
// file per entry; App.tsx picks the page by path.
export type Route = {
	path: string
	file: string
	title: string
	description: string
}

export const ROUTES: Route[] = [
	{
		path: '/',
		file: 'index.html',
		title: 'SnoutCode: the native IDE for Java, Gradle and Spring',
		description:
			'SnoutCode is a free, open source IDE written in Rust for Java, Gradle, Maven and Spring Boot, TypeScript on Node and Playwright tests. Git and Claude Code are built in, and it works on a Linux machine over SSH. For macOS, Linux and Windows.'
	},
	{
		path: '/download',
		file: 'download.html',
		title: 'Download SnoutCode for macOS, Linux and Windows',
		description:
			'Download SnoutCode, the free and open source IDE for Java, Gradle and Spring: a disk image for macOS, a .deb and an Arch package for Linux, and an installer for Windows. Or build it from source.'
	},
	{
		path: '/404',
		file: '404.html',
		title: 'Page not found: SnoutCode',
		description: 'This page is not on snoutcode.com.'
	}
]

export function routeFor(path: string): Route {
	const clean = path.replace(/\/+$/, '').replace(/\.html$/, '') || '/'
	return ROUTES.find((r) => r.path === clean) ?? ROUTES[ROUTES.length - 1]
}

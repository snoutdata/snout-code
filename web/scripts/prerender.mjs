// Writes every page of the site as static HTML (dist/<page>.html), from the SSR build of
// src/entry-server.tsx. The client bundle then hydrates the page. Cloudflare Pages serves
// /download from download.html, and 404.html for any path it has no file for.
import { readFileSync, rmSync, writeFileSync } from 'node:fs'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { join } from 'node:path'

const web = fileURLToPath(new URL('..', import.meta.url))
const dist = join(web, 'dist')
const ssr = join(web, 'dist-ssr')
const SITE = 'https://snoutcode.com'

const { render, ROUTES } = await import(pathToFileURL(join(ssr, 'entry-server.js')).href)
const template = readFileSync(join(dist, 'index.html'), 'utf8')

function escape(text) {
	return text.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')
}

for (const route of ROUTES) {
	const url = route.path === '/' ? `${SITE}/` : `${SITE}${route.path}`
	const head = [
		`<title>${escape(route.title)}</title>`,
		`<meta name="description" content="${escape(route.description)}" />`,
		`<meta property="og:title" content="${escape(route.title)}" />`,
		`<meta property="og:description" content="${escape(route.description)}" />`,
		route.path === '/404'
			? '<meta name="robots" content="noindex" />'
			: `<link rel="canonical" href="${url}" />\n\t\t<meta property="og:url" content="${url}" />`
	].join('\n\t\t')
	const html = template.replace('<!--head-->', head).replace('<!--app-->', render(route.path))
	writeFileSync(join(dist, route.file), html)
	console.log(`prerendered ${route.path} -> dist/${route.file}`)
}

const pages = ROUTES.filter((r) => r.path !== '/404')
const today = new Date().toISOString().slice(0, 10)
writeFileSync(
	join(dist, 'sitemap.xml'),
	`<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${pages
		.map((r) => `\t<url><loc>${SITE}${r.path === '/' ? '/' : r.path}</loc><lastmod>${today}</lastmod></url>`)
		.join('\n')}\n</urlset>\n`
)
rmSync(ssr, { recursive: true, force: true })

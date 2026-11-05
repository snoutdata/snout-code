// Renders public/og-image.png (1200x630, the link preview) from the built homepage's hero.
// Needs puppeteer, which is not a dependency (it downloads a browser):
//
//   npm run build && npm i --no-save puppeteer && npm run og
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { extname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

let puppeteer
try {
	puppeteer = (await import('puppeteer')).default
} catch {
	console.error('puppeteer is not installed: npm i --no-save puppeteer')
	process.exit(1)
}

const web = fileURLToPath(new URL('..', import.meta.url))
const dist = join(web, 'dist')
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2', '.woff': 'font/woff' }
const server = createServer(async (req, res) => {
	const path = new URL(req.url, 'http://x').pathname
	const file = path === '/' ? join(dist, 'index.html') : join(dist, path)
	if (!existsSync(file)) {
		res.writeHead(404)
		res.end()
		return
	}
	res.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' })
	res.end(await readFile(file))
}).listen(5197)

const browser = await puppeteer.launch()
const page = await browser.newPage()
await page.setViewport({ width: 1200, height: 630, deviceScaleFactor: 1 })
await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }])
await page.goto('http://localhost:5197/', { waitUntil: 'networkidle0' })
await page.addStyleTag({
	content: '.nav-wrap, .hero-cta, .hero-proof, .eyebrow { display: none !important } .hero { padding-top: 56px !important } .hero-visual { margin-top: 110px !important }'
})
await page.screenshot({ path: join(web, 'public/og-image.png') })
await browser.close()
server.close()
console.log('wrote public/og-image.png')

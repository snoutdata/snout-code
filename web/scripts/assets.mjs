// Copies what the site shows from where it already lives in this repository, so the app's
// screenshots (docs/images) and icon (assets/icon) have one source each. The copies are in
// .gitignore. Run by `npm run dev` and `npm run build`.
import { copyFileSync, mkdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const web = fileURLToPath(new URL('..', import.meta.url))
const repo = join(web, '..')
const images = join(repo, 'docs/images')
const icon = join(repo, 'assets/icon')
const pub = join(web, 'public')

// Every screenshot a page uses (src/pages/*.tsx, by file name).
const SHOTS = [
	'spring-beans.png',
	'run-and-debug-debugger-dark.png',
	'run-and-debug-debugger.png',
	'java-completion.png',
	'gradle-window.png',
	'spring-injections.png',
	'agents-presence.png',
	'agents-presence-dark.png',
	'playwright-tests-window.png',
	'playwright-recording.png',
	'playwright-debugging.png',
	'node-scripts.png',
	'node-vitest.png',
	'git-view.png',
	'remote-ssh-project.png'
]

mkdirSync(join(pub, 'shots'), { recursive: true })
for (const file of SHOTS) {
	copyFileSync(join(images, file), join(pub, 'shots', file))
}
copyFileSync(join(icon, 'snout.svg'), join(pub, 'favicon.svg'))
copyFileSync(join(icon, 'snout-512.png'), join(pub, 'apple-touch-icon.png'))
copyFileSync(join(icon, 'snout-512.png'), join(pub, 'icon-512.png'))
console.log(`assets: ${SHOTS.length} screenshots and the icon copied into public/`)

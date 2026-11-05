// Deploys dist/ to the Cloudflare Pages project `snout-code`.
//
//   CLOUDFLARE_API_TOKEN=... CLOUDFLARE_ACCOUNT_ID=... npm run deploy
//
// This repository is public, so it holds no credentials and no account details: both come from
// the environment. There is deliberately no fallback to a `wrangler login` session, because a
// session for another Cloudflare account would deploy the site into that account.
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const PROJECT = 'snout-code'
// Pinned: a deploy is not the place to find out what changed in wrangler.
const WRANGLER = 'wrangler@4.127.1'

const dist = join(fileURLToPath(new URL('..', import.meta.url)), 'dist')
const token = (process.env.CLOUDFLARE_API_TOKEN ?? '').trim()
const account = (process.env.CLOUDFLARE_ACCOUNT_ID ?? '').trim()

if (!existsSync(join(dist, 'index.html'))) {
	console.error('There is no dist/ to deploy. Run `npm run build` first.')
	process.exit(1)
}
if (!token || !account) {
	console.error('Set CLOUDFLARE_API_TOKEN (Pages: Edit) and CLOUDFLARE_ACCOUNT_ID to deploy.')
	console.error('This will not fall back to a `wrangler login` session.')
	process.exit(1)
}

const result = spawnSync(
	'npx',
	['--yes', WRANGLER, 'pages', 'deploy', dist, '--project-name', PROJECT, '--branch', 'main', '--commit-dirty=true'],
	{
		stdio: 'inherit',
		shell: process.platform === 'win32',
		env: { ...process.env, CLOUDFLARE_API_TOKEN: token, CLOUDFLARE_ACCOUNT_ID: account }
	}
)
process.exit(result.status ?? 1)

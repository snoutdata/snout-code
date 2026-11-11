import { RELEASES_API, RELEASES_URL } from './site'

// What the download page needs to send a visitor straight to their file: which system they are
// on, and where the latest release keeps each file. The system is guessed after the page loads
// (the prerendered page shows macOS first); the files come from the GitHub API at runtime, so the
// page never goes stale after a release. When either fails, every link is the latest-release
// page on GitHub, so no button is ever dead.

export type Os = 'mac' | 'windows' | 'linux'
export type Arch = 'x64' | 'arm64'

export const OSES: { id: Os, name: string }[] = [
	{ id: 'mac', name: 'macOS' },
	{ id: 'windows', name: 'Windows' },
	{ id: 'linux', name: 'Linux' }
]

export function isOs(value: string): value is Os {
	return value === 'mac' || value === 'windows' || value === 'linux'
}

// Every file a release ships, by how scripts/make-release.sh names it. What the page calls each
// is in the catalogs, under download.files.<id>.
export type FileId = 'dmg' | 'exe-x64' | 'exe-arm64' | 'deb-x64' | 'deb-arm64' | 'arch-x64' | 'arch-arm64'

export const FILES: { id: FileId, os: Os, arch: Arch, match: RegExp }[] = [
	{ id: 'dmg', os: 'mac', arch: 'arm64', match: /-arm64\.dmg$/ },
	{ id: 'exe-x64', os: 'windows', arch: 'x64', match: /-windows-x64-setup\.exe$/ },
	{ id: 'exe-arm64', os: 'windows', arch: 'arm64', match: /-windows-arm64-setup\.exe$/ },
	{ id: 'deb-x64', os: 'linux', arch: 'x64', match: /_amd64\.deb$/ },
	{ id: 'deb-arm64', os: 'linux', arch: 'arm64', match: /_arm64\.deb$/ },
	{ id: 'arch-x64', os: 'linux', arch: 'x64', match: /-x86_64\.pkg\.tar\.zst$/ },
	{ id: 'arch-arm64', os: 'linux', arch: 'arm64', match: /-aarch64\.pkg\.tar\.zst$/ }
]

export interface Visitor {
	os: Os
	arch: Arch
}

// A best guess from the user agent; anything that is neither a Mac nor Linux is taken as Windows.
// An iPhone or iPad is shown the Mac, an Android phone Linux: the files they would want elsewhere.
export function detectVisitor(): Visitor {
	const ua = (navigator.userAgent || '').toLowerCase()
	const os: Os = /mac|iphone|ipad/.test(ua) ? 'mac' : /linux|android|x11|cros/.test(ua) ? 'linux' : 'windows'
	const arch: Arch = /aarch64|arm64|armv8/.test(ua) ? 'arm64' : 'x64'
	return { os, arch }
}

// Chromium browsers say the machine's architecture when asked (Windows on Arm reports x64 in its
// user agent); others never answer, and the guess stands.
export async function refineArch(visitor: Visitor): Promise<Visitor> {
	const data = (navigator as Navigator & { userAgentData?: { getHighEntropyValues(hints: string[]): Promise<{ architecture?: string }> } }).userAgentData
	if (!data || visitor.os === 'mac') {
		return visitor
	}
	try {
		const { architecture } = await data.getHighEntropyValues(['architecture'])
		return architecture === 'arm' ? { ...visitor, arch: 'arm64' } : architecture === 'x86' ? { ...visitor, arch: 'x64' } : visitor
	} catch {
		return visitor
	}
}

export interface ReleaseFile {
	url: string
	name: string | null
	size: number | null
}

export interface Release {
	// "1.0.8", or null while it loads and when the lookup fails.
	version: string | null
	files: Record<FileId, ReleaseFile>
}

// Every file pointing at the latest-release page: what the page shows before the lookup answers,
// in the prerendered HTML, and when the lookup fails.
export function fallbackRelease(): Release {
	const files = {} as Record<FileId, ReleaseFile>
	for (const f of FILES) {
		files[f.id] = { url: RELEASES_URL, name: null, size: null }
	}
	return { version: null, files }
}

interface GhAsset {
	name: string
	size: number
	browser_download_url: string
}

// The latest release's files. Never throws: offline, rate limited or changed, it answers the fallback.
export async function fetchLatestRelease(): Promise<Release> {
	const release = fallbackRelease()
	try {
		const res = await fetch(RELEASES_API, { headers: { Accept: 'application/vnd.github+json' } })
		if (!res.ok) {
			return release
		}
		const data = (await res.json()) as { tag_name?: string, assets?: GhAsset[] }
		for (const f of FILES) {
			const asset = (data.assets ?? []).find((a) => f.match.test(a.name))
			if (asset) {
				release.files[f.id] = { url: asset.browser_download_url, name: asset.name, size: asset.size }
			}
		}
		release.version = data.tag_name ? data.tag_name.replace(/^v/, '') : null
		return release
	} catch {
		return release
	}
}

export function megabytes(size: number | null): string | null {
	return size === null ? null : `${Math.round(size / 1_000_000)} MB`
}

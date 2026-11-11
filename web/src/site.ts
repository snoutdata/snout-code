// Where the site points. One place, so a moved repository is a one-line change.
export const GITHUB_URL = 'https://github.com/snoutdata/snout-code'
export const RELEASES_URL = `${GITHUB_URL}/releases/latest`
// Windows installs from the Store, so SmartScreen never stops it; the app updates itself from there.
export const MS_STORE_URL = 'https://apps.microsoft.com/detail/xpfp2nj5xztk5s'
export const ISSUES_URL = `${GITHUB_URL}/issues`
export const DOCS_URL = `${GITHUB_URL}/blob/main/docs/README.md`
export const LICENSE_URL = `${GITHUB_URL}/blob/main/LICENSE`
export const NOTICE_URL = `${GITHUB_URL}/blob/main/NOTICE`
export const CONTRIBUTING_URL = `${GITHUB_URL}#contributing`
export const SNOUTDATA_URL = 'https://snoutdata.com'
export const SITE_URL = 'https://snoutcode.com'

export function docs(page: string): string {
	return `${GITHUB_URL}/blob/main/docs/${page}`
}

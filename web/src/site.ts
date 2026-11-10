// Where the site points. One place, so a moved repository is a one-line change.
export const GITHUB_URL = 'https://github.com/snoutdata/snout-code'
export const RELEASES_URL = `${GITHUB_URL}/releases/latest`
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

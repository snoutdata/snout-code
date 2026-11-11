import { useEffect, useState } from 'react'
import { detectVisitor, OSES } from '../download'
import { useT } from '../i18n'
import { Glyph } from './Glyph'

// The homepage's download button. Prerendered, it says "Download SnoutCode"; once the page loads,
// it names the visitor's system ("Download for macOS") and opens the download page on that system.
export function DownloadButton() {
	const t = useT()
	const [os, setOs] = useState<string | null>(null)
	useEffect(() => setOs(detectVisitor().os), [])
	const name = OSES.find((o) => o.id === os)?.name
	return (
		<a className="btn btn-lg" href={os ? `/download#${os}` : '/download'}>
			<Glyph kind="download" />
			{name ? t('download.for', { os: name }) : t('common.downloadSnoutCode')}
		</a>
	)
}

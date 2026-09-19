import { useStumpVersion } from '@stump/client'
import { cx, Link, TEXT_VARIANTS } from '@stump/components'
import { useMemo } from 'react'

export default function ApplicationVersion() {
	const version = useStumpVersion()

	// NoirPanther versions are `<Stump semver>-r<N>` (e.g. 0.1.7-r2) and releases are
	// tagged `v<semver>`; the exact build is the commit shown alongside it.
	const semver = version?.semver

	const url = useMemo(() => {
		if (!version) return undefined

		const { rev } = version
		const repoUrl = 'https://github.com/vint1024/NoirPanther'
		if (semver && /-r\d+$/.test(semver)) {
			return `${repoUrl}/releases/tag/v${semver}`
		} else if (rev) {
			return `${repoUrl}/commit/${rev}`
		} else {
			return repoUrl
		}
	}, [version, semver])

	if (!version) return null

	return (
		<Link
			href={url}
			target="__blank"
			rel="noopener noreferrer"
			className={cx('space-x-2 pl-2 flex items-center text-xxs', TEXT_VARIANTS.muted)}
			underline={false}
		>
			<span>
				v{semver}
				{!!version.rev && ` - ${version.rev}`}
			</span>
		</Link>
	)
}

import { useCheckForServerUpdate } from '@stump/client'
import { Alert, AlertDescription, Link } from '@stump/components'
import { useLocaleContext } from '@stump/i18n'
import { AlertTriangle } from 'lucide-react'
import { Suspense } from 'react'
import { Helmet } from 'react-helmet'

import { ContentContainer } from '@/components/container'
import { SceneContainer } from '@/components/container'

import HelpfulLinks from './HelpfulLinks'
import { ServerConfiguration } from './ServerConfiguration'
import ServerInfoSection from './ServerInfoSection'
import ServerStats from './ServerStats'

export default function GeneralServerSettingsScene() {
	const { t } = useLocaleContext()

	// NoirPanther: the notice names the version and links to its release notes
	const { updateAvailable, latestVersion, releaseUrl } = useCheckForServerUpdate()

	return (
		<SceneContainer>
			<Helmet>
				<title>NoirPanther | {t('settingsScene.server/general.helmet')}</title>
			</Helmet>

			<ContentContainer>
				<div className="gap-12 flex flex-col">
					<Suspense>
						<ServerStats />
					</Suspense>

					{updateAvailable && (
						<Alert variant="warning">
							<AlertTriangle />
							<AlertDescription>
								{t('settingsScene.server/general.sections.updateAvailable.message', {
									version: latestVersion,
								})}{' '}
								{releaseUrl && (
									<Link href={releaseUrl} target="_blank" rel="noopener noreferrer">
										{t('settingsScene.server/general.sections.updateAvailable.releaseNotes')}
									</Link>
								)}
							</AlertDescription>
						</Alert>
					)}

					<ServerConfiguration />

					<ServerInfoSection />

					<HelpfulLinks />
				</div>
			</ContentContainer>
		</SceneContainer>
	)
}

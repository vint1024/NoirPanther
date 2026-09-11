import { useLocaleContext } from '@stump/i18n'
import { Fullscreen, Shrink } from 'lucide-react'

import { useEpubReaderControls } from '../context'
import ControlButton from './ControlButton'

export default function FullScreenToggle() {
	const { t } = useLocaleContext()
	const { fullscreen, setFullscreen } = useEpubReaderControls()

	const Icon = fullscreen ? Shrink : Fullscreen
	return (
		<ControlButton
			title={
				fullscreen
					? t('components.readers.epub.controls.FullScreenToggle.exitFullscreen')
					: t('components.readers.epub.controls.FullScreenToggle.enterFullscreen')
			}
			onClick={() => setFullscreen(!fullscreen)}
		>
			<Icon className="h-4 w-4" />
		</ControlButton>
	)
}

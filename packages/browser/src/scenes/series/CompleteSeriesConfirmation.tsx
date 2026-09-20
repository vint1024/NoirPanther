import { Alert, AlertTitle, ConfirmationModal } from '@stump/components'
import { useLocaleContext } from '@stump/i18n'
import { AlertTriangle } from 'lucide-react'

const getKey = (key: string) => `scenes.series.CompleteSeriesConfirmation.${key}`

type Props = {
	isOpen: boolean
	onCancel: () => void
	onConfirm: () => void
}

export default function CompleteSeriesConfirmation({ isOpen, onCancel, onConfirm }: Props) {
	const { t } = useLocaleContext()

	return (
		<ConfirmationModal
			title={t(getKey('title'))}
			description={t(getKey('description'))}
			isOpen={isOpen}
			onClose={onCancel}
			onConfirm={onConfirm}
			confirmVariant="destructive"
		>
			<Alert>
				<AlertTriangle />
				<AlertTitle>{t(getKey('warning'))}</AlertTitle>
			</Alert>
		</ConfirmationModal>
	)
}

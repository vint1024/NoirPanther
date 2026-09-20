import { Sheet } from '@stump/components'
import { Preformatted } from '@stump/components'
import { useLocaleContext } from '@stump/i18n'
import { usePrevious } from 'react-use'

import { User } from './UserTable'

type Props = {
	user: User | null
	onClose: () => void
}

// TODO: do more than just json dump

export default function InspectUserSlideOver({ user, onClose }: Props) {
	const previousUser = usePrevious(user)

	const { t } = useLocaleContext()
	const displayedUser = user || previousUser

	return (
		<Sheet
			open={!!user}
			onClose={onClose}
			title={t('scenes.settings.server.users.user-table.InspectUserSlideOver.title')}
			description={t('scenes.settings.server.users.user-table.InspectUserSlideOver.description')}
		>
			<div className="px-4 gap-y-8 flex flex-col">
				<Preformatted title="JSON" content={displayedUser} />
			</div>
		</Sheet>
	)
}

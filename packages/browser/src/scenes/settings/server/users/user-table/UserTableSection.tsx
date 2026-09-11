import { ButtonOrLink, Heading, Text } from '@stump/components'
import { UserPermission } from '@stump/graphql'
import { useLocaleContext } from '@stump/i18n'
import { Suspense } from 'react'

import { useAppContext } from '@/context'

import UserTable from './UserTable'

export default function UserTableSection() {
	const { t } = useLocaleContext()
	const { checkPermission } = useAppContext()

	return (
		<div className="gap-y-4 flex flex-col">
			<div className="flex items-end justify-between">
				<div>
					<Heading size="sm">
						{t('scenes.settings.server.users.user-table.UserTableSection.heading')}
					</Heading>
					<Text size="sm" variant="muted" className="mt-1">
						{t('scenes.settings.server.users.user-table.UserTableSection.description')}
					</Text>
				</div>
				{checkPermission(UserPermission.ManageUsers) && (
					<div className="gap-2 flex items-end">
						<ButtonOrLink href="create" variant="secondary" size="sm">
							{t('scenes.settings.server.users.user-table.UserTableSection.createUser')}
						</ButtonOrLink>
					</div>
				)}
			</div>

			<Suspense>
				<UserTable />
			</Suspense>
		</div>
	)
}

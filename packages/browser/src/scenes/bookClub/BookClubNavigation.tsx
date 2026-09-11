import { cn, cx, Link } from '@stump/components'
import { useLocaleContext } from '@stump/i18n'
import { useMemo } from 'react'
import { useLocation } from 'react-router'

import { useBookClubContext } from '@/components/bookClub'
import { usePreferences } from '@/hooks'

// TODO(book-clubs): Implement
// TODO: when viewing a thread, only show something like "<-- Return to chat board"
export default function BookClubNavigation() {
	const { t } = useLocaleContext()
	const location = useLocation()
	const {
		preferences: { primaryNavigationMode, layoutMaxWidthPx },
	} = usePreferences()
	const { viewerIsMember } = useBookClubContext()

	const tabs = useMemo(() => {
		const base = [
			{
				isActive: location.pathname.match(/\/clubs\/[^/]+\/?(home)?$/),
				label: t('scenes.bookClub.BookClubNavigation.home'),
				to: '.',
			},
		]

		if (!viewerIsMember) {
			return base
		}

		// The web discussion scene is still a stub upstream (TODO(graphql)); the
		// NoirPanther mobile client carries the chat. Hide the tab until it exists.
		return [
			...base,
			{
				isActive: location.pathname.match(/\/clubs\/[^/]+\/members(\/.*)?$/),
				label: t('scenes.bookClub.BookClubNavigation.members'),
				to: 'members',
			},
			{
				isActive: location.pathname.match(/\/clubs\/[^/]+\/settings(\/.*)?$/),
				label: t('scenes.bookClub.BookClubNavigation.settings'),
				to: 'settings',
			},
		]
	}, [location, viewerIsMember, t])

	const preferTopBar = primaryNavigationMode === 'TOPBAR'

	// Don't bother rendering navigation if the user doesn't have access to any other tabs
	if (tabs.length <= 1) {
		return null
	}

	return (
		<div className="top-0 md:relative md:top-[unset] md:z-[unset] sticky z-10 w-full border-b border-border bg-background">
			<nav
				className={cn(
					'gap-x-6 px-3 md:overflow-x-hidden -mb-px scrollbar-hide flex overflow-x-scroll',
					{
						'mx-auto': preferTopBar && !!layoutMaxWidthPx,
					},
				)}
				style={{ maxWidth: preferTopBar ? layoutMaxWidthPx || undefined : undefined }}
			>
				{tabs.map((tab) => (
					<Link
						to={tab.to}
						key={tab.to}
						underline={false}
						className={cx('px-1 py-3 text-sm font-medium border-b-2 whitespace-nowrap', {
							'text-brand border-primary': tab.isActive,
							'border-transparent text-muted-foreground hover:border-border': !tab.isActive,
						})}
					>
						{tab.label}
					</Link>
				))}
			</nav>
		</div>
	)
}

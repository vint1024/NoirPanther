import { useInfiniteCursorGraphQL } from '@stump/client'
import { Avatar, Button, Card, Text } from '@stump/components'
import { graphql } from '@stump/graphql'
import { useLocaleContext } from '@stump/i18n'
import upperFirst from 'lodash/upperFirst'
import { useMemo } from 'react'

import { useBookClubContext } from '@/components/bookClub'

const query = graphql(`
	query BookClubMembersList($id: ID!, $pagination: CursorPagination!) {
		bookClubMembers(bookClubId: $id, pagination: $pagination) {
			nodes {
				id
				avatarUrl
				isCreator
				displayName
				role
			}
			cursorInfo {
				nextCursor
				limit
			}
		}
	}
`)

/**
 * The club's member list for every member (the settings tab has the managed
 * table with removal). Deliberately a plain list rather than sharing the
 * settings table: pulling that module into a second async chunk produced a
 * circular vendor chunk that broke app start-up.
 */
export default function BookClubMembersScene() {
	const { t } = useLocaleContext()
	const {
		bookClub: { id, roleSpec },
	} = useBookClubContext()

	const { data, hasNextPage, isFetchingNextPage, fetchNextPage } = useInfiniteCursorGraphQL(
		query,
		['bookClubMembersList', id],
		{ id, pagination: { limit: 50 } },
	)
	const members = useMemo(
		() => data?.pages.flatMap((page) => page.bookClubMembers.nodes) ?? [],
		[data],
	)
	const spec = (roleSpec ?? {}) as Record<string, string>

	return (
		<Card className="divide-y divide-border">
			{members.map((member) => {
				const name =
					member.displayName || t('scenes.bookClub.tabs.settings.members.MembersTable.member')
				const role = spec[member.role] || upperFirst(member.role.toLowerCase())
				return (
					<div key={member.id} className="gap-3 px-4 py-3 flex items-center">
						<Avatar src={member.avatarUrl ?? undefined} fallback={name} className="h-8 w-8" />
						<div className="min-w-0 flex flex-1 flex-col">
							<Text size="sm" className="font-medium truncate">
								{name}
							</Text>
							<Text size="xs" variant="muted">
								{member.isCreator
									? t('scenes.bookClub.tabs.members.BookClubMembersScene.creator')
									: role}
							</Text>
						</div>
					</div>
				)
			})}
			{hasNextPage && (
				<div className="p-3 flex justify-center">
					<Button
						size="sm"
						variant="outline"
						disabled={isFetchingNextPage}
						onClick={() => fetchNextPage()}
					>
						{t('scenes.bookClub.tabs.settings.members.MembersTable.loadMore')}
					</Button>
				</div>
			)}
		</Card>
	)
}

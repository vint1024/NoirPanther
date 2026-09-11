import { UserPermission } from '@stump/graphql'
import { lazy, useEffect } from 'react'
import { Navigate, Route, Routes, useNavigate } from 'react-router'

import { UnderConstruction } from '@/components/unimplemented'

import { useAppContext } from '../../context'
import BookClubHomeLayout from './BookClubLayout.tsx'
import BookClubSettingsRouter from './tabs/settings'

const CreateBookClubScene = lazy(() => import('./createClub'))
const UserBookClubsScene = lazy(() => import('./UserBookClubsScene.tsx'))
const BookClubExplorerScene = lazy(() => import('./explore/BookClubExploreScene.tsx'))

// club-specific routes
const BookClubHomeScene = lazy(() => import('./tabs/home'))
const BookClubDiscussionScene = lazy(() => import('./tabs/discussion/index.ts'))
const BookClubMembersScene = lazy(() => import('./tabs/members'))

// Upstream still hides book clubs in production builds (stumpapp/stump#120)
// and only renders them in dev. The NoirPanther fork ships them: the mobile
// client is built around clubs, and the web scenes carry our pagination /
// DataLoader work. Flip this back to `import.meta.env.DEV` to re-hide them.
const BOOK_CLUBS_ENABLED = true

export default function BookClubRouter() {
	const { checkPermission } = useAppContext()

	const navigate = useNavigate()
	const canAccess = checkPermission(UserPermission.AccessBookClub)
	useEffect(() => {
		if (!canAccess) {
			navigate('..', { replace: true })
		}
	}, [canAccess, navigate])

	if (!canAccess) {
		return null
	}

	if (!BOOK_CLUBS_ENABLED) {
		return (
			<Routes>
				<Route path="*" element={<UnderConstruction issue={120} />} />
			</Routes>
		)
	}

	return (
		<Routes>
			<Route path="" element={<UserBookClubsScene />} />
			<Route path="explore" element={<BookClubExplorerScene />} />
			{/* TODO: router guard bookclub:create */}
			<Route path="create" element={<CreateBookClubScene />} />
			<Route path=":slug/*" element={<BookClubHomeLayout />}>
				<Route path="" element={<BookClubHomeScene />} />
				<Route path="home" element={<Navigate to=".." replace />} />
				<Route path="discussion" element={<BookClubDiscussionScene />} />
				<Route path="members" element={<BookClubMembersScene />} />
				<Route path="settings/*" element={<BookClubSettingsRouter />} />
			</Route>
			<Route path="*" element={<Navigate to="/404" />} />
		</Routes>
	)
}

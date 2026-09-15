import { useSDK, useSuspenseGraphQL } from '@stump/client'
import { graphql } from '@stump/graphql'
import { Helmet } from 'react-helmet'
import { useMediaMatch } from 'rooks'

import { SceneContainer } from '@/components/container'
import QuickSearch from '@/components/QuickSearch'

import ContinueReadingMedia, { usePrefetchContinueReading } from './ContinueReading'
import NoLibraries from './NoLibraries'
import OnDeck, { usePrefetchOnDeck } from './OnDeck'
import RecentlyAddedMedia, { usePrefetchRecentlyAddedMedia } from './RecentlyAddedMedia'
import RecentlyAddedSeries, { usePrefetchRecentlyAddedSeries } from './RecentlyAddedSeries'

const query = graphql(`
	query HomeSceneQuery {
		numberOfLibraries
	}
`)

export const usePrefetchHomeScene = () => {
	const prefetchRecentMedia = usePrefetchRecentlyAddedMedia()
	const prefetchContinueReading = usePrefetchContinueReading()
	const prefetchRecentSeries = usePrefetchRecentlyAddedSeries()
	const prefetchOnDeck = usePrefetchOnDeck()

	return () =>
		Promise.all([
			prefetchRecentMedia(),
			prefetchContinueReading(),
			prefetchRecentSeries(),
			prefetchOnDeck(),
		])
}

// TODO: account for new accounts, i.e. no media at all
export default function HomeScene() {
	const { sdk } = useSDK()
	const { data } = useSuspenseGraphQL(query, sdk.cacheKey('numberOfLibraries'))
	// Mobile/PWA has no sidebar "Explore" entry in sight, so offer search right on top
	const isMobile = useMediaMatch('(max-width: 768px)')

	const helmet = (
		<Helmet>
			{/* Doing this so Helmet splits the title into an array, I'm not just insane lol */}
			<title>NoirPanther | {'Home'}</title>
		</Helmet>
	)

	if (!data) {
		return null
	}

	const { numberOfLibraries } = data

	if (numberOfLibraries === 0) {
		return (
			<>
				{helmet}
				<NoLibraries />
			</>
		)
	}

	return (
		<SceneContainer className="gap-6 flex flex-col">
			{helmet}

			{isMobile && <QuickSearch />}

			<ContinueReadingMedia />
			<OnDeck />
			<RecentlyAddedMedia />
			<RecentlyAddedSeries />
			<div className="pb-5 sm:pb-0" />
		</SceneContainer>
	)
}

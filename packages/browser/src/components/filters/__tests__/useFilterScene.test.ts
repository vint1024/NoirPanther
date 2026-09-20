import { renderHook } from '@testing-library/react'

import { useSearchMediaFilter, useSearchSeriesFilter } from '../useFilterScene'

/**
 * B11: typing an author into a search box has to find that author's books. The mobile client has
 * always searched `metadata.writers` alongside the name; the web only looked at name, title and
 * summary, so "толстой" found nothing while the same search in the app found three books. The
 * filter is a plain hook, so the shape of what it sends is worth pinning.
 */
describe('useSearchMediaFilter', () => {
	it('searches the author as well as the name, title and summary', () => {
		const { result } = renderHook(() => useSearchMediaFilter('толстой'))
		const fields = (result.current ?? []).map((clause) =>
			'name' in clause ? 'name' : Object.keys((clause as { metadata: object }).metadata)[0],
		)
		expect(fields).toEqual(expect.arrayContaining(['name', 'title', 'summary', 'writers']))
	})

	it('passes the term through untouched, so the server can fold case itself', () => {
		const { result } = renderHook(() => useSearchMediaFilter('ВОЙНА'))
		for (const clause of result.current ?? []) {
			expect(JSON.stringify(clause)).toContain('ВОЙНА')
		}
	})

	it('filters nothing when the box is empty', () => {
		expect(renderHook(() => useSearchMediaFilter(undefined)).result.current).toBeUndefined()
		expect(renderHook(() => useSearchMediaFilter('')).result.current).toBeUndefined()
	})
})

describe('useSearchSeriesFilter', () => {
	// Series metadata has no writers column (see SeriesMetadataFilterInput), so a series search
	// stays on name/title/summary — this test says so on purpose, to stop the "fix" being copied.
	it('searches name, title and summary', () => {
		const { result } = renderHook(() => useSearchSeriesFilter('oz'))
		expect(result.current).toHaveLength(3)
	})
})

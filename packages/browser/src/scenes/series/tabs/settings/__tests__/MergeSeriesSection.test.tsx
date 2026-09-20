import { useGraphQL, useGraphQLMutation, useSDK, useSuspenseGraphQL } from '@stump/client'
import { LocaleProvider } from '@stump/i18n'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import MergeSeriesSection from '../MergeSeriesSection'

vi.mock('@stump/client', () => ({
	useGraphQL: vi.fn(),
	useGraphQLMutation: vi.fn(),
	useSDK: vi.fn(),
	useSuspenseGraphQL: vi.fn(),
}))
vi.mock('@tanstack/react-query', () => ({
	useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}))

const SERIES_ID = 'series-1'

const merge = vi.fn()
const unmerge = vi.fn()

const setup = ({ mergedSources = [] as { name: string; path: string }[] } = {}) => {
	vi.mocked(useSDK).mockReturnValue({
		sdk: { cacheKey: (key: string) => [key], cacheKeys: {} },
	} as any)
	vi.mocked(useSuspenseGraphQL).mockReturnValue({
		data: {
			seriesById: {
				id: SERIES_ID,
				library: { id: 'library-1' },
				mergedSources,
			},
		},
	} as any)
	vi.mocked(useGraphQL).mockReturnValue({
		data: {
			series: {
				nodes: [
					{ id: SERIES_ID, name: 'This very series', path: '/books/one' },
					{ id: 'series-2', name: 'The target series', path: '/books/two' },
				],
			},
		},
	} as any)
	// the component calls this twice per render, always in the same order: merge, then unmerge
	let hookCall = 0
	vi.mocked(useGraphQLMutation).mockImplementation(
		() =>
			({
				mutate: hookCall++ % 2 === 0 ? merge : unmerge,
				isPending: false,
			}) as any,
	)

	return render(
		<LocaleProvider>
			<MergeSeriesSection seriesId={SERIES_ID} />
		</LocaleProvider>,
	)
}

/**
 * A2/B10: merging series is a fork feature with an easy-to-get-wrong API — `mergeSeries` takes
 * the TARGET and the sources, while `unmergeSeries` takes the series that absorbed the others,
 * not the source. Swapping either argument compiles and silently does the wrong thing to a user's
 * library, so these tests pin the arguments, not just the buttons.
 */
describe('MergeSeriesSection', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('never offers to merge a series into itself', async () => {
		setup()
		const options = screen.getAllByRole('option').map((option) => option.textContent)
		expect(options).not.toContain('This very series')
		expect(options).toContain('The target series')
	})

	it('merges this series into the chosen target', async () => {
		setup()

		const button = screen.getByRole('button', { name: /merge/i })
		expect(button).toBeDisabled()

		await userEvent.selectOptions(screen.getByRole('combobox'), 'series-2')
		await userEvent.click(button)

		expect(merge).toHaveBeenCalledWith({ targetId: 'series-2', sourceIds: [SERIES_ID] })
	})

	it('undoes merges with the id of the series that absorbed them', async () => {
		setup({ mergedSources: [{ name: 'Absorbed', path: '/books/absorbed' }] })

		expect(screen.getByText(/Absorbed/)).toBeInTheDocument()
		await userEvent.click(screen.getByRole('button', { name: /restore|unmerge|separate/i }))

		expect(unmerge).toHaveBeenCalledWith({ id: SERIES_ID })
	})

	it('shows nothing to undo when the series has absorbed none', () => {
		setup()
		expect(screen.queryByRole('button', { name: /restore|unmerge|separate/i })).toBeNull()
	})
})

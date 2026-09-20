import { LocaleProvider } from '@stump/i18n'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import QuickSearch from '../QuickSearch'

const navigate = vi.fn()
vi.mock('react-router', () => ({
	useNavigate: () => navigate,
}))
vi.mock('@/paths', () => ({
	usePaths: () => ({ bookSearch: () => '/books' }),
}))

const renderSearch = (props = {}) =>
	render(
		<LocaleProvider>
			<QuickSearch {...props} />
		</LocaleProvider>,
	)

/**
 * B16: the mobile/PWA search box. It hands the query to the book search scene, which reads it
 * with `useURLKeywordSearch` — and that hook decodes the value once more than you would expect,
 * so the box has to encode it twice. Losing that (or the trim) breaks searching for anything with
 * a space or a Cyrillic letter, which is most of this library.
 */
describe('QuickSearch', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('sends the query to the book search, encoded the way the scene reads it', async () => {
		renderSearch()

		await userEvent.type(screen.getByRole('searchbox'), 'война и мир{Enter}')

		expect(navigate).toHaveBeenCalledWith({
			pathname: '/books',
			search: `search=${encodeURIComponent(encodeURIComponent('война и мир'))}`,
		})
	})

	it('trims what the user typed', async () => {
		renderSearch()

		await userEvent.type(screen.getByRole('searchbox'), '  tolstoy  {Enter}')

		expect(navigate).toHaveBeenCalledWith({
			pathname: '/books',
			search: 'search=tolstoy',
		})
	})

	it('still opens the search scene on an empty query, without a stray parameter', async () => {
		renderSearch()

		await userEvent.type(screen.getByRole('searchbox'), '   {Enter}')

		expect(navigate).toHaveBeenCalledWith({ pathname: '/books', search: '' })
	})

	it('clears itself and tells the caller, so the menu it lives in can close', async () => {
		const onSubmitted = vi.fn()
		renderSearch({ onSubmitted })

		const box = screen.getByRole('searchbox')
		await userEvent.type(box, 'dune{Enter}')

		expect(box).toHaveValue('')
		expect(onSubmitted).toHaveBeenCalled()
	})
})

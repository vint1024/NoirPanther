import { LocaleProvider } from '@stump/i18n'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'
import { FormProvider, useForm } from 'react-hook-form'

import BasicLibraryInformation from '../BasicLibraryInformation'

// The section pulls in a tag picker (GraphQL) and a library-type select; neither is what this
// test is about, and both need a server.
vi.mock('@/components/TagSelect', () => ({
	default: () => <div data-testid="tag-select" />,
}))
vi.mock('../LibraryType', () => ({
	default: () => <div data-testid="library-type" />,
}))
vi.mock('@/context', () => ({
	useAppContext: () => ({ checkPermission: () => true }),
}))
vi.mock('@/scenes/library/context', () => ({
	useLibraryContextSafe: () => null,
}))

const Wrapper = ({ children }: PropsWithChildren) => {
	const form = useForm({
		defaultValues: { extraPaths: [] as string[], name: '', path: '' },
	})
	return (
		<LocaleProvider>
			<FormProvider {...form}>{children}</FormProvider>
		</LocaleProvider>
	)
}

const renderSection = () =>
	render(
		<Wrapper>
			<BasicLibraryInformation onPickDirectory={vi.fn()} />
		</Wrapper>,
	)

/**
 * A1 (multi-folder libraries) lives half in the server and half in this form. The v0.1.7 merge
 * kept the `extraPaths` field in the schema and the mutation, and dropped the inputs that fill
 * it: the feature was gone from the UI while every type still checked out. These tests render the
 * section and look for the controls, which is the only thing that would have caught that.
 */
describe('BasicLibraryInformation — extra folders', () => {
	it('offers a way to add a folder', () => {
		renderSection()
		expect(screen.getByRole('button', { name: /add folder/i })).toBeInTheDocument()
	})

	it('adds, fills and removes an extra folder row', async () => {
		const { container } = renderSection()

		expect(container.querySelector('input[name="extraPaths.0"]')).toBeNull()

		await userEvent.click(screen.getByRole('button', { name: /add folder/i }))
		const extraPath = container.querySelector<HTMLInputElement>('input[name="extraPaths.0"]')
		expect(extraPath).not.toBeNull()

		await userEvent.type(extraPath as HTMLInputElement, '/books/more')
		expect(extraPath).toHaveValue('/books/more')

		await userEvent.click(screen.getByRole('button', { name: /remove folder/i }))
		expect(container.querySelector('input[name="extraPaths.0"]')).toBeNull()
	})

	it('keeps adding rows, so a library can have several extra folders', async () => {
		renderSection()

		const addFolder = screen.getByRole('button', { name: /add folder/i })
		await userEvent.click(addFolder)
		await userEvent.click(addFolder)

		expect(screen.getAllByRole('button', { name: /remove folder/i })).toHaveLength(2)
	})
})

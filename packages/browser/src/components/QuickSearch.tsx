import { cn } from '@stump/components'
import { useLocaleContext } from '@stump/i18n'
import { SearchIcon } from 'lucide-react'
import { FormEvent, useCallback, useState } from 'react'
import { useNavigate } from 'react-router'

import { usePaths } from '@/paths'

type Props = {
	/** Called after navigating to the book search (e.g. to close a menu) */
	onSubmitted?: () => void
	className?: string
	autoFocus?: boolean
}

/**
 * A plain search box for mobile/PWA that sends the query to the book search
 * scene (`/books?search=…`). Styled like the filter-header `Search` input so it
 * looks native next to it.
 */
export default function QuickSearch({ onSubmitted, className, autoFocus }: Props) {
	const { t } = useLocaleContext()
	const navigate = useNavigate()
	const paths = usePaths()
	const [value, setValue] = useState('')

	const handleSubmit = useCallback(
		(e: FormEvent<HTMLFormElement>) => {
			e.preventDefault()
			const query = value.trim()
			const params = new URLSearchParams()
			// `useURLKeywordSearch` stores the value encoded once more, mirror it
			if (query) params.set('search', encodeURIComponent(query))
			navigate({ pathname: paths.bookSearch(), search: params.toString() })
			setValue('')
			onSubmitted?.()
		},
		[navigate, onSubmitted, paths, value],
	)

	return (
		<form
			role="search"
			onSubmit={handleSubmit}
			className={cn(
				'h-9 gap-2 text-sm relative flex w-full items-center overflow-hidden rounded-md border border-border bg-background/60',
				'text-muted-foreground focus-within:border-ring focus-within:text-foreground',
				className,
			)}
		>
			<SearchIcon className="ml-2.5 h-4 w-4 shrink-0" />
			<input
				type="search"
				enterKeyHint="search"
				autoFocus={autoFocus}
				autoCapitalize="off"
				autoCorrect="off"
				value={value}
				onChange={(e) => setValue(e.target.value)}
				placeholder={t('components.filters.QuickSearch.placeholder')}
				aria-label={t('components.filters.QuickSearch.menuLabel')}
				className="pr-2.5 text-base md:text-sm h-full w-full bg-transparent text-foreground outline-none placeholder:text-muted-foreground"
			/>
			<button type="submit" className="sr-only" tabIndex={-1}>
				{t('components.filters.QuickSearch.menuLabel')}
			</button>
		</form>
	)
}

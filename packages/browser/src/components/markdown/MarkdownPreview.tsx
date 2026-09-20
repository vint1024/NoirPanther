/* eslint-disable @typescript-eslint/no-unused-vars */
import { cn, cx, Divider, Heading, Text } from '@stump/components'
import { useLocaleContext } from '@stump/i18n'
import { forwardRef, PropsWithChildren, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeRaw from 'rehype-raw'
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize'
import remarkDirective from 'remark-directive'
import remarkDirectiveRehype from 'remark-directive-rehype'
import remarkGfm from 'remark-gfm'

type Props = {
	children: string
	className?: string
}

/**
 * Descriptions come from book files, which are downloaded from all over the internet, and
 * `rehypeRaw` renders the HTML inside them. React strips event handlers and unsafe URLs, but
 * everything else was rendered as-is: a book could embed a tracking pixel (who read what and
 * when), an off-site form (a login prompt on a page the reader trusts), an iframe, or a style
 * block that repaints the app. Keep the formatting tags a description legitimately uses and
 * drop the rest.
 */
export const SANITIZE_SCHEMA = {
	...defaultSchema,
	tagNames: [
		'p',
		'br',
		'hr',
		'span',
		'div',
		'blockquote',
		'pre',
		'code',
		'em',
		'i',
		'strong',
		'b',
		'u',
		's',
		'del',
		'ins',
		'sub',
		'sup',
		'small',
		'mark',
		'abbr',
		'a',
		'img',
		'ul',
		'ol',
		'li',
		'dl',
		'dt',
		'dd',
		'h1',
		'h2',
		'h3',
		'h4',
		'h5',
		'h6',
		'table',
		'thead',
		'tbody',
		'tfoot',
		'tr',
		'th',
		'td',
	],
	attributes: {
		...defaultSchema.attributes,
		// Images may only come from this server (no third-party tracking pixels)
		img: ['alt', 'title', 'width', 'height'],
		'*': ['className', 'id', 'title', 'lang', 'dir'],
	},
	protocols: {
		...defaultSchema.protocols,
		href: ['http', 'https', 'mailto'],
		src: [],
	},
}

export default function MarkdownPreview({ children, className }: Props) {
	return (
		<ReactMarkdown
			remarkPlugins={[remarkDirective, remarkDirectiveRehype, remarkGfm]}
			rehypePlugins={[rehypeRaw, [rehypeSanitize, SANITIZE_SCHEMA]]}
			className={cn('text-foreground', className)}
			components={{
				h1: ({ ref: _, ...props }) => (
					<>
						<Heading {...props} size="xl" />
						<Divider className="my-1" variant="muted" />
					</>
				),
				h2: ({ ref: _, ...props }) => <Heading {...props} size="lg" />,
				h3: ({ ref: _, ...props }) => <Heading {...props} size="md" />,
				h4: ({ ref: _, ...props }) => <Heading {...props} size="xs" />,
				h5: ({ ref: _, ...props }) => <Text {...props} className="font-medium" />,
				p: ({ ref: _, node: __, ...props }) => <Text {...props} />,
				ul: ({ ref: _, ...props }) => <ul {...props} className="my-2 pl-6 list-disc" />,
				ol: ({ ref: _, ...props }) => <ol {...props} className="my-2 pl-6 list-decimal" />,
				li: ({ ref: _, ...props }) => <li {...props} className="my-1" />,
				table: Table,
				thead: Thead,
				tbody: Tbody,
				tr: Tr,
				td: Td,
				th: Th,
				// @ts-expect-error: this is a custom component
				spoiler: Spoiler,
				'youtube-video': YouTubeVideo,
			}}
		>
			{children}
		</ReactMarkdown>
	)
}

const YouTubeVideo = ({ id, children }: PropsWithChildren<{ id: string }>) => (
	<iframe src={'https://www.youtube.com/embed/' + id} width="200" height="200">
		{children}
	</iframe>
)

/**
 * A spoiler component, e.g. :spoiler[hidden text]
 */
const Spoiler = ({ children }: PropsWithChildren) => {
	const { t } = useLocaleContext()
	const [isSpoiler, setIsSpoiler] = useState(true)

	return (
		<span
			className={cx(
				{
					'text-gray/0 cursor-pointer bg-gray-800': isSpoiler,
				},
				{
					'bg-background/10': !isSpoiler,
				},
			)}
			onClick={() => setIsSpoiler(!isSpoiler)}
			title={
				isSpoiler
					? t('components.markdown.MarkdownPreview.clickToReveal')
					: t('components.markdown.MarkdownPreview.clickToHide')
			}
		>
			{children}
		</span>
	)
}

const Table = forwardRef<HTMLTableElement, PropsWithChildren>((props, ref) => {
	return (
		<div className="my-1 overflow-hidden rounded-xl border border-border">
			<table ref={ref} {...props} className="w-full divide-y divide-border" />
		</div>
	)
})
Table.displayName = 'MarkdownTable'

const Thead = forwardRef<HTMLTableSectionElement, PropsWithChildren>((props, ref) => {
	return <thead ref={ref} {...props} />
})
Thead.displayName = 'MarkdownTableHeader'

const Tbody = forwardRef<HTMLTableSectionElement, PropsWithChildren>((props, ref) => {
	return <tbody ref={ref} {...props} className="divide-y divide-border" />
})
Tbody.displayName = 'MarkdownTableBody'

const Tr = forwardRef<HTMLTableRowElement, PropsWithChildren>((props, ref) => {
	return <tr ref={ref} {...props} className="w-fit divide-x divide-border" />
})
Tr.displayName = 'MarkdownTableRow'

const Td = forwardRef<HTMLTableCellElement, PropsWithChildren>((props, ref) => {
	return <td ref={ref} {...props} className="py-2 pl-1.5 pr-1.5 first:pl-4 last:pr-4" />
})
Td.displayName = 'MarkdownTableCell'

const Th = forwardRef<HTMLTableCellElement, PropsWithChildren>((props, ref) => {
	return (
		<th
			ref={ref}
			{...props}
			className="h-8 pl-1.5 pr-1.5 text-sm first:pl-4 last:pr-4 relative bg-muted/50 text-left"
		/>
	)
})
Th.displayName = 'MarkdownTableHeaderCell'

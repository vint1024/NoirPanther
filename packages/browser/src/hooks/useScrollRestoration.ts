import { useEffect, useLayoutEffect, useRef } from 'react'
import { useLocation, useNavigationType } from 'react-router-dom'

const STORAGE_KEY = 'noirpanther:scroll-positions:v2'
const MAX_ENTRIES = 100
// How long we keep trying to restore while the (suspended / paginated) content is
// still rendering and the containers aren't wide/tall enough yet
const RESTORE_TIMEOUT_MS = 3000

type NestedOffset = { left: number; top: number }
type Entry = {
	/** Vertical offset of the page scroller */
	top: number
	/** Offsets of nested scrollers (card rails, tables…) keyed by their DOM path */
	nested?: Record<string, NestedOffset>
}
type Positions = Record<string, Entry>

const readPositions = (): Positions => {
	try {
		const raw = window.sessionStorage.getItem(STORAGE_KEY)
		return raw ? (JSON.parse(raw) as Positions) : {}
	} catch {
		return {}
	}
}

const positions: Positions = readPositions()
let persistTimer: ReturnType<typeof setTimeout> | undefined

const persist = () => {
	clearTimeout(persistTimer)
	persistTimer = setTimeout(() => {
		try {
			const keys = Object.keys(positions)
			if (keys.length > MAX_ENTRIES) {
				for (const key of keys.slice(0, keys.length - MAX_ENTRIES)) delete positions[key]
			}
			window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(positions))
		} catch {
			// sessionStorage unavailable (private mode, quota) — in-memory map still works
		}
	}, 250)
}

const entryFor = (key: string): Entry => (positions[key] ??= { top: 0 })

const rememberTop = (keys: string[], top: number) => {
	for (const key of keys) entryFor(key).top = top
	persist()
}

const rememberNested = (keys: string[], path: string, offset: NestedOffset) => {
	for (const key of keys) {
		const entry = entryFor(key)
		entry.nested ??= {}
		entry.nested[path] = offset
	}
	persist()
}

// The initial history entry has this key; after a full reload every entry looks like it
const INITIAL_KEY = 'default'
const pathKey = (pathname: string, search: string) => `path:${pathname}${search}`

const PAGE_ROOT_ID = 'main'

/** Child-index path from `#main` down to `el`, e.g. "0/2/0/1" — stable for a given render */
const domPath = (el: HTMLElement): string | undefined => {
	const root = document.getElementById(PAGE_ROOT_ID)
	if (!root || !root.contains(el)) return undefined
	const indices: number[] = []
	let node: HTMLElement | null = el
	while (node && node !== root) {
		const parent: HTMLElement | null = node.parentElement
		if (!parent) return undefined
		indices.unshift(Array.prototype.indexOf.call(parent.children, node))
		node = parent
	}
	return indices.join('/')
}

const resolvePath = (path: string): HTMLElement | undefined => {
	let node: Element | null = document.getElementById(PAGE_ROOT_ID)
	for (const index of path.split('/')) {
		node = node?.children[Number(index)] ?? null
		if (!node) return undefined
	}
	return node instanceof HTMLElement ? node : undefined
}

/**
 * Remembers where the user was on a page and puts them back there on back/forward
 * navigation (POP): the vertical offset of the app's scroll container plus the
 * offsets of any nested scroller inside it (the horizontal card rails on the home
 * page, wide tables…). Forward navigations (PUSH/REPLACE) start at the top.
 *
 * Offsets are keyed by history entry (`location.key`) and, as a fallback that
 * survives a full document reload, by URL.
 *
 * `getCandidates` returns the elements that may be doing the page scrolling, in
 * order of preference. There can be more than one because overlayscrollbars moves
 * scrolling from `#main` into its own viewport child some time after mount — so
 * the scroll events are captured on `document`, and the element to restore is
 * resolved on every attempt.
 */
export function useScrollRestoration(getCandidates: () => Array<HTMLElement | null | undefined>) {
	const location = useLocation()
	const navigationType = useNavigationType()
	const key = location.key
	// Second key by URL so the offsets survive a full reload (the iOS PWA reloads the
	// document on edge-swipe back, and reloads reset every history key to `default`)
	const urlKey = pathKey(location.pathname, location.search)
	// While a restore is in flight the containers emit scroll events on their own
	// (content growing/shrinking clamps offsets) — those must not overwrite the saved ones
	const restoringRef = useRef(false)

	// Track the offsets of the current entry. Scroll events don't bubble, but they do
	// capture — so one listener on the document sees whichever element is scrolling.
	// (Listening on `document` rather than the scroll container itself because the
	// container mounts later than this effect runs on a cold load.)
	useEffect(() => {
		const onScroll = (e: Event) => {
			if (restoringRef.current) return
			const target = e.target
			if (!(target instanceof HTMLElement)) return
			if (getCandidates().includes(target)) {
				rememberTop([key, urlKey], target.scrollTop)
				return
			}
			// A nested scroller inside the page (rails, tables): remember it by DOM path
			const path = domPath(target)
			if (path)
				rememberNested([key, urlKey], path, { left: target.scrollLeft, top: target.scrollTop })
		}
		document.addEventListener('scroll', onScroll, { capture: true, passive: true })
		return () => document.removeEventListener('scroll', onScroll, { capture: true })
	}, [getCandidates, key, urlKey])

	// Restore (POP) or reset (PUSH/REPLACE) when the entry changes
	useLayoutEffect(() => {
		const scrollers = () => getCandidates().filter((c): c is HTMLElement => !!c)

		if (navigationType !== 'POP') {
			for (const scroller of scrollers()) scroller.scrollTop = 0
			return
		}

		// A reloaded document (key === 'default') falls back to the URL-keyed entry
		const entry = positions[key] ?? (key === INITIAL_KEY ? positions[urlKey] : undefined)
		if (!entry) return
		const nested = Object.entries(entry.nested ?? {}).filter(
			([, offset]) => offset.left > 0 || offset.top > 0,
		)
		if (!entry.top && nested.length === 0) return

		restoringRef.current = true
		let cancelled = false
		let frame = 0
		const startedAt = performance.now()

		const finish = () => {
			cancelled = true
			restoringRef.current = false
			cancelAnimationFrame(frame)
			document.removeEventListener('wheel', finish)
			document.removeEventListener('touchmove', finish)
		}
		// The user took over — stop fighting them
		document.addEventListener('wheel', finish, { passive: true })
		document.addEventListener('touchmove', finish, { passive: true })

		// Move `el` to `wanted`; true once it is there, false while the content is still
		// too short (then it's parked as far as it currently goes and retried next frame)
		const settle = (el: HTMLElement, axis: 'top' | 'left', wanted: number) => {
			if (!wanted) return true
			const max =
				axis === 'top' ? el.scrollHeight - el.clientHeight : el.scrollWidth - el.clientWidth
			const prop = axis === 'top' ? 'scrollTop' : 'scrollLeft'
			if (max >= wanted) {
				el[prop] = wanted
				return Math.abs(el[prop] - wanted) < 2
			}
			if (max > 0) el[prop] = max
			return false
		}

		const attempt = () => {
			if (cancelled) return
			let done = true
			// Whichever candidate actually overflows right now is the one scrolling
			const page = scrollers().find((c) => c.scrollHeight > c.clientHeight)
			if (entry.top) done = !!page && settle(page, 'top', entry.top) && done
			for (const [path, offset] of nested) {
				const el = resolvePath(path)
				if (!el) {
					done = false
					continue
				}
				done = settle(el, 'left', offset.left) && done
				done = settle(el, 'top', offset.top) && done
			}
			if (done) {
				finish()
			} else if (performance.now() - startedAt < RESTORE_TIMEOUT_MS) {
				frame = requestAnimationFrame(attempt)
			} else {
				finish()
			}
		}
		attempt()

		return finish
	}, [getCandidates, key, navigationType, urlKey])
}

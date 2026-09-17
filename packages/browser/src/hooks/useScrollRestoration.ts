import { useEffect, useLayoutEffect, useRef } from 'react'
import { useLocation, useNavigationType } from 'react-router-dom'

const STORAGE_KEY = 'noirpanther:scroll-positions'
const MAX_ENTRIES = 100
// How long we keep trying to restore while the (suspended / paginated) content is
// still rendering and the container isn't tall enough yet
const RESTORE_TIMEOUT_MS = 3000

type Positions = Record<string, number>

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

const remember = (keys: string[], top: number) => {
	for (const key of keys) positions[key] = top
	persist()
}

// The initial history entry has this key; after a full reload every entry looks like it
const INITIAL_KEY = 'default'
const pathKey = (pathname: string, search: string) => `path:${pathname}${search}`

/**
 * Remembers the scroll offset of the app's scroll container per history entry
 * (`location.key`) and restores it when the user navigates back/forward (POP).
 * Forward navigations (PUSH/REPLACE) start at the top, like a normal page load.
 *
 * `getCandidates` returns the elements that may be doing the scrolling, in order of
 * preference. There can be more than one because overlayscrollbars moves scrolling
 * from `#main` into its own viewport child some time after mount — so the scroll
 * events are captured on `document`, and the element to restore is resolved on
 * every attempt.
 */
export function useScrollRestoration(getCandidates: () => Array<HTMLElement | null | undefined>) {
	const location = useLocation()
	const navigationType = useNavigationType()
	const key = location.key
	// Second key by URL so the offset survives a full reload (the iOS PWA reloads the
	// document on some back gestures, and reloads reset every history key to `default`)
	const urlKey = pathKey(location.pathname, location.search)
	// While a restore is in flight the container emits scroll events on its own (content
	// shrinking/growing clamps scrollTop) — those must not overwrite the saved offset
	const restoringRef = useRef(false)

	// Track the offset of the current entry. Scroll events don't bubble, but they do
	// capture — so one listener on the document sees whichever element is scrolling.
	// (Listening on `document` rather than the scroll container itself because the
	// container mounts later than this effect runs on a cold load.)
	useEffect(() => {
		const onScroll = (e: Event) => {
			if (restoringRef.current) return
			const target = e.target
			if (!(target instanceof HTMLElement)) return
			// Only the page scroller itself: nested scrollers (the horizontal card rails
			// on the home page, tables…) also emit scroll events here, and their
			// scrollTop is 0 — recording that would wipe the saved vertical offset
			if (!getCandidates().includes(target)) return
			remember([key, urlKey], target.scrollTop)
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

		// A reloaded document (key === 'default') falls back to the URL-keyed offset
		const target = positions[key] ?? (key === INITIAL_KEY ? positions[urlKey] : undefined)
		if (!target) return

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

		const attempt = () => {
			if (cancelled) return
			// Whichever candidate actually overflows right now is the one scrolling
			const current = scrollers().find((c) => c.scrollHeight > c.clientHeight)
			if (current) {
				const maxTop = current.scrollHeight - current.clientHeight
				if (maxTop >= target) {
					current.scrollTop = target
					if (Math.abs(current.scrollTop - target) < 2) {
						finish()
						return
					}
				} else {
					// Get as close as we can while the rest of the content is still loading
					current.scrollTop = maxTop
				}
			}
			if (performance.now() - startedAt < RESTORE_TIMEOUT_MS) {
				frame = requestAnimationFrame(attempt)
			} else {
				finish()
			}
		}
		attempt()

		return finish
	}, [getCandidates, key, navigationType, urlKey])
}

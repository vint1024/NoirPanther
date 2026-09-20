import { constants as zlibConstants } from 'node:zlib'

import babel from '@rolldown/plugin-babel'
import tailwindcss from '@tailwindcss/vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { compression, defineAlgorithm } from 'vite-plugin-compression2'
import { VitePWA } from 'vite-plugin-pwa'
import reactFallbackThrottlePlugin from 'vite-plugin-react-fallback-throttle'
import tsconfigPaths from 'vite-plugin-tsconfig-paths'

// https://www.npmjs.com/package/vite-plugin-node-polyfills
import { name, version } from './package.json'

// https://vitejs.dev/config/
export default defineConfig({
	build: {
		assetsDir: './assets',
		manifest: true,
		outDir: '../dist',
		emptyOutDir: true,
		rollupOptions: {
			output: {
				manualChunks(id) {
					const path = id.replaceAll('\\', '/')
					// Our i18n package is a workspace source, not a dependency — but it pulls in
					// i18next and date-fns locales, so it belongs with them (see vendor-react below)
					if (/packages[\\/]i18n[\\/]/.test(path)) {
						return 'vendor-react'
					}
					if (!path.includes('/node_modules/')) {
						return
					}

					if (path.includes('/node_modules/lucide-react/')) {
						return 'vendor-lucide'
					}
					if (path.includes('/node_modules/@tanstack/')) {
						return 'vendor-tanstack'
					}
					if (path.includes('/node_modules/lodash/')) {
						return 'vendor-lodash'
					}
					if (path.includes('/node_modules/framer-motion/')) {
						return 'vendor-framer'
					}
					if (path.includes('/node_modules/overlayscrollbars/')) {
						return 'vendor-overlayscrollbars'
					}
					// NoirPanther: react, the router, i18next (+ our i18n package) and date-fns
					// must land in ONE chunk. Split apart, rolldown put date-fns' en-US locale
					// in its own chunk that imported a CJS-interop helper from the react chunk
					// while the react chunk imported the locale — a circular chunk graph that
					// left the helper undefined and broke app start-up (blank splash).
					// Detector: .build-logs/chunk_cycles.py
					if (
						/\/node_modules\/(react|react-dom|scheduler|use-sync-external-store|react-i18next|i18next|date-fns|react-router|@remix-run)\//.test(
							path,
						)
					) {
						return 'vendor-react'
					}
				},
			},
		},
	},
	clearScreen: false,
	define: {
		pkgJson: { name, version },
	},
	plugins: [
		tailwindcss(),
		react(),
		babel({ presets: [reactCompilerPreset()] }),
		tsconfigPaths(),
		compression({
			include: [/\.(js|mjs|json|css|html|svg|xml|wasm)$/i],
			exclude: [/\.(png|jpe?g|gif|webp|avif|woff2?|mp4|webm)$/i],
			threshold: 1024,
			algorithms: [
				defineAlgorithm('gzip', { level: 9 }),
				defineAlgorithm('brotliCompress', {
					params: {
						[zlibConstants.BROTLI_PARAM_QUALITY]: 11,
					},
				}),
			],
		}),
		VitePWA({
			// We manually register in src/index.tsx to add idle scheduling and script preflight checks.
			injectRegister: null,
			// 'prompt' (without an actual prompt): a new service worker installs and WAITS, so the
			// running page keeps its own (still cached) assets and is never reloaded mid-session.
			// The update takes over on the next launch — iOS kills the home-screen app daily anyway.
			// 'autoUpdate' reloaded the page seconds after each deploy, right in the middle of use.
			registerType: 'prompt',
			devOptions: {
				enabled: false,
			},
			workbox: {
				inlineWorkboxRuntime: true,
				navigateFallbackDenylist: [
					/^\/api(?:\/|$)/,
					/^\/opds(?:\/|$)/,
					/^\/kobo(?:\/|$)/,
					/^\/koreader(?:\/|$)/,
				],
				maximumFileSizeToCacheInBytes: 6 * 1024 * 1024, // 6MB
			},
			outDir: '../dist',
			base: '/',
			// TODO(pwa): Add more manifest definitions for better overall experience
			manifest: {
				id: 'stump',
				name: 'NoirPanther',
				short_name: 'NoirPanther',
				description: 'NoirPanther book server',
				theme_color: '#0b0a10',
				background_color: '#0b0a10',
				icons: [
					{
						src: '/assets/favicon-16x16.png',
						sizes: '16x16',
						type: 'image/png',
					},
					{
						src: '/assets/favicon-192x192.png',
						sizes: '192x192',
						type: 'image/png',
					},
					{
						src: '/assets/favicon-512x512.png',
						sizes: '512x512',
						type: 'image/png',
						purpose: 'any maskable',
					},
				],
			},
			manifestFilename: 'manifest.webmanifest',
		}),
		reactFallbackThrottlePlugin(),
	],
	publicDir: '../../../packages/browser/public',
	root: 'src',
	server: {
		port: 3000,
	},
})

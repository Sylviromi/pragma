/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
	plugins: [sveltekit()],
	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		watch: {
			/* Ignore everything the coding agent itself writes while working on
			   this repo: build artifacts, the plan file, docs, git internals.
			   Without these, a plan.md/docs update or a cargo run lands as an
			   HMR full-reload that kicks the running app's frontend mid-turn. */
			ignored: [
				'**/src-tauri/**',
				'**/.git/**',
				'**/.svelte-kit/**',
				'**/docs/**',
				'**/build/**',
				'**/node_modules/**',
				'**/PLAN.md'
			],
			/* /mnt is an automount with flaky inotify; fall back to polling so
			   edits always reach the running app (stale-serve has bitten us
			   repeatedly). */
			usePolling: true,
			interval: 300
		}
	},
	test: {
		environment: 'jsdom',
		setupFiles: ['src/test-setup.ts'],
		globals: true
	}
});
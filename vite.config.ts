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
			ignored: ['**/src-tauri/**'],
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
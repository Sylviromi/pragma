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
			ignored: ['**/src-tauri/**']
		}
	},
	test: {
		environment: 'jsdom',
		setupFiles: ['src/test-setup.ts'],
		globals: true
	}
});
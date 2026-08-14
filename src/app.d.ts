// App ambient types for SvelteKit
declare global {
	namespace App {}
}

// Vite `?raw` imports (inline logo.svg used by the tab bar chip).
declare module '*.svg?raw' {
	const content: string
	export default content
}

export {};

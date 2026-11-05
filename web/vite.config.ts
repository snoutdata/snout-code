import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// A static site: `vite build` makes the client bundle, `vite build --ssr` the renderer that
// scripts/prerender.mjs runs to write every page's HTML, so the site needs no server.
export default defineConfig({
	plugins: [react()],
	server: { port: 5190 }
})

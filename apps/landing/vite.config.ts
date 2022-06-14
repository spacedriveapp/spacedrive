import vercelSsr from '@magne4000/vite-plugin-vercel-ssr';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import md, { Mode } from 'vite-plugin-markdown';
import ssr from 'vite-plugin-ssr/plugin';
import svg from 'vite-plugin-svgr';
import vercel from 'vite-plugin-vercel';

// https://vitejs.dev/config/
export default defineConfig({
	// @ts-ignore
	plugins: [react(), ssr(), svg(), md({ mode: [Mode.REACT] })],
	resolve: {
		alias: {
			'~/docs': __dirname + '../../../docs'
		}
	},
	server: {
		port: 8003
	},
	build: {
		polyfillDynamicImport: false
	},
	vercel: {
		// Tweak what you need, check TS definition for details
	},
	publicDir: 'public'
});

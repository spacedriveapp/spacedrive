import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import md, { Mode } from 'vite-plugin-markdown';
import ssr from 'vite-plugin-ssr/plugin';
import svg from 'vite-plugin-svgr';

// https://vitejs.dev/config/
export default defineConfig({
	// @ts-ignore
	plugins: [react(), ssr({ prerender: true }), svg(), md({ mode: [Mode.REACT] })],
	resolve: {
		alias: {
			'~/docs': __dirname + '../../../docs'
		}
	},
	server: {
		port: 8003
	},
	publicDir: 'public'
});

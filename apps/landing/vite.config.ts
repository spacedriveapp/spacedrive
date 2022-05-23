import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import pages from 'vite-plugin-pages';
import md, { Mode } from 'vite-plugin-markdown';
import svg from 'vite-plugin-svgr';

// https://vitejs.dev/config/
export default defineConfig({
	// @ts-ignore
	plugins: [
		react(),
		pages({
			dirs: 'src/pages'
			// onRoutesGenerated: (routes) => generateSitemap({ routes })
		}),
		svg(),
		md({ mode: [Mode.REACT] })
	],
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

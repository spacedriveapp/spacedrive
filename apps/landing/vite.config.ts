import react from '@vitejs/plugin-react';
import { visualizer } from 'rollup-plugin-visualizer';
import { defineConfig } from 'vite';
import esm from 'vite-plugin-esmodule';
import md, { Mode } from 'vite-plugin-markdown';
import ssr from 'vite-plugin-ssr/plugin';
import svg from 'vite-plugin-svgr';

export default defineConfig({
	plugins: [react(), ssr({ prerender: true }), svg(), md({ mode: [Mode.REACT] }), visualizer()],
	resolve: {
		alias: {
			'~/docs': __dirname + '../../../docs',
			'@heroicons/react/24/solid': '@heroicons/react/24/solid/index.js',
			'@heroicons/react/24/outline': '@heroicons/react/24/outline/index.js'
		}
	},

	server: {
		port: 8003
	},
	publicDir: 'public'
});

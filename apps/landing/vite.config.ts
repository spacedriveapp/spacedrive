import react from '@vitejs/plugin-react';
import { visualizer } from 'rollup-plugin-visualizer';
import { defineConfig } from 'vite';
import md, { Mode } from 'vite-plugin-markdown';
import ssr from 'vite-plugin-ssr/plugin';
import svg from 'vite-plugin-svgr';

export default defineConfig({
	plugins: [react(), ssr({ prerender: true }), svg(), md({ mode: [Mode.REACT] }), visualizer()],
	build: {
		rollupOptions: {
			output: {
				format: 'cjs'
			}
		}
	},
	resolve: {
		alias: {
			'~/docs': __dirname + '../../../docs'
		}
	},
	build: {
		rollupOptions: {
			output: {
				format: 'cjs'
			}
		}
	},
	server: {
		port: 8003
	},
	publicDir: 'public'
});

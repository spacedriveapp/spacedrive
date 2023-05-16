import vercelSsr from '@magne4000/vite-plugin-vercel-ssr';
import react from '@vitejs/plugin-react';
import path from 'path';
import { visualizer } from 'rollup-plugin-visualizer';
import { defineConfig } from 'vite';
import md, { Mode } from 'vite-plugin-markdown';
import ssr from 'vite-plugin-ssr/plugin';
import svg from 'vite-plugin-svgr';
import vercel from 'vite-plugin-vercel';

export default defineConfig({
	plugins: [
		react(),
		svg(),
		md({ mode: [Mode.REACT] }),
		visualizer(),
		ssr({ prerender: true }),
		vercel(),
		vercelSsr()
	],
	css: {
		modules: {
			localsConvention: 'camelCaseOnly'
		}
	},
	resolve: {
		alias: [
			{
				find: '@sd/',
				replacement: path.join(__dirname, '../../packages/')
			}
		]
	},
	server: {
		port: 8003
	},
	publicDir: 'public'
});

import react from '@vitejs/plugin-react';
import path from 'path';
import { visualizer } from 'rollup-plugin-visualizer';
import { defineConfig } from 'vite';
import esm from 'vite-plugin-esmodule';
import md, { Mode } from 'vite-plugin-markdown';
import ssr from 'vite-plugin-ssr/plugin';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from "vite-tsconfig-paths"

export default defineConfig({
	plugins: [
		tsconfigPaths(),
		react(),
		ssr({ prerender: true }),
		svg(),
		md({ mode: [Mode.REACT] }),
		visualizer()
	],
	// resolve: {
	// 	alias: [
	// 		{
	// 			find: '@sd/',
	// 			replacement: path.join(__dirname, '../../packages/')
	// 		}
	// 	]
	// },
	server: {
		port: 8003
	},
	publicDir: 'public'
});

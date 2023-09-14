import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import { comlink } from 'vite-plugin-comlink';
import { createHtmlPlugin } from 'vite-plugin-html';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from 'vite-tsconfig-paths';

import relativeAliasResolver from './relativeAliasResolver';

export default defineConfig({
	plugins: [
		tsconfigPaths(),
		react(),
		svg({ svgrOptions: { icon: true } }),
		createHtmlPlugin({
			minify: true
		}),
		comlink()
	],
	css: {
		modules: {
			localsConvention: 'camelCaseOnly'
		}
	},
	root: 'src',
	build: {
		sourcemap: true,
		outDir: '../dist',
		assetsDir: '.'
	},
	worker: {
		plugins: [comlink()]
	}
});

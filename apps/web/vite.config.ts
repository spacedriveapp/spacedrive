import react from '@vitejs/plugin-react';
import { visualizer } from 'rollup-plugin-visualizer';
import { defineConfig } from 'vite';
import { createHtmlPlugin } from 'vite-plugin-html';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from 'vite-plugin-tsconfig-paths';

import { name, version } from './package.json';

// https://vitejs.dev/config/
export default defineConfig({
	server: {
		port: 8002
	},
	plugins: [
		react(),
		svg({ svgrOptions: { icon: true } }),
		tsconfigPaths(),
		createHtmlPlugin({
			minify: true
		}),
		visualizer({
			gzipSize: true,
			brotliSize: true
		})
	],
	root: 'src',
	publicDir: '../public',
	define: {
		pkgJson: { name, version }
	},
	build: {
		outDir: '../dist',
		assetsDir: '.'
	}
});

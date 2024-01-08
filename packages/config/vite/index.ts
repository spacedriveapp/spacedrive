import react from '@vitejs/plugin-react-swc';
import { defineConfig } from 'vite';
import { createHtmlPlugin } from 'vite-plugin-html';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from 'vite-tsconfig-paths';

export default defineConfig({
	plugins: [
		tsconfigPaths(),
		// @ts-expect-error
		react(),
		svg({ svgrOptions: { icon: true } }),
		createHtmlPlugin({
			minify: true
		})
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
	}
});

import { fileURLToPath } from 'node:url';
import ts from '@babel/preset-typescript';
import react from '@vitejs/plugin-react-swc';
import million from 'million/compiler';
import { defineConfig } from 'vite';
import { createHtmlPlugin } from 'vite-plugin-html';
import i18nextLoader from 'vite-plugin-i18next-loader';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from 'vite-tsconfig-paths';

import { narrowSolidPlugin } from './narrowSolidPlugin';

const url = new URL('../../../interface/locales', import.meta.url);

export default defineConfig({
	plugins: [
		million.vite({ auto: true }),
		tsconfigPaths(),
		i18nextLoader({
			paths: [fileURLToPath(url.href)],
			namespaceResolution: 'relativePath'
		}),
		react(),
		narrowSolidPlugin({ include: '**/*.solid.tsx', babel: { presets: [[ts, {}]] } }),
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

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import svg from 'vite-plugin-svgr';

import { name, version } from './package.json';

// https://vitejs.dev/config/
export default defineConfig({
	server: {
		port: 8001
	},
	plugins: [
		//@ts-ignore
		react({
			jsxRuntime: 'classic'
		}),
		svg({ svgrOptions: { icon: true } })
	],
	root: 'src',
	publicDir: '../../packages/interface/src/assets',
	define: {
		pkgJson: { name, version }
	},
	build: {
		outDir: '../dist',
		assetsDir: '.'
	}
});

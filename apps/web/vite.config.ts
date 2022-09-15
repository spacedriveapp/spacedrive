import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from 'vite-plugin-tsconfig-paths';

import { name, version } from './package.json';

// https://vitejs.dev/config/
export default defineConfig({
	server: {
		port: 8002
	},
	plugins: [react(), svg({ svgrOptions: { icon: true } }), tsconfigPaths()],
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

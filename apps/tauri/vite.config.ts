import path from 'path';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react-swc';
import {defineConfig} from 'vite';

const COMMANDS = ['initialize_core', 'core_rpc', 'subscribe_events'];

export default defineConfig(async () => ({
	plugins: [react(), tailwindcss()],

	resolve: {
		dedupe: ['react', 'react-dom'],
		alias: [
			{
				find: /^react$/,
				replacement: path.resolve(
					__dirname,
					'./node_modules/react/index.js'
				)
			},
			{
				find: /^react\/jsx-runtime$/,
				replacement: path.resolve(
					__dirname,
					'./node_modules/react/jsx-runtime.js'
				)
			},
			{
				find: /^react\/jsx-dev-runtime$/,
				replacement: path.resolve(
					__dirname,
					'./node_modules/react/jsx-dev-runtime.js'
				)
			},
			{
				find: /^react-dom$/,
				replacement: path.resolve(
					__dirname,
					'./node_modules/react-dom/index.js'
				)
			},
			{
				find: /^react-dom\/client$/,
				replacement: path.resolve(
					__dirname,
					'./node_modules/react-dom/client.js'
				)
			},
			{
				find: 'openapi-fetch',
				replacement: path.resolve(
					__dirname,
					'../../packages/interface/node_modules/openapi-fetch/dist/index.mjs'
				)
			},
			{
				find: '@spaceui/tokens/src/css',
				replacement: path.resolve(
					__dirname,
					'../../../spaceui/packages/tokens/src/css'
				)
			},
			{
				find: '@spaceui/tokens',
				replacement: path.resolve(
					__dirname,
					'../../../spaceui/packages/tokens'
				)
			},
			{
				find: '@spaceui/ai',
				replacement: path.resolve(
					__dirname,
					'../../../spaceui/packages/ai/src/index.ts'
				)
			},
			{
				find: '@spaceui/primitives',
				replacement: path.resolve(
					__dirname,
					'../../../spaceui/packages/primitives/src/index.ts'
				)
			},
			{
				find: '@spacebot/api-client',
				replacement: path.resolve(
					__dirname,
					'../../../spacebot/packages/api-client/src'
				)
			},
			{
				find: '@sd/interface',
				replacement: path.resolve(
					__dirname,
					'../../packages/interface/src'
				)
			},
			{
				find: '@sd/ts-client',
				replacement: path.resolve(
					__dirname,
					'../../packages/ts-client/src'
				)
			}
		]
	},

	optimizeDeps: {
		exclude: ['@spaceui/ai', '@spaceui/primitives', '@spaceui/tokens']
	},

	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		fs: {
			allow: [
				path.resolve(__dirname, '../../..'),
				path.resolve(__dirname, '../../../spaceui')
			]
		},
		watch: {
			ignored: ['**/src-tauri/**']
		}
	},
	envPrefix: ['VITE_', 'TAURI_ENV_*'],
	build: {
		target: ['es2021', 'chrome100', 'safari13'],
		minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
		sourcemap: !!process.env.TAURI_ENV_DEBUG
	}
}));

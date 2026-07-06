import path from 'path';
import fs from 'fs';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react-swc';
import {defineConfig} from 'vite';

const spaceui = path.resolve(__dirname, '../../../spaceui/packages');
const hasSpaceui = fs.existsSync(spaceui);
const spacebot = path.resolve(__dirname, '../../../spacebot/packages');
const hasSpacebot = fs.existsSync(spacebot);

export default defineConfig(() => ({
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
			// SpaceUI — resolve to source for HMR when available locally
			...(hasSpaceui
				? [
						{
							find: /^@spacedrive\/tokens\/css\/themes\/(.+)$/,
							replacement: `${spaceui}/tokens/src/css/themes/$1.css`,
						},
						{
							find: /^@spacedrive\/tokens\/theme$/,
							replacement: `${spaceui}/tokens/src/css/theme.css`,
						},
						{
							find: /^@spacedrive\/tokens\/css$/,
							replacement: `${spaceui}/tokens/src/css/base.css`,
						},
						{
							find: /^@spacedrive\/tokens$/,
							replacement: `${spaceui}/tokens`,
						},
						{
							find: /^@spacedrive\/ai$/,
							replacement: `${spaceui}/ai/src/index.ts`,
						},
						{
							find: /^@spacedrive\/primitives$/,
							replacement: `${spaceui}/primitives/src/index.ts`,
						},
					]
				: []),
			...(hasSpacebot
				? [
						{
							find: /^@spacebot\/api-client$/,
							replacement: `${spacebot}/api-client/src`,
						},
					]
				: [
						{
							find: /^@spacebot\/api-client$/,
							replacement: path.resolve(
								__dirname,
								'./stubs/spacebot-api-client.ts'
							),
						},
					]),
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
		exclude: ['@spacedrive/ai', '@spacedrive/primitives', '@spacedrive/tokens']
	},

	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		fs: {
			allow: [
				path.resolve(__dirname, '../../..'),
				...(hasSpaceui ? [spaceui] : []),
			]
		},
		watch: {
			ignored: ['**/src-tauri/**']
		}
	},
	envPrefix: ['VITE_', 'TAURI_ENV_*'],
	build: {
		target: ['es2021', 'chrome100', 'safari13'],
		minify: !process.env.TAURI_ENV_DEBUG ? ('esbuild' as const) : false,
		sourcemap: !!process.env.TAURI_ENV_DEBUG,
		rollupOptions: {
			external: [
				...(!hasSpacebot ? ['@spacebot/api-client'] : []),
			],
		}
	}
}));

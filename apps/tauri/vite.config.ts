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
	plugins: [
		react(),
		tailwindcss(),
		// Provide a stub for @spacebot/api-client when the spacebot sibling repo is not present.
		// This prevents Vite dev from failing with "Failed to resolve import" which leads to
		// grey/blank screen in the Tauri webview (the module graph breaks for Spacebot code
		// pulled in via the router).
		{
			name: 'spacebot-stub',
			resolveId(id: string) {
				if (id === '@spacebot/api-client' && !hasSpacebot) {
					return '\0virtual:spacebot-stub';
				}
			},
			load(id: string) {
				if (id === '\0virtual:spacebot-stub') {
					// Return pure JS (no TS syntax like `?:` or `export type`).
					// Vite serves this virtual as JS; browser would choke on TS syntax
					// causing "Unexpected token '?'" and empty #root (grey screen).
					return `
export const apiClient = {};
export function getEventsUrl() { return ''; }
export function setServerUrl(_url) {}
export default apiClient;
`;
				}
			},
		},
		// Stub hast-util-to-jsx-runtime (pulls in style-to-js which has CJS interop problems under Vite).
		// This prevents the "no default export" / require errors that keep #root empty (grey screen).
		{
			name: 'hast-util-to-jsx-runtime-stub',
			resolveId(id) {
				if (id === 'hast-util-to-jsx-runtime') return '\0virtual:hast-to-jsx-stub';
			},
			load(id) {
				if (id === '\0virtual:hast-to-jsx-stub') {
					return `
export function toJsxRuntime(tree, options) {
  // Minimal stub: avoid pulling style-to-js and heavy hast transform in dev.
  // Return a harmless empty span if createElement is provided by React JSX runtime.
  try {
    const create = (options && options.createElement) || ((t, p, ...c) => ({type:t, props:p, children:c}));
    return create('span', { style: { display: 'none' } }, '');
  } catch {
    return null;
  }
}
export default toJsxRuntime;
`;
				}
			},
		},
		// Also provide style-to-js directly as pure JS (belt and suspenders).
		{
			name: 'style-to-js-stub',
			resolveId(id) {
				if (id === 'style-to-js' || id.includes('style-to-js')) {
					return '\0virtual:style-to-js-stub';
				}
			},
			load(id) {
				if (id === '\0virtual:style-to-js-stub') {
					return `
function camelCase(str) {
  return String(str || '').trim().replace(/-+([a-z0-9])/gi, (_, c) => c.toUpperCase());
}
export default function styleToJS(style) {
  const out = {};
  if (!style || typeof style !== 'string') return out;
  String(style).split(';').forEach((d) => {
    const i = d.indexOf(':');
    if (i > -1) {
      const k = d.slice(0, i).trim();
      const v = d.slice(i + 1).trim();
      if (k && v) out[camelCase(k)] = v;
    }
  });
  return out;
}
`;
				}
			},
		},
	],

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
				: []),
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
		exclude: [
			'@spacedrive/ai',
			'@spacedrive/primitives',
			'@spacedrive/tokens',
			// Transitives that pull in awkward CJS style-to-js and cause default export / require errors in dev.
			'style-to-js',
			'style-to-object',
			'hast-util-to-jsx-runtime',
		]
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

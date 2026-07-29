import path from 'path';
import fs from 'fs';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react-swc';
import {defineConfig} from 'vite';

/** Resolve a Bun-hoisted transitive dependency to its CJS entry. */
function resolveBunPackageDir(packageName: string): string {
	const bunDir = path.resolve(__dirname, '../../node_modules/.bun');
	const match = fs
		.readdirSync(bunDir)
		.find((entry) => entry.startsWith(`${packageName}@`));
	if (!match) {
		throw new Error(`Could not find bun package: ${packageName}`);
	}
	return path.resolve(bunDir, match, 'node_modules', packageName);
}

function resolveBunEntry(packageName: string): string {
	const pkgDir = resolveBunPackageDir(packageName);
	const pkgJson = JSON.parse(
		fs.readFileSync(path.join(pkgDir, 'package.json'), 'utf8')
	) as {main?: string; browser?: string};
	const entry = pkgJson.browser ?? pkgJson.main ?? 'index.js';
	const candidates = [
		path.resolve(pkgDir, entry),
		path.resolve(pkgDir, `${entry}.js`),
		path.resolve(pkgDir, entry, 'index.js'),
	];
	for (const candidate of candidates) {
		if (fs.existsSync(candidate)) {
			return candidate;
		}
	}
	throw new Error(`Could not resolve entry for ${packageName}`);
}

// CJS packages loaded via Bun's .bun store need pre-bundling for ESM default imports.
const CJS_INTEROP_PACKAGES = [
	'style-to-js',
	'style-to-object',
	'inline-style-parser',
	'extend',
	'is-plain-obj',
	'bail',
	'trough',
	'ms',
] as const;

const cjsInteropEntries = CJS_INTEROP_PACKAGES.flatMap((packageName) => {
	try {
		return [{packageName, entry: resolveBunEntry(packageName)}];
	} catch {
		return [];
	}
});

const debugStub = path.resolve(__dirname, './src/stubs/debug.ts');

const spaceui = path.resolve(__dirname, '../../../spaceui/packages');
const hasSpaceui = fs.existsSync(spaceui);
const spacebot = path.resolve(__dirname, '../../../spacebot/packages');
const hasSpacebot = fs.existsSync(spacebot);
const spacebotStub = path.resolve(
	__dirname,
	'./src/stubs/spacebot-api-client.ts'
);

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
			{
				find: /^@spacebot\/api-client$/,
				replacement: hasSpacebot
					? `${spacebot}/api-client/src`
					: spacebotStub,
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
			},
			// react-markdown/unified CJS deps imported as ESM defaults
			...cjsInteropEntries.map(({packageName, entry}) => ({
				find: packageName,
				replacement: entry,
			})),
			{find: 'debug', replacement: debugStub},
		]
	},

	optimizeDeps: {
		exclude: ['@spacedrive/ai', '@spacedrive/primitives', '@spacedrive/tokens'],
		include: cjsInteropEntries.map(({entry}) => entry),
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
		rollupOptions: {}
	}
}));

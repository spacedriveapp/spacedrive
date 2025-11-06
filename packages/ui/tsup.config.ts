import { defineConfig } from 'tsup';

export default defineConfig({
	entry: ['src/index.ts', 'src/forms/index.ts'],
	format: ['esm'],
	dts: false,
	clean: true,
	external: [
		'react',
		'react-dom',
		'react-router-dom',
		'@jamiepine/assets',
		'zod',
		'clsx',
		'class-variance-authority',
		'react-hook-form',
		'valtio',
		'rooks',
		'@zxcvbn-ts/core',
		'@zxcvbn-ts/language-en',
		'@zxcvbn-ts/language-common'
	],
	noExternal: [],
	bundle: true,
	splitting: false,
	treeshake: true,
	sourcemap: true,
	esbuildOptions(options) {
		options.jsx = 'automatic';
	}
});

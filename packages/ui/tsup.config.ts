import { defineConfig } from 'tsup';

export default defineConfig({
	entry: ['src/index.ts', 'src/forms/index.ts'],
	format: ['esm'],
	dts: true,
	clean: true,
	external: [
		'react',
		'react-dom',
		'react-router-dom',
		'@jamiepine/assets',
		'zod',
		'clsx',
		'class-variance-authority'
	],
	treeshake: true,
	sourcemap: true,
	esbuildOptions(options) {
		options.jsx = 'automatic';
	}
});

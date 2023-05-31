module.exports = {
	extends: [
		require.resolve('@sd/config/eslint/base.js'),
		require.resolve('@sd/config/eslint/tailwind.js'),
		'next',
		'next/core-web-vitals'
	],
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	rules: {
		// Overriding this next rule again here.
		'react/no-unescaped-entities': 'off',
		// ???
		'turbo/no-undeclared-env-vars': 'off'
	},
	ignorePatterns: [
		'dist',
		'**/*.js',
		'**/*.json',
		'node_modules',
		'.contentlayer',
		'.next',
		'posts'
	]
};

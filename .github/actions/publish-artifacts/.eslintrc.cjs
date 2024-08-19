module.exports = {
	root: true,
	env: {
		'node': true,
		'es2022': true,
		'browser': false,
		'commonjs': false,
		'shared-node-browser': false
	},
	parser: '@typescript-eslint/parser',
	extends: [
		'eslint:recommended',
		'standard',
		'plugin:@typescript-eslint/strict-type-checked',
		'plugin:@typescript-eslint/stylistic-type-checked',
		'plugin:prettier/recommended'
	],
	plugins: ['@typescript-eslint'],
	parserOptions: {
		project: true
	},
	ignorePatterns: ['node_modules/**/*', 'dist/**/*']
};

module.exports = {
	root: true,
	env: {
		node: true,
		es2022: true,
		browser: false,
		commonjs: false,
		'shared-node-browser': false,
	},
	rules: {
		'no-void': [
			'error',
			{
				allowAsStatement: true,
			},
		],
		'no-proto': 'error',
		'valid-jsdoc': 'off',
		'import/order': [
			'error',
			{
				alphabetize: {
					order: 'asc',
				},
				'newlines-between': 'always',
			},
		],
		'no-unused-vars': [
			'error',
			{ argsIgnorePattern: '^_', destructuredArrayIgnorePattern: '^_' },
		],
		'jsdoc/require-returns-check': 'off',
		'jsdoc/require-param-description': 'off',
		'jsdoc/require-returns-description': 'off',
		'standard/no-callback-literal': 'off',
	},
	parser: '@babel/eslint-parser',
	plugins: ['@babel'],
	extends: [
		'eslint:recommended',
		'standard',
		'plugin:import/recommended',
		'plugin:prettier/recommended',
		'plugin:jsdoc/recommended-typescript-flavor',
	],
	settings: {
		jsdoc: {
			mode: 'typescript',
			tagNamePreference: {
				typicalname: 'typicalname',
			},
		},
	},
	parserOptions: {
		project: './tsconfig.json',
		sourceType: 'module',
		babelOptions: {
			presets: [
				[
					'@babel/preset-env',
					{
						shippedProposals: true,
					},
				],
			],
		},
		tsconfigRootDir: __dirname,
		requireConfigFile: false,
	},
}

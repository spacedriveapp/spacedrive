const path = require('node:path');

module.exports = {
	parser: '@typescript-eslint/parser',
	parserOptions: {
		ecmaFeatures: {
			jsx: true
		},
		ecmaVersion: 12,
		sourceType: 'module',
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	plugins: ['react'],
	extends: [
		'eslint:recommended',
		'plugin:@typescript-eslint/recommended',
		'plugin:react/recommended',
		'plugin:react-hooks/recommended',
		'plugin:tailwindcss/recommended',
		'turbo',
		'prettier'
	],
	env: {
		browser: true,
		node: true
	},
	rules: {
		// TypeScript
		'@typescript-eslint/no-unused-vars': 'off',
		'@typescript-eslint/ban-ts-comment': 'off',
		'@typescript-eslint/no-explicit-any': 'off',
		'@typescript-eslint/no-var-requires': 'off',
		'@typescript-eslint/no-non-null-assertion': 'off',
		'@typescript-eslint/explicit-module-boundary-types': 'off',
		'@typescript-eslint/no-empty-interface': 'off',
		'@typescript-eslint/no-empty-function': 'off',
		'@typescript-eslint/ban-types': 'off',
		// React
		'react/display-name': 'off',
		'react/prop-types': 'off',
		'react/no-unescaped-entities': 'off',
		'react/react-in-jsx-scope': 'off',
		'react-hooks/rules-of-hooks': 'warn',
		'react-hooks/exhaustive-deps': 'warn',
		// Tailwind
		'tailwindcss/no-custom-classname': 'off',
		'tailwindcss/classnames-order': [
			'warn',
			{
				config: path.resolve(path.join(__dirname, './tailwind.config.js'))
			}
		],
		// General
		'no-control-regex': 'off',
		'no-mixed-spaces-and-tabs': ['warn', 'smart-tabs'],
		'turbo/no-undeclared-env-vars': [
			'error',
			{
				cwd: path.resolve(path.join(__dirname, '..', '..'))
			}
		],
		// Custom routing rules
		'no-restricted-syntax': [
			'error',
			{
				selector: "CallExpression[callee.name='useParams']",
				message: 'useParams is illegal, use useZodRouteParams!'
			},
			{
				selector: "CallExpression[callee.name='useSearchParams']",
				message: 'useSearchParams is illegal, use useZodSearchParams!'
			}
		]
	},
	settings: {
		react: {
			version: 'detect'
		},
		tailwindcss: {
			callees: ['classnames', 'clsx', 'ctl', 'cva', 'tw', 'twStyle'],
			tags: ['tw', 'twStyle']
		}
	},
	ignorePatterns: ['dist', '**/*.js', '**/*.json', 'node_modules', 'public', 'vite.config.ts']
};

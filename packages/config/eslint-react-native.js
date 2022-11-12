module.exports = {
	env: {
		'react-native/react-native': true
	},
	parser: '@typescript-eslint/parser',
	parserOptions: {
		ecmaFeatures: {
			jsx: true
		},
		ecmaVersion: 12,
		sourceType: 'module'
	},
	extends: [
		'eslint:recommended',
		'plugin:react/recommended',
		'plugin:react-hooks/recommended',
		'plugin:@typescript-eslint/recommended'
	],
	plugins: ['react', 'react-native'],
	rules: {
		'react/display-name': 'off',
		'react/prop-types': 'off',
		'react/no-unescaped-entities': 'off',
		'react/react-in-jsx-scope': 'off',
		'react-hooks/rules-of-hooks': 'error',
		'react-hooks/exhaustive-deps': 'warn',
		'@typescript-eslint/no-unused-vars': 'off',
		'@typescript-eslint/ban-ts-comment': 'off',
		'@typescript-eslint/no-explicit-any': 'off',
		'@typescript-eslint/no-var-requires': 'off',
		'@typescript-eslint/no-non-null-assertion': 'off',
		'@typescript-eslint/explicit-module-boundary-types': 'off',
		'no-control-regex': 'off',
		'no-mixed-spaces-and-tabs': ['warn', 'smart-tabs'],
		'no-restricted-imports': [
			'error',
			{
				paths: [
					{
						name: 'react-native',
						importNames: ['SafeAreaView'],
						message: 'Import SafeAreaView from react-native-safe-area-context instead'
					},
					{
						name: 'react-native',
						importNames: ['Button'],
						message: 'Import Button from ~/components instead.'
					}
				]
			}
		]
	},
	ignorePatterns: ['**/*.js', '**/*.json', 'node_modules', 'android', 'ios', '.expo'],
	settings: {
		react: {
			version: 'detect'
		}
	}
};

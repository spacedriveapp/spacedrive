module.exports = {
	extends: [require.resolve('@sd/config/eslint/reactNative.js')],
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	rules: {
		'tailwindcss/classnames-order': [
			'warn',
			{
				config: './tailwind.config.js'
			}
		],
		'tailwindcss/no-contradicting-classname': 'warn',
		'tailwindcss/enforces-shorthand': 'off',
		'@typescript-eslint/no-require-imports': 'off',
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
						importNames: ['Image', 'ImageProps', 'ImageBackground'],
						message: 'Import it from expo-image instead'
					},
					{
						name: 'react-native-toast-message',
						message: 'Import it from components instead'
					}
				]
			}
		]
	}
};

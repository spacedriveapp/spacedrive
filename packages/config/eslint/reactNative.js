module.exports = {
	extends: [require.resolve('./base.js'), 'plugin:tailwindcss/recommended'],
	env: {
		'react-native/react-native': true
	},
	plugins: ['react-native'],
	ignorePatterns: ['android', 'ios', '.expo'],
	rules: {
		'no-restricted-imports': [
			'error',
			{
				paths: [
					{
						name: 'react-native',
						importNames: ['SafeAreaView'],
						message: 'Import SafeAreaView from react-native-safe-area-context instead'
					}
					// {
					// 	name: 'react-native',
					// 	importNames: ['Button'],
					// 	message: 'Import Button from ~/components instead.'
					// }
				]
			}
		],
		'tailwindcss/no-custom-classname': 'off',
		'tailwindcss/no-contradicting-classname': 'warn'
	},
	settings: {
		tailwindcss: {
			config: './apps/mobile/tailwind.config.js',
			callees: ['classnames', 'clsx', 'ctl', 'cva', 'tw', `twStyle`],
			tags: ['tw', 'twStyle']
		}
	}
};

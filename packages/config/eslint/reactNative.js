module.exports = {
	extends: [require.resolve('./base.js')],
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
		]
	},
	settings: {
		tailwindcss: {
			config: 'apps/mobile/tailwind.config.js'
		}
	}
};

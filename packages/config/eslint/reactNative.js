module.exports = {
	extends: [require.resolve('./base.js'), require.resolve('./tailwind.js')],
	env: {
		'react-native/react-native': true
	},
	plugins: ['react-native'],
	ignorePatterns: ['android', 'ios', '.expo']
};

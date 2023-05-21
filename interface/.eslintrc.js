module.exports = {
	extends: [require.resolve('@sd/config/eslint/web.js')],
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	rules: {
		'react-hooks/exhaustive-deps': [
			'warn',
			{ additionalHooks: '(useCallbackToWatchForm|useCallbackToWatchResize)' }
		]
	}
};

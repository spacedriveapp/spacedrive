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
		]
	}
};

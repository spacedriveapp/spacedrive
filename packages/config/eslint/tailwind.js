const path = require('node:path');
module.exports = {
	extends: ['plugin:tailwindcss/recommended'],
	rules: {
		// FIX-ME: https://github.com/francoismassart/eslint-plugin-tailwindcss/issues/307
		'tailwindcss/enforces-shorthand': 'off',
		'tailwindcss/no-custom-classname': 'off',
		'tailwindcss/classnames-order': [
			'warn',
			{
				config: path.resolve(
					path.join(__dirname, '../../..', 'packages/ui/tailwind.config.js')
				)
			}
		]
	},
	settings: {
		tailwindcss: {
			callees: ['classnames', 'clsx', 'ctl', 'cva', 'tw', 'twStyle'],
			tags: ['tw', 'twStyle']
		}
	}
};

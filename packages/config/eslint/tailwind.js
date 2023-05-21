const path = require('node:path');
module.exports = {
	extends: ['plugin:tailwindcss/recommended'],
	rules: {
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

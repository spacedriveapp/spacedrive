const base = require('@sd/ui/tailwind')('landing');

/** @type {import('tailwindcss').Config} */
module.exports = {
	...base,
	theme: {
		...base.theme,
		extend: {
			...base.theme?.extend,
			animation: {
				...base.theme?.extend?.animation,
				scroll: 'scroll 80s linear infinite'
			},
			keyframes: {
				...base.theme?.extend?.keyframes,
				scroll: {
					'0%': { transform: 'translateX(0)' },
					'100%': { transform: 'translateX(-50%)' }
				}
			}
		}
	}
};

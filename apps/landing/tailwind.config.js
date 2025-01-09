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
				'scroll': 'scroll 80s linear infinite',
				'handle-rotate': 'handle-rotate 2s ease forwards',
				'digit-reveal': 'digit-reveal 1s cubic-bezier(0.4, 0, 0.2, 1) forwards'
			},
			keyframes: {
				...base.theme?.extend?.keyframes,
				'scroll': {
					'0%': { transform: 'translateX(0)' },
					'100%': { transform: 'translateX(-50%)' }
				},
				'handle-rotate': {
					'0%': {
						transform: 'rotate(0deg)',
						animationTimingFunction: 'cubic-bezier(0.3, 0, 0.3, 1)'
					},
					'65%': {
						transform: 'rotate(330deg)',
						animationTimingFunction: 'cubic-bezier(0, 0, 0.2, 1)'
					},
					'75%': {
						transform: 'rotate(330deg)',
						animationTimingFunction: 'cubic-bezier(0.3, 0, 0, 1)'
					},
					'100%': { transform: 'rotate(417deg)' }
				},
				'digit-reveal': {
					'0%': {
						opacity: '0',
						transform: 'scale(1.2)'
					},
					'50%': {
						opacity: '0.4',
						transform: 'scale(1.1)'
					},
					'100%': {
						opacity: '0.2',
						transform: 'scale(1)'
					}
				}
			},
			gridTemplateColumns: {
				20: 'repeat(20, minmax(0, 1fr))'
			}
		}
	}
};

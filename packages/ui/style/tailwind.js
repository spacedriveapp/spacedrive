// const colors = require('tailwindcss/colors');
const plugin = require('tailwindcss/plugin');
const defaultTheme = require('tailwindcss/defaultTheme');

function alpha(variableName) {
	// some tailwind magic to allow us to specify opacity with CSS variables (eg: bg-app/80)
	// https://tailwindcss.com/docs/customizing-colors#using-css-variables
	return `hsla(var(${variableName}), <alpha-value>)`;
}

module.exports = function (app, options) {
	let config = {
		content: [
			!options?.ignorePackages && '../../packages/*/src/**/*.{ts,tsx,html}',
			app ? `../../apps/${app}/src/**/*.{ts,tsx,html}` : `./src/**/*.{ts,tsx,html}`
		],
		darkMode: app == 'landing' ? 'class' : 'media',
		mode: 'jit',
		theme: {
			screens: {
				xs: '475px',
				...defaultTheme.screens
			},
			fontSize: {
				'tiny': '.65rem',
				'xs': '.75rem',
				'sm': '.80rem',
				'base': '1rem',
				'lg': '1.125rem',
				'xl': '1.25rem',
				'2xl': '1.5rem',
				'3xl': '1.875rem',
				'4xl': '2.25rem',
				'5xl': '3rem',
				'6xl': '4rem',
				'7xl': '5rem'
			},
			extend: {
				colors: {
					accent: {
						DEFAULT: alpha('--color-accent'),
						faint: alpha('--color-accent-faint'),
						deep: alpha('--color-accent-deep')
					},
					ink: {
						DEFAULT: alpha('--color-ink'),
						dull: alpha('--color-ink-dull'),
						faint: alpha('--color-ink-faint')
					},
					sidebar: {
						DEFAULT: alpha('--color-sidebar'),
						box: alpha('--color-sidebar-box'),
						line: alpha('--color-sidebar-line'),
						divider: alpha('--color-sidebar-divider'),
						button: alpha('--color-sidebar-button'),
						selected: alpha('--color-sidebar-selected'),
						shade: alpha('--color-sidebar-shade')
					},
					app: {
						DEFAULT: alpha('--color-app'),
						box: alpha('--color-app-box'),
						overlay: alpha('--color-app-overlay'),
						input: alpha('--color-app-input'),
						focus: alpha('--color-app-focus'),
						line: alpha('--color-app-line'),
						divider: alpha('--color-app-divider'),
						button: alpha('--color-app-button'),
						selected: alpha('--color-app-selected'),
						hover: alpha('--color-app-hover'),
						shade: alpha('--color-app-shade'),
						frame: alpha('--color-app-frame')
					},
					menu: {
						DEFAULT: alpha('--color-menu'),
						line: alpha('--color-menu-line'),
						hover: alpha('--color-menu-hover'),
						selected: alpha('--color-menu-selected'),
						shade: alpha('--color-menu-shade'),
						ink: alpha('--color-menu-ink'),
						faint: alpha('--color-menu-faint')
					}
				},
				extend: {
					transitionTimingFunction: {
						'css': 'ease',
						'css-in': 'ease-in',
						'css-out': 'ease-out',
						'css-in-out': 'ease-in-out',
						'in-sine': 'cubic-bezier(0.12, 0, 0.39, 0)',
						'out-sine': 'cubic-bezier(0.61, 1, 0.88, 1)',
						'in-out-sine': 'cubic-bezier(0.37, 0, 0.63, 1)',
						'in-quad': 'cubic-bezier(0.11, 0, 0.5, 0)',
						'out-quad': 'cubic-bezier(0.5, 1, 0.89, 1)',
						'in-out-quad': 'cubic-bezier(0.45, 0, 0.55, 1)',
						'in-cubic': 'cubic-bezier(0.32, 0, 0.67, 0)',
						'out-cubic': 'cubic-bezier(0.33, 1, 0.68, 1)',
						'in-out-cubic': 'cubic-bezier(0.65, 0, 0.35, 1)',
						'in-quart': 'cubic-bezier(0.5, 0, 0.75, 0)',
						'out-quart': 'cubic-bezier(0.25, 1, 0.5, 1)',
						'in-out-quart': 'cubic-bezier(0.76, 0, 0.24, 1)',
						'in-quint': 'cubic-bezier(0.64, 0, 0.78, 0)',
						'out-quint': 'cubic-bezier(0.22, 1, 0.36, 1)',
						'in-out-quint': 'cubic-bezier(0.83, 0, 0.17, 1)',
						'in-expo': 'cubic-bezier(0.7, 0, 0.84, 0)',
						'out-expo': 'cubic-bezier(0.16, 1, 0.3, 1)',
						'in-out-expo': 'cubic-bezier(0.87, 0, 0.13, 1)',
						'in-circ': 'cubic-bezier(0.55, 0, 1, 0.45)',
						'out-circ': 'cubic-bezier(0, 0.55, 0.45, 1)',
						'in-out-circ': 'cubic-bezier(0.85, 0, 0.15, 1)',
						'in-back': 'cubic-bezier(0.36, 0, 0.66, -0.56)',
						'out-back': 'cubic-bezier(0.34, 1.56, 0.64, 1)',
						'in-out-back': 'cubic-bezier(0.68, -0.6, 0.32, 1.6)'
					}
				}
			}
		},
		plugins: [
			require('@tailwindcss/forms'),
			// plugin(({ addVariant }) => {
			// 	addVariant('open', '&[data-state="open"]');
			// 	addVariant('closed', '&[data-state="closed"]');
			// }),
			require('@headlessui/tailwindcss'),
			require('tailwindcss-radix')()
		]
	};
	if (app === 'landing') {
		config.plugins.push(require('@tailwindcss/typography'));
		config.plugins.push(require('@tailwindcss/line-clamp'));
	}
	return config;
};

// 	primary: {
// 		DEFAULT: '#2599FF',
// 		50: '#FFFFFF',
// 		100: '#F1F8FF',
// 		200: '#BEE1FF',
// 		300: '#8BC9FF',
// 		400: '#58B1FF',
// 		500: '#2599FF',
// 		600: '#0081F1',
// 		700: '#0065BE',
// 		800: '#004A8B',
// 		900: '#002F58'
// 	},
// 	gray: {
// 		DEFAULT: '#505468',
// 		50: '#F1F1F4',
// 		100: '#E8E9ED',
// 		150: '#E0E1E6',
// 		200: '#D8DAE3',
// 		250: '#D2D4DC',
// 		300: '#C0C2CE',
// 		350: '#A6AABF',
// 		400: '#9196A8',
// 		450: '#71758A',
// 		500: '#303544',
// 		550: '#20222d',
// 		600: '#171720',
// 		650: '#121219',
// 		700: '#121317',
// 		750: '#0D0E11',
// 		800: '#0C0C0F',
// 		850: '#08090D',
// 		900: '#060609',
// 		950: '#030303'
// 	}
// },
// fontFamily: { sans: ['Inter', ...defaultTheme.fontFamily.sans] }

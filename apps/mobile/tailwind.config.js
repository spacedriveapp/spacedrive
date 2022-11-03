// Extented colors are copied from packages/ui/style/colors.scss

module.exports = {
	content: ['./screens/**/*.{js,ts,jsx}', './components/**/*.{js,ts,jsx}', 'App.tsx'],
	theme: {
		// TODO: Needs some tweaking
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
					DEFAULT: 'hsla(208, 100%, 47%, 1)',
					faint: 'hsla(208, 100%, 64%, 1)',
					deep: 'hsla(208, 100%, 47%, 1)'
				},
				ink: {
					DEFAULT: 'hsla(230, 0%, 100%, 1)',
					dull: 'hsla(230, 10%, 70%, 1)',
					faint: 'hsla(230, 10%, 55%, 1)'
				},
				// 'sidebar' on desktop
				drawer: {
					DEFAULT: 'hsla(230, 15%, 7%, 1)',
					box: 'hsla(230, 15%, 16%, 1)',
					line: 'hsla(230, 15%, 23%, 1)',
					divider: 'hsla(230, 15%, 17%, 1)',
					button: 'hsla(230, 15%, 18%, 1)',
					selected: 'hsla(230, 15%, 24%, 1)',
					shade: 'hsla(230, 15%, 23%, 1)'
				},
				app: {
					DEFAULT: 'hsla(230, 15%, 14%, 1)',
					box: 'hsla(230, 15%, 19%, 1)',
					overlay: 'hsla(230, 17%, 18%, 1)',
					input: 'hsla(230, 15%, 20%, 1)',
					focus: 'hsla(230, 15%, 10%, 1)',
					line: 'hsla(230, 15%, 26%, 1)',
					divider: 'hsla(230, 15%, 5%, 1)',
					button: 'hsla(230, 15%, 23%, 1)',
					selected: 'hsla(230, 15%, 27%, 1)',
					hover: 'hsla(230, 15%, 25%, 1)',
					active: 'hsla(230, 15%, 30%, 1)',
					shade: 'hsla(230, 15%, 0%, 1)',
					frame: 'hsla(230, 15%, 25%, 1)'
				},
				menu: {
					DEFAULT: 'hsla(230, 25%, 5%, 1)',
					line: 'hsla(230, 15%, 7%, 1)',
					hover: 'hsla(230, 15%, 30%, 1)',
					selected: 'hsla(230, 5%, 30%, 1)',
					shade: 'hsla(230, 5%, 0%, 1)',
					ink: 'hsla(230, 5%, 100%, 1)',
					faint: 'hsla(230, 5%, 80%, 1)'
				},
				// legacy support
				primary: {
					DEFAULT: '#2599FF',
					50: '#FFFFFF',
					100: '#F1F8FF',
					200: '#BEE1FF',
					300: '#8BC9FF',
					400: '#58B1FF',
					500: '#2599FF',
					600: '#0081F1',
					700: '#0065BE',
					800: '#004A8B',
					900: '#002F58'
				},
				gray: {
					DEFAULT: '#505468',
					50: '#F1F1F4',
					100: '#E8E9ED',
					150: '#E0E1E6',
					200: '#D8DAE3',
					250: '#D2D4DC',
					300: '#C0C2CE',
					350: '#A6AABF',
					400: '#9196A8',
					450: '#71758A',
					500: '#303544',
					550: '#20222d',
					600: '#171720',
					650: '#121219',
					700: '#121317',
					750: '#0D0E11',
					800: '#0C0C0F',
					850: '#08090D',
					900: '#060609',
					950: '#030303'
				}
			},
			extend: {}
		}
	},
	variants: {
		extend: {}
	},
	plugins: []
};

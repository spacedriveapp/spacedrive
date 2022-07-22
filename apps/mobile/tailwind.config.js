module.exports = {
	content: ['./screens/**/*.{js,ts,jsx}', './components/**/*.{js,ts,jsx}', 'App.tsx'],
	theme: {
		fontSize: {
			'tiny': '.65rem',
			'xs': '.75rem',
			'sm': '.84rem',
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

// Extented colors are copied from packages/ui/style/colors.scss

module.exports = {
	content: ['./screens/**/*.{js,ts,jsx}', './components/**/*.{js,ts,jsx}', 'App.tsx'],
	theme: {
		extend: {
			colors: {
				// Brand blue
				accent: {
					DEFAULT: 'hsla(208, 100%, 47%, 1)',
					faint: 'hsla(208, 100%, 64%, 1)',
					deep: 'hsla(208, 100%, 47%, 1)'
				},
				ink: {
					DEFAULT: 'hsla(230, 0%, 100%, 1)',
					light: 'hsla(230, 0%, 82%, 1)',
					dull: 'hsla(230, 10%, 70%, 1)',
					faint: 'hsla(230, 10%, 55%, 1)'
				},
				// Brand gray
				app: {
					DEFAULT: 'hsla(230, 15%, 13%, 1)',
					// background (dark)
					box: 'hsla(230, 15%, 17%, 1)',
					darkBox: 'hsla(230, 15%, 7%, 1)',
					// foreground (light)
					overlay: 'hsla(230, 15%, 19%, 1)',
					// border
					line: 'hsla(230, 15%, 25%, 1)',
					darkLine: 'hsla(230, 15%, 7%, 1)',
					// 'selected' on desktop
					highlight: 'hsla(230, 15%, 27%, 1)',
					// shadow
					shade: 'hsla(230, 15%, 0%, 1)',
					// button
					button: 'hsla(230, 15%, 23%, 1)',
					// menu
					menu: 'hsla(230, 25%, 5%, 1)',
					50: 'hsla(230, 15%, 5%, 1)',
					100: 'hsla(230, 15%, 10%, 1)',
					150: 'hsla(230, 15%, 15%, 1)',
					200: 'hsla(230, 15%, 20%, 1)',
					250: 'hsla(230, 15%, 30%, 1)',
					300: 'hsla(230, 15%, 35%, 1)',
					350: 'hsla(230, 15%, 40%, 1)',
					450: 'hsla(230, 15%, 45%, 1)',
					500: 'hsla(230, 15%, 50%, 1)',
					550: 'hsla(230, 15%, 55%, 1)',
					600: 'hsla(230, 15%, 60%, 1)',
					650: 'hsla(230, 15%, 65%, 1)',
					700: 'hsla(230, 15%, 70%, 1)',
					750: 'hsla(230, 15%, 75%, 1)',
					800: 'hsla(230, 15%, 80%, 1)',
					850: 'hsla(230, 15%, 85%, 1)',
					900: 'hsla(230, 15%, 90%, 1)',
					950: 'hsla(230, 15%, 95%, 1)',
					1000: 'hsla(230, 15%, 100%, 1)'
				},
				sidebar: {
					box: 'hsla(230, 15%, 16%, 1)',
					line: 'hsla(230, 15%, 23%, 1)',
					button: 'hsla(230, 15%, 18%, 1)'
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

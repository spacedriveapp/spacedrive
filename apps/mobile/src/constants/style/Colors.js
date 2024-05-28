// We can override these values if needed (like desktop).
const LIGHT_HUE = 235;
const DARK_HUE = 235;
const ALPHA = 1;

const nonThemeColors = {
	black: `hsla(0, 0%, 0%, ${ALPHA})`,
	white: `hsla(0, 0%, 100%, ${ALPHA})`,
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
};

module.exports = {
	dark: {
		...nonThemeColors,
		// accent theme colors
		accent: {
			DEFAULT: `hsla(208, 100%, 57%, ${ALPHA})`,
			faint: `hsla(208, 100%, 64%, ${ALPHA})`,
			deep: `hsla(208, 100%, 47%, ${ALPHA})`
		},
		// text
		ink: {
			DEFAULT: `hsla(${DARK_HUE}, 0%, 100%, ${ALPHA})`,
			light: `hsla(${DARK_HUE}, 0%, 82%, ${ALPHA})`,
			dull: `hsla(${DARK_HUE}, 10%, 70%, ${ALPHA})`,
			faint: `hsla(${DARK_HUE}, 10%, 55%, ${ALPHA})`
		},
		app: {
			DEFAULT: `hsla(${DARK_HUE}, 15%, 13%, ${ALPHA})`,
			//BG colors for elements
			header: `hsla(${DARK_HUE}, 10%, 6%, ${ALPHA})`,
			screen: `hsla(${DARK_HUE}, 15%, 12%, ${ALPHA})`,
			navtab: `hsla(${DARK_HUE}, 10%, 6%, ${ALPHA})`,
			card: `hsla(${DARK_HUE}, 10%, 5%, ${ALPHA})`,
			divider: `hsla(${DARK_HUE}, 10%, 16%, ${ALPHA})`,
			input: `hsla(${DARK_HUE}, 10%, 10%, ${ALPHA})`,
			//a lighter version of card bg
			boxLight: `hsla(${DARK_HUE}, 10%, 10%, ${ALPHA})`,
			//default button variant
			button: `hsla(${DARK_HUE}, 10%, 14%, ${ALPHA})`,
			//used with 'pills'
			highlight: `hsla(${DARK_HUE}, 10%, 16%, ${ALPHA})`,
			//Modal background color
			modal: `hsla(${DARK_HUE}, 10%, 7%, ${ALPHA})`,
			//Borders
			cardborder: `hsla(${DARK_HUE}, 10%, 10%, ${ALPHA})`,
			inputborder: `hsla(${DARK_HUE}, 10%, 16%, ${ALPHA})`,
			lightborder: `hsla(${DARK_HUE}, 10%, 20%, ${ALPHA})`,
			iconborder: `hsla(${DARK_HUE}, 10%, 100%, ${ALPHA})`,
			// background (dark)
			box: `hsla(${DARK_HUE}, 15%, 18%, ${ALPHA})`,
			darkBox: `hsla(${DARK_HUE}, 10%, 7%, ${ALPHA})`,
			// foreground (light)
			overlay: `hsla(${DARK_HUE}, 15%, 17%, ${ALPHA})`,
			// border
			line: `hsla(${DARK_HUE}, 15%, 25%, ${ALPHA})`,
			darkLine: `hsla(${DARK_HUE}, 15%, 7%, ${ALPHA})`,
			// shadow
			shade: `hsla(${DARK_HUE}, 15%, 0%, ${ALPHA})`,
			// menu
			menu: `hsla(${DARK_HUE}, 10%, 5%, ${ALPHA})`
		},
		sidebar: {
			box: `hsla(${DARK_HUE}, 15%, 16%, ${ALPHA})`,
			line: `hsla(${DARK_HUE}, 15%, 23%, ${ALPHA})`,
			button: `hsla(${DARK_HUE}, 15%, 18%, ${ALPHA})`
		}
	},
	vanilla: {
		...nonThemeColors,
		// accent theme colors
		accent: {
			DEFAULT: `hsla(208, 100%, 57%, ${ALPHA})`,
			faint: `hsla(208, 100%, 67%, ${ALPHA})`,
			deep: `hsla(208, 100%, 47%, ${ALPHA})`
		},
		// text
		ink: {
			DEFAULT: `hsla(${LIGHT_HUE}, 5%, 20%, ${ALPHA})`,
			dull: `hsla(${LIGHT_HUE}, 5%, 30%, ${ALPHA})`,
			faint: `hsla(${LIGHT_HUE}, 5%, 40%, ${ALPHA})`,
			// TODO:
			light: `hsla(${LIGHT_HUE}, 0%, 82%, ${ALPHA})`
		},
		app: {
			DEFAULT: `hsla(${LIGHT_HUE}, 5%, 100%, ${ALPHA})`,
			// background (dark)
			box: `hsla(${LIGHT_HUE}, 5%, 98%, ${ALPHA})`,
			darkBox: `hsla(${LIGHT_HUE}, 5%, 100%, ${ALPHA})`,
			// foreground (light)
			overlay: `hsla(${LIGHT_HUE}, 5%, 98%, ${ALPHA})`,
			// border
			line: `hsla(${LIGHT_HUE}, 5%, 90%, ${ALPHA})`,
			// TODO:
			darkLine: `hsla(${LIGHT_HUE}, 15%, 7%, ${ALPHA})`,
			// `selected` on desktop
			highlight: `hsla(${LIGHT_HUE}, 5%, 93%, ${ALPHA})`,
			// shadow
			shade: `hsla(${LIGHT_HUE}, 15%, 50%, ${ALPHA})`,
			// button
			button: `hsla(${LIGHT_HUE}, 5%, 100%, ${ALPHA})`,
			// menu
			menu: `hsla(${LIGHT_HUE}, 5%, 100%, ${ALPHA})`,
			// input
			input: `hsla(${LIGHT_HUE}, 5%, 100%, ${ALPHA})`
		},
		sidebar: {
			box: `hsla(${LIGHT_HUE}, 5%, 100%, ${ALPHA})`,
			line: `hsla(${LIGHT_HUE}, 10%, 85%, ${ALPHA})`,
			button: `hsla(${LIGHT_HUE}, 15%, 100%, ${ALPHA})`
		}
	}
};

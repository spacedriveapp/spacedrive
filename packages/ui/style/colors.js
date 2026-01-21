/**
 * Spacedrive V2 Color System
 *
 * Shared color values used across all platforms:
 * - Desktop (via CSS variables)
 * - Mobile (directly in Tailwind config)
 *
 * Colors are defined as HSL values for consistency.
 */

const colors = {
	// Accent color
	accent: {
		DEFAULT: '208, 100%, 57%',
		faint: '208, 100%, 64%',
		deep: '208, 100%, 47%',
	},

	// Text colors (ink)
	ink: {
		DEFAULT: '235, 15%, 92%',
		dull: '235, 10%, 70%',
		faint: '235, 10%, 55%',
	},

	// Sidebar colors
	sidebar: {
		DEFAULT: '235, 15%, 7%',
		box: '235, 15%, 16%',
		line: '235, 15%, 23%',
		ink: '235, 15%, 92%',
		inkDull: '235, 10%, 70%',
		inkFaint: '235, 10%, 55%',
		divider: '235, 15%, 17%',
		button: '235, 15%, 18%',
		selected: '235, 15%, 24%',
	},

	// Main app colors
	app: {
		DEFAULT: '235, 15%, 13%',
		box: '235, 15%, 18%',
		darkBox: '235, 10%, 7%',
		overlay: '235, 15%, 16%',
		line: '235, 15%, 23%',
		frame: '235, 15%, 25%',
		button: '235, 15%, 20%',
		hover: '235, 15%, 22%',
		selected: '235, 15%, 24%',
	},

	// Menu colors (dropdowns, context menus)
	menu: {
		DEFAULT: '235, 15%, 13%',
		line: '235, 15%, 23%',
		hover: '235, 15%, 20%',
		selected: '235, 15%, 24%',
		shade: '235, 15%, 8%',
		ink: '235, 15%, 92%',
		faint: '235, 10%, 55%',
	},
};

module.exports = colors;

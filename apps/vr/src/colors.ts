/**
 * Spacedrive color scheme for VR
 * Converted from packages/ui/style/colors.scss
 */

// Helper to convert HSL to RGB hex
function hslToHex(h: number, s: number, l: number): string {
	l /= 100;
	const a = (s * Math.min(l, 1 - l)) / 100;
	const f = (n: number) => {
		const k = (n + h / 30) % 12;
		const color = l - a * Math.max(Math.min(k - 3, 9 - k, 1), -1);
		return Math.round(255 * color)
			.toString(16)
			.padStart(2, "0");
	};
	return `#${f(0)}${f(8)}${f(4)}`;
}

const DARK_HUE = 235;

export const colors = {
	// Global
	black: "#000000",
	white: "#ffffff",

	// Accent
	accent: hslToHex(208, 100, 57), // #1da1f2
	accentFaint: hslToHex(208, 100, 64),
	accentDeep: hslToHex(208, 100, 47),

	// Text/Ink
	ink: hslToHex(DARK_HUE, 35, 92),
	inkDull: hslToHex(DARK_HUE, 10, 70),
	inkFaint: hslToHex(DARK_HUE, 10, 55),

	// Sidebar
	sidebar: hslToHex(DARK_HUE, 15, 7),
	sidebarBox: hslToHex(DARK_HUE, 15, 16),
	sidebarLine: hslToHex(DARK_HUE, 15, 23),
	sidebarInk: hslToHex(DARK_HUE, 15, 92),
	sidebarInkDull: hslToHex(DARK_HUE, 10, 70),
	sidebarInkFaint: hslToHex(DARK_HUE, 10, 55),
	sidebarDivider: hslToHex(DARK_HUE, 15, 17),
	sidebarButton: hslToHex(DARK_HUE, 15, 18),
	sidebarSelected: hslToHex(DARK_HUE, 15, 24),
	sidebarShade: hslToHex(DARK_HUE, 15, 23),

	// Main app
	app: hslToHex(DARK_HUE, 15, 13),
	appBox: hslToHex(DARK_HUE, 15, 18),
	appDarkBox: hslToHex(DARK_HUE, 15, 15),
	appDarkerBox: hslToHex(DARK_HUE, 16, 11),
	appLightBox: hslToHex(DARK_HUE, 15, 34),
	appOverlay: hslToHex(DARK_HUE, 15, 17),
	appInput: hslToHex(DARK_HUE, 15, 20),
	appFocus: hslToHex(DARK_HUE, 15, 10),
	appLine: hslToHex(DARK_HUE, 15, 23),
	appDivider: hslToHex(DARK_HUE, 15, 5),
	appButton: hslToHex(DARK_HUE, 15, 23),
	appHover: hslToHex(DARK_HUE, 15, 25),
	appSelected: hslToHex(DARK_HUE, 15, 26),
	appSelectedItem: hslToHex(DARK_HUE, 15, 18),
	appActive: hslToHex(DARK_HUE, 15, 30),
	appShade: hslToHex(DARK_HUE, 15, 0),
	appFrame: hslToHex(DARK_HUE, 15, 25),
	appSlider: hslToHex(DARK_HUE, 15, 20),

	// Menu
	menu: hslToHex(DARK_HUE, 15, 10),
	menuLine: hslToHex(DARK_HUE, 15, 14),
	menuInk: hslToHex(DARK_HUE, 25, 92),
	menuFaint: hslToHex(DARK_HUE, 5, 80),
	menuHover: hslToHex(DARK_HUE, 15, 30),
	menuSelected: hslToHex(DARK_HUE, 5, 30),
	menuShade: hslToHex(DARK_HUE, 5, 0),
};

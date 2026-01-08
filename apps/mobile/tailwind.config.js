const sharedColors = require("@sd/ui/style/colors");

/**
 * Convert shared color format (HSL string) to NativeWind format (hsl() function)
 * Shared: '235, 15%, 13%'
 * NativeWind: 'hsl(235, 15%, 13%)'
 *
 * Also converts camelCase keys to kebab-case for NativeWind compatibility
 */
function toHSL(colorValue) {
  if (typeof colorValue === "string") {
    return `hsl(${colorValue})`;
  }

  // Handle nested objects (like accent.DEFAULT)
  const result = {};
  for (const [key, value] of Object.entries(colorValue)) {
    // Preserve DEFAULT (must be uppercase for Tailwind)
    // Convert camelCase to kebab-case for everything else
    const kebabKey =
      key === "DEFAULT"
        ? key
        : key.replace(/([a-z])([A-Z])/g, "$1-$2").toLowerCase();
    result[kebabKey] = toHSL(value);
  }
  return result;
}

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{ts,tsx}", "./index.js"],
  presets: [require("nativewind/preset")],
  theme: {
    extend: {
      colors: {
        // Use shared colors from @sd/ui
        accent: toHSL(sharedColors.accent),
        ink: toHSL(sharedColors.ink),
        sidebar: toHSL(sharedColors.sidebar),
        app: toHSL(sharedColors.app),
        menu: toHSL(sharedColors.menu),
      },
      fontSize: {
        md: "16px",
      },
    },
  },
  plugins: [],
};

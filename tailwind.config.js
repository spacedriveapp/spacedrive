const colors = require('tailwindcss/colors');

module.exports = {
  purge: [
    './src/index.html',
    './src/**/*.{vue,js,ts,jsx,tsx}',
    './node_modules/@vechaiui/**/*.{js,ts,jsx,tsx}'
  ],
  darkMode: 'media',
  mode: 'jit',
  theme: {
    colors: {
      ...colors,
      gray: {
        ...colors.gray,
        800: '#2A2A37',
        900: '#24242F'
        // 100: '#F1EBEB'
      }
    }
  },
  variants: {
    extend: {
      // bg: { transparent: 'transparent' }
    }
  },
  plugins: [require('@tailwindcss/forms'), require('@vechaiui/core')]
};

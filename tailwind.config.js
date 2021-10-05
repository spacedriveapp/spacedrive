const colors = require('tailwindcss/colors');
const plugin = require('tailwindcss/plugin');
const defaultTheme = require('tailwindcss/defaultTheme');

module.exports = {
  purge: ['./src/index.html', './src/**/*.{vue,js,ts,jsx,tsx}'],
  darkMode: 'media',
  mode: 'jit',
  theme: {
    extend: {
      boxShadow: {
        box: '0px 4px 9px rgba(0, 0, 0, 0.05)',
        backdrop: '0px 4px 66px rgba(0, 0, 0, 0.08)'
      },
      bg: {
        funky: 'linear-gradient(90.63deg,#46bcff 12.1%,#85edfb 50.85%,#e04cf8 91.09%)'
      },
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
          200: '#C0C2CE',
          300: '#979CAF',
          400: '#6F7590',
          500: '#505468',
          600: '#434656',
          700: '#353845',
          800: '#282A34',
          900: '#1B1C23'
        }
      },
      fontFamily: { sans: ['Inter', ...defaultTheme.fontFamily.sans] }
    }
  },
  variants: {
    extend: {}
  },
  plugins: [require('@tailwindcss/forms')]
};

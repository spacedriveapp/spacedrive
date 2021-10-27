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
          150: '#E0E1E6',
          200: '#D8DAE3',
          250: '#D2D4DC',
          300: '#C0C2CE',
          350: '#A6AABF',
          400: '#9196A8',
          450: '#71758A',
          500: '#505468',
          550: '#434656',
          600: '#323846',
          650: '#222630',
          700: '#1C202A',
          750: '#282A34',
          800: '#1C1E26',
          850: '#30303E',
          900: '#15171F',
          950: '#12141A'
        }
      }
      // fontFamily: { sans: ['Inter', ...defaultTheme.fontFamily.sans] }
    }
  },
  variants: {
    extend: {}
  },
  plugins: []
};

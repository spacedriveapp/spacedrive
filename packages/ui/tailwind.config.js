// const colors = require('tailwindcss/colors');
// const plugin = require('tailwindcss/plugin');
// const defaultTheme = require('tailwindcss/defaultTheme');

module.exports = require('./style/tailwind')();

module.exports = {
    theme: {
        extend: {
            zIndex: {
                '55': '55',
            }
        }
    }
}

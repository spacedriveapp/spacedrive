export default {
	plugins: {
		'tailwindcss': {},
		'autoprefixer': {},
		'postcss-pseudo-companion-classes': {
			prefix: 'sb-pseudo--',
			restrictTo: [':hover', ':focus']
		}
	}
};

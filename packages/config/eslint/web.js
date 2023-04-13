module.exports = {
	extends: [require.resolve('./tailwind.js')],
	ignorePatterns: ['public', 'vite.config.ts'],
	env: {
		browser: true,
		node: true
	}
};

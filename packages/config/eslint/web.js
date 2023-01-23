module.exports = {
	extends: [require.resolve('./base.js')],
	ignorePatterns: ['public', 'vite.config.ts'],
	env: {
		browser: true,
		node: true
	}
};

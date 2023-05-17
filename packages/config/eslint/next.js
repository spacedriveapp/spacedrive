module.exports = {
	extends: [
		require.resolve('./base.js'),
		require.resolve('./tailwind.js'),
		'next/core-web-vitals',
		'next'
	],
	ignorePatterns: [
		'dist',
		'**/*.js',
		'**/*.json',
		'node_modules',
		'.contentlayer',
		'.next',
		'posts'
	]
};

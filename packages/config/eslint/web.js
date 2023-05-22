module.exports = {
	extends: [require.resolve('./base.js'), require.resolve('./tailwind.js')],
	ignorePatterns: ['public', 'vite.config.ts'],
	env: {
		browser: true,
		node: true
	},
	rules: {
		'no-restricted-syntax': [
			'error',
			{
				selector: "CallExpression[callee.name='useParams']",
				message: 'useParams is illegal, use useZodRouteParams!'
			},
			{
				selector: "CallExpression[callee.name='useSearchParams']",
				message: 'useSearchParams is illegal, use useZodSearchParams!'
			}
		]
	}
};

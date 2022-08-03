module.exports = {
	root: true,
	parser: '@typescript-eslint/parser',
	parserOptions: {
		project: [
			'apps/desktop/tsconfig.json',
			'apps/web/tsconfig.json',
			'apps/landing/tsconfig.json',
			'packages/client/tsconfig.json',
			'packages/interface/tsconfig.json',
			'packages/ui/tsconfig.json'
		]
	},
	plugins: ['@typescript-eslint'],
	extends: ['standard-with-typescript', 'prettier'],
	rules: {
		'@typescript-eslint/explicit-function-return-type': 'off'
	}
};

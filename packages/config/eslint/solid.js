module.exports = {
	plugins: ['solid'],
	overrides: [
		{
			files: ['**/*.solid.tsx'],
			extends: ['plugin:solid/recommended'],
			rules: {
				'solid/reactivity': 'warn',
				'solid/no-destructure': 'warn',
				'solid/jsx-no-undef': 'error'
			}
		}
	]
};

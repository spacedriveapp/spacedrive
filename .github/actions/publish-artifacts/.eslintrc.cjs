module.exports = {
	extends: [require.resolve('@sd/config/eslint/base.js')],
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	ignorePatterns: ['dist/**/*']
};

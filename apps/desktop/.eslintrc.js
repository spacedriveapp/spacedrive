module.exports = {
	extends: [require.resolve('@sd/config/eslint/web.js')],
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	}
};

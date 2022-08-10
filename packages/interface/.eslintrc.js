module.exports = {
	...require('@sd/config/eslint-react.js'),
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	}
};

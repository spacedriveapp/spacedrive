module.exports = {
	...require('@sd/config/eslint-react-native.js'),
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	}
};

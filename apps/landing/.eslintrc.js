module.exports = {
	...require('@sd/config/eslint-react.js'),
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	ignorePatterns: ['**/*.js', '**/*.json', 'node_modules', 'public', 'dist', 'vite.config.ts']
};

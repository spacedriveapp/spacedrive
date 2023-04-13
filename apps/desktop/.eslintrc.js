module.exports = {
	extends: [require.resolve('@sd/config/eslint/web.js')],
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	settings: {
		tailwindcss: {
			config: './tailwind.config.js'
		}
	}
};

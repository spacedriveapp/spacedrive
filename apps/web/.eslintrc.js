module.exports = {
	extends: [require.resolve('@sd/config/eslint/web.js')],
	parserOptions: {
		tsconfigRootDir: __dirname,
		project: './tsconfig.json'
	},
	ignorePatterns: ['playwright.config.ts', 'tests/**/*'],
	settings: {
		tailwindcss: {
			config: './tailwind.config.js'
		}
	}
};

module.exports = {
	extends: ['plugin:react/recommended', 'plugin:react-hooks/recommended'],
	plugins: ['react'],
	rules: {
		'react/display-name': 'off',
		'react/prop-types': 'off',
		'react/no-unescaped-entities': 'off',
		'react/react-in-jsx-scope': 'off',
		'react-hooks/rules-of-hooks': 'warn',
		'react-hooks/exhaustive-deps': 'warn'
	},
	ignorePatterns: ['dist', '**/*.js', '**/*.json', '**/*.solid.tsx', 'node_modules'],
	settings: {
		react: {
			version: 'detect'
		}
	}
};

module.exports = {
	plugins: ['react'],
	overrides: [
		{
			files: ['**/*.tsx'],
			excludedFiles: '*.solid.tsx',
			extends: ['plugin:react/recommended', 'plugin:react-hooks/recommended'],
			rules: {
				'react/display-name': 'off',
				'react/prop-types': 'off',
				'react/no-unescaped-entities': 'off',
				'react/react-in-jsx-scope': 'off',
				'react-hooks/rules-of-hooks': 'warn',
				'react-hooks/exhaustive-deps': 'warn'
			}
		}
	],
	settings: {
		react: {
			version: 'detect'
		}
	}
};

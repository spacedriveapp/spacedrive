const path = require('node:path');
module.exports = {
    parser: '@typescript-eslint/parser',
    parserOptions: {
        ecmaFeatures: {
            jsx: true
        },
        ecmaVersion: 12,
        sourceType: 'module'
    },
    extends: [
        'eslint:recommended',
        'plugin:react/recommended',
        'plugin:react-hooks/recommended',
        'plugin:@typescript-eslint/recommended',
        'turbo',
        'prettier'
    ],
    plugins: ['react'],
    rules: {
        'react/display-name': 'off',
        'react/prop-types': 'off',
        'react/no-unescaped-entities': 'off',
        'react/react-in-jsx-scope': 'off',
        'react-hooks/rules-of-hooks': 'warn',
        'react-hooks/exhaustive-deps': 'warn',
        '@typescript-eslint/no-unused-vars': 'off',
        '@typescript-eslint/ban-ts-comment': 'off',
        '@typescript-eslint/no-explicit-any': 'off',
        '@typescript-eslint/no-var-requires': 'off',
        '@typescript-eslint/no-non-null-assertion': 'off',
        '@typescript-eslint/explicit-module-boundary-types': 'off',
        '@typescript-eslint/no-empty-interface': 'off',
        '@typescript-eslint/no-empty-function': 'off',
        '@typescript-eslint/ban-types': 'off',
        'no-control-regex': 'off',
        'no-mixed-spaces-and-tabs': ['warn', 'smart-tabs'],
        'turbo/no-undeclared-env-vars': [
            'error',
            {
                cwd: path.resolve(path.join(__dirname, '..', '..', '..'))
            }
        ]
    },
    ignorePatterns: ['dist', '**/*.js', '**/*.json', 'node_modules'],
    settings: {
        react: {
            version: 'detect'
        }
    }
};

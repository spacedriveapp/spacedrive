export default /** @type {import('prettier').Config} */ ({
	semi: true,
	plugins: ['@ianvs/prettier-plugin-sort-imports'],
	useTabs: true,
	endOfLine: 'lf',
	printWidth: 100,
	quoteProps: 'consistent',
	singleQuote: true,
	arrowParens: 'avoid',
	importOrder: [
		// Node.js built-in modules
		'<TYPES>^(node:)',
		'<TYPES>',
		'<TYPES>^[.]',
		'<BUILTIN_MODULES>',
		'',
		// Imports not matched by other special words or groups.
		'<THIRD_PARTY_MODULES>',
		'',
		// spacedrive packages
		'^@sd/(interface|client|ui)(/.*)?$',
		// internal packages
		'^@/',
		'^~/',
		'',
		// relative
		'^[../]',
		'^[.]',
		'^(?!.*[.]css$)[./].*$',
		'.css$',
		'^(?!.*[.]scss$)[./].*$',
		'.scss$'
	],
	trailingComma: 'none',
	bracketSameLine: false,
	importOrderParserPlugins: ['importAttributes'],
	importOrderTypeScriptVersion: '5.0.0',
	overrides: [
		{
			files: '*.ts',
			options: {
				importOrderParserPlugins: ['typescript', 'decorators', 'importAttributes']
			}
		},
		{
			files: ['*.d.ts', '*.d.mts', '*.d.cts'],
			options: {
				importOrderParserPlugins: [
					'["typescript", { "dts": true }]',
					'decorators',
					'importAttributes'
				]
			}
		},
		{
			files: '*.tsx',
			options: {
				importOrderParserPlugins: ['jsx', 'typescript', 'decorators', 'importAttributes']
			}
		},
		{
			files: '*.jsx',
			options: {
				importOrderParserPlugins: ['jsx', 'decorators', 'importAttributes']
			}
		}
	]
});

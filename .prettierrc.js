/**
 * {@type require('prettier').Config}
 */
module.exports = {
	useTabs: true,
	printWidth: 100,
	singleQuote: true,
	trailingComma: 'none',
	bracketSameLine: false,
	semi: true,
	quoteProps: 'consistent',
	importOrder: [
		// external packages
		'<THIRD_PARTY_MODULES>',
		// spacedrive packages
		'^@sd/(interface|client|ui)(/.*)?$',
		// internal packages
		'^@/',
		'^~/',
		'',
		// relative
		'^[../]',
		'^[./]'
	],
	importOrderParserPlugins: ['typescript', 'jsx', 'decorators-legacy'],
	importOrderTypeScriptVersion: '4.4.0',
	tailwindConfig: './packages/ui/tailwind.config.js',
	plugins: ['@ianvs/prettier-plugin-sort-imports', 'prettier-plugin-tailwindcss']
};

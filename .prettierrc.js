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
		'^([A-Za-z]|@[^s/])',
		// spacedrive packages
		'^@sd/(interface|client|ui)(/.*)?$',
		// this package
		'^~/',
		// relative
		'^\\.'
	],
	importOrderSortSpecifiers: true,
	importOrderParserPlugins: ['importAssertions', 'typescript', 'jsx'],
	pluginSearchDirs: false,
	plugins: ['@trivago/prettier-plugin-sort-imports', 'prettier-plugin-tailwindcss'],
	tailwindConfig: 'packages/ui/tailwind.config.js'
};

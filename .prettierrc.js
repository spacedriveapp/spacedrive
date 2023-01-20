module.exports = {
	pluginSearchDirs: ['.'],
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
	plugins: ['@trivago/prettier-plugin-sort-imports']
};

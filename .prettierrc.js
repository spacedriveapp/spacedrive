module.exports = {
	pluginSearchDirs: ['.'],
	useTabs: true,
	printWidth: 100,
	singleQuote: true,
	trailingComma: 'none',
	bracketSameLine: false,
	semi: true,
	quoteProps: 'consistent',
	importOrder: ['^\\w', '^@sd/(interface|client|ui)(/.*)?$', '^[\\./~]'],
	importOrderSortSpecifiers: true,
	plugins: ['@trivago/prettier-plugin-sort-imports']
};

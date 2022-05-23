var mainConfig = require('./.prettierrc.json');

module.exports = {
	...mainConfig,
	plugins: ['@trivago/prettier-plugin-sort-imports']
};

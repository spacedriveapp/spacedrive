const fs = require('fs/promises');
const path = require('path');

// only supports files rn
module.exports = {
	relativeAliasResolver: {
		find: /^(~\/.+)/,
		replacement: '$1',
		async customResolver(source, importer) {
			const [pkg] = importer.split('/src/');

			const [_, sourcePath] = source.split('~/');

			const absolutePath = `${pkg}/src/${sourcePath}`;

			const folderItems = await fs.readdir(path.join(absolutePath, '../'));

			const item = folderItems.find((i) => i.startsWith(sourcePath.split('/').at(-1)));

			const fullPath = absolutePath + path.extname(item);

			const stats = await fs.stat(fullPath);

			if (stats.isDirectory()) {
				const directoryItems = await fs.readdir(absolutePath + path.extname(item));

				const indexFile = directoryItems.find((i) => i.startsWith('index'));

				return `${absolutePath}/${indexFile}`;
			} else {
				return fullPath;
			}
		}
	}
};

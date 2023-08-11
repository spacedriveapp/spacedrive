import fs from 'fs/promises';
import path from 'path';
import { Alias } from 'vite';

const projectPath = path.resolve(__dirname, '../../../');
const pkgJsonCache = new Map();

const resolver: Alias = {
	find: /^(~\/.+)/,
	replacement: '$1',
	async customResolver(source, importer) {
		let root: null | string = null;

		const [_, sourcePath] = source.split('~/');

		const relativeImporter = importer!.replace(projectPath, '');

		if (relativeImporter.includes('/src/')) {
			const [pkg] = relativeImporter.split('/src/');

			root = `${projectPath}${pkg}/src`;
		} else {
			let parent = importer!;

			while (parent !== '/') {
				parent = path.dirname(parent);

				let hasPkgJson = pkgJsonCache.get(parent);

				if (hasPkgJson === undefined)
					try {
						await fs.stat(`${parent}/package.json`);
						pkgJsonCache.set(parent, (hasPkgJson = true));
					} catch {
						pkgJsonCache.set(parent, (hasPkgJson = false));
					}

				if (hasPkgJson) {
					root = parent;
					break;
				}
			}

			if (root === null)
				throw new Error(`Failed to resolve import path ${source} in file ${importer}`);
		}

		const absolutePath = `${root}/${sourcePath}`;

		const folderItems = await fs.readdir(path.join(absolutePath, '../'));

		const item = folderItems.find((i) => i.startsWith(sourcePath.split('/').at(-1)!))!;

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
};

export default resolver;

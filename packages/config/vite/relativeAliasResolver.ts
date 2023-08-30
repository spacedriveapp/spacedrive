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

		if (importer) importer = path.normalize(importer);

		// source is the path imported on typescript, which always use / as path separator
		const [_, sourcePath] = source.split('~/');

		const relativeImporter = importer?.replace(projectPath, '');
		if (relativeImporter && relativeImporter.includes(`${path.sep}src${path.sep}`)) {
			const [pkg] = relativeImporter.split(`${path.sep}src${path.sep}`);

			root = `${projectPath}${pkg}${path.sep}src`;
		} else if (importer) {
			const pathObj = path.parse(importer);

			let parent = pathObj.dir;
			while (parent !== pathObj.root) {
				parent = path.dirname(parent);

				let hasPkgJson = pkgJsonCache.get(parent);

				if (hasPkgJson === undefined)
					try {
						await fs.stat(`${parent}${path.sep}package.json`);
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
		} else {
			throw new Error(`Failed to resolve import path ${source} in file ${importer}`);
		}

		const absolutePath = `${root}${path.sep}${sourcePath}`;

		const folderItems = await fs.readdir(path.join(absolutePath, '..'));

		// sourcePath is derived from the path imported on typescript, which always use / as path separator
		const item = folderItems.find((i) => i.startsWith(sourcePath.split('/').at(-1)!))!;

		const fullPath = absolutePath + path.extname(item);

		const stats = await fs.stat(fullPath);

		if (stats.isDirectory()) {
			const directoryItems = await fs.readdir(absolutePath + path.extname(item));

			const indexFile = directoryItems.find((i) => i.startsWith('index'));

			return `${absolutePath}${path.sep}${indexFile}`;
		} else {
			return fullPath;
		}
	}
};

export default resolver;

#!/usr/bin/env node
import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { format as prettierFormat } from 'prettier';

import prettierConfig from '../../../.prettierrc.json' assert { type: 'json' };

/**
 * Make a friendly name from an svg filename
 *
 * @example `folder-light` => `FolderLight`
 * @example `folder-open` => `FolderOpen`
 * @param {string} iconName Icon name to convert
 */
function iconFriendlyName(iconName, delimeter = '-') {
	return iconName
		.split(delimeter)
		.map((seg) => seg.toLowerCase())
		.join('');
}

function iconBaseName(filePath) {
	return path.basename(filePath, path.extname(filePath))
}

async function exists(path) {
	try {
		await fs.access(path);
		return true;
	} catch {
		return false;
	}
}

(async function main() {
	const files = await fs.readdir('./packages/interface/src/assets/icons');
	const icons = files.filter((path) => path.endsWith('.svg'));

	const generatedCode = `\
${icons
	.map((path) => iconBaseName(path))
	.map((baseName) => `import { ReactComponent as ${iconFriendlyName(baseName)} } from './${baseName}.svg';`)
	.join('\n')}

export default {
${icons
	.map((path) => iconFriendlyName(iconBaseName(path)))
	.map((baseName) => `\t${iconFriendlyName(baseName)},`)
	.join('\n')}
};
`;

	const outPath = path.resolve('./packages/interface/src/assets/icons/index.ts');

	if (await exists(outPath)) {
		await fs.rm(outPath);
	}

	await fs.writeFile(outPath, prettierFormat( generatedCode, prettierConfig));
})();

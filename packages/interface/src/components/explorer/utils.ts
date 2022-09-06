import { ExplorerItem, File, FilePath } from '@sd/core';

export function isPath(item: ExplorerItem): item is FilePath & { type: 'Path' } {
	return item.type === 'Path';
}

export function isObject(item: ExplorerItem): item is File & { type: 'Object' } {
	return item.type === 'Object';
}

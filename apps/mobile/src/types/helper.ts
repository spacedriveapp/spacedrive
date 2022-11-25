import { ExplorerItem } from '@sd/client';

export type valueof<T> = T[keyof T];

export function isPath(item: ExplorerItem): item is Extract<ExplorerItem, { type: 'Path' }> {
	return item.type === 'Path';
}

export function isObject(item: ExplorerItem): item is Extract<ExplorerItem, { type: 'Object' }> {
	return item.type === 'Object';
}

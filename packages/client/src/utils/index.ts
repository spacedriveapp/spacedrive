import { ExplorerItem } from '../core';

export * from './objectKind';
export * from './formatBytes';
export * from './keys';

export function isPath(item: ExplorerItem): item is Extract<ExplorerItem, { type: 'Path' }> {
	return item.type === 'Path';
}

export function isObject(item: ExplorerItem): item is Extract<ExplorerItem, { type: 'Object' }> {
	return item.type === 'Object';
}

export function arraysEqual<T>(a: T[], b: T[]) {
	if (a === b) return true;
	if (a == null || b == null) return false;
	if (a.length !== b.length) return false;

	return a.every((n, i) => b[i] === n);
}

export function isKeyOf<T extends object>(obj: T, key: PropertyKey): key is keyof T {
	return key in obj;
}

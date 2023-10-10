import { ExplorerItem } from '../core';

export * from './objectKind';
export * from './explorerItem';
export * from './jobs';
// export * from './keys';

export function isPath(item: ExplorerItem): item is Extract<ExplorerItem, { type: 'Path' }> {
	return item.type === 'Path';
}

export function arraysEqual<T>(a: readonly T[], b: readonly T[]) {
	if (a === b) return true;
	if (a == null || b == null) return false;
	if (a.length !== b.length) return false;

	return a.every((n, i) => b[i] === n);
}

export function isKeyOf<T extends object>(obj: T, key: PropertyKey): key is keyof T {
	return key in obj;
}

// From: https://github.com/microsoft/TypeScript/issues/13298#issuecomment-885980381
// Warning: Avoid using the types bellow as a generic parameter, as it tanks the typechecker performance
export type UnionToIntersection<U> = (U extends never ? never : (arg: U) => never) extends (
	arg: infer I
) => void
	? I
	: never;

export type UnionToTuple<T> = UnionToIntersection<T extends never ? never : (t: T) => T> extends (
	_: never
) => infer W
	? [...UnionToTuple<Exclude<T, W>>, W]
	: [];

export function formatNumber(n: number) {
	if (!n) return '0';
	return Intl.NumberFormat().format(n);
}

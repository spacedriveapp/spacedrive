import { QueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';

import type { Object } from '..';
import type { ExplorerItem, FilePath, NonIndexedPathItem } from '../core';
import { LibraryConfigWrapped } from '../core';

export * from './jobs';

export const useItemsAsObjects = (items: ExplorerItem[]) => {
	return useMemo(() => {
		const array: Object[] = [];

		for (const item of items) {
			switch (item.type) {
				case 'Path': {
					if (!item.item.object) return [];
					array.push(item.item.object);
					break;
				}
				case 'Object': {
					array.push(item.item);
					break;
				}
				default:
					return [];
			}
		}

		return array;
	}, [items]);
};

export const useItemsAsFilePaths = (items: ExplorerItem[]) => {
	return useMemo(() => {
		const array: FilePath[] = [];

		for (const item of items) {
			switch (item.type) {
				case 'Path': {
					array.push(item.item);
					break;
				}
				case 'Object': {
					// this isn't good but it's the current behaviour
					const filePath = item.item.file_paths[0];
					if (filePath) array.push(filePath);
					else return [];

					break;
				}
				default:
					return [];
			}
		}

		return array;
	}, [items]);
};

export const useItemsAsEphemeralPaths = (items: ExplorerItem[]) => {
	return useMemo(() => {
		return items
			.filter((item) => item.type === 'NonIndexedPath')
			.map((item) => item.item as NonIndexedPathItem);
	}, [items]);
};

export function getItemObject(data: ExplorerItem) {
	return data.type === 'Object' ? data.item : data.type === 'Path' ? data.item.object : null;
}

export function getItemFilePath(data: ExplorerItem) {
	if (data.type === 'Path' || data.type === 'NonIndexedPath') return data.item;
	return (data.type === 'Object' && data.item.file_paths[0]) || null;
}

export function getEphemeralPath(data: ExplorerItem) {
	return data.type === 'NonIndexedPath' ? data.item : null;
}

export function getIndexedItemFilePath(data: ExplorerItem) {
	return data.type === 'Path'
		? data.item
		: data.type === 'Object'
			? data.item.file_paths[0] ?? null
			: null;
}

export function getItemLocation(data: ExplorerItem) {
	return data.type === 'Location' ? data.item : null;
}
export function getItemSpacedropPeer(data: ExplorerItem) {
	return data.type === 'SpacedropPeer' ? data.item : null;
}

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

export type UnionToTuple<T> =
	UnionToIntersection<T extends never ? never : (t: T) => T> extends (_: never) => infer W
		? [...UnionToTuple<Exclude<T, W>>, W]
		: [];

export function formatNumber(n: number) {
	if (!n) return '0';
	return Intl.NumberFormat().format(n);
}

export function insertLibrary(queryClient: QueryClient, library: LibraryConfigWrapped) {
	queryClient.setQueryData(['library.list'], (libraries: any) => {
		// The invalidation system beat us to it
		if (libraries.items.find((l: any) => l.__id === library.uuid)) return libraries;

		return {
			items: [
				...(libraries.items || []),
				{
					__type: 'LibraryConfigWrapped',
					__id: library.uuid
				}
			],
			nodes: [
				...(libraries.nodes || []),
				{
					__type: 'LibraryConfigWrapped',
					__id: library.uuid,
					...library
				}
			]
		};
	});
}

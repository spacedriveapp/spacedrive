import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { proxy, snapshot, subscribe, useSnapshot } from 'valtio';
import { z } from 'zod';
import type {
	ExplorerItem,
	ExplorerLayout,
	ExplorerSettings,
	FilePath,
	Location,
	NodeState,
	Tag
} from '@sd/client';
import { ObjectKindEnum, type Ordering, type OrderingKeys } from '@sd/client';

import { createDefaultExplorerSettings } from './store';
import { uniqueId } from './util';

export type ExplorerParent =
	| {
			type: 'Location';
			location: Location;
			subPath?: FilePath;
	  }
	| {
			type: 'Ephemeral';
			path: string;
	  }
	| {
			type: 'Tag';
			tag: Tag;
	  }
	| {
			type: 'Node';
			node: NodeState;
	  };

export interface UseExplorerProps<TOrder extends Ordering> {
	items: ExplorerItem[] | null;
	count?: number;
	parent?: ExplorerParent;
	loadMore?: () => void;
	isFetchingNextPage?: boolean;
	isLoadingPreferences?: boolean;
	scrollRef?: RefObject<HTMLDivElement>;
	overscan?: number;
	/**
	 * @defaultValue `true`
	 */
	selectable?: boolean;
	settings: ReturnType<typeof useExplorerSettings<TOrder, any>>;
	/**
	 * @defaultValue `true`
	 */
	showPathBar?: boolean;
	layouts?: Partial<Record<ExplorerLayout, boolean>>;
}

/**
 * Controls top-level config and state for the explorer.
 * View- and inspector-specific state is not handled here.
 */
export function useExplorer<TOrder extends Ordering>({
	settings,
	layouts,
	...props
}: UseExplorerProps<TOrder>) {
	const scrollRef = useRef<HTMLDivElement>(null);

	return {
		// Default values
		selectable: true,
		scrollRef,
		count: props.items?.length,
		showPathBar: true,
		layouts: {
			grid: true,
			list: true,
			media: true,
			...layouts
		},
		...settings,
		// Provided values
		...props,
		// Selected items
		...useSelectedItems(props.items)
	};
}

export type UseExplorer<TOrder extends Ordering> = ReturnType<typeof useExplorer<TOrder>>;

export function useExplorerSettings<TOrder extends Ordering, T>({
	settings,
	onSettingsChanged,
	orderingKeys,
	data
}: {
	settings: ReturnType<typeof createDefaultExplorerSettings<TOrder>>;
	onSettingsChanged?: (settings: ExplorerSettings<TOrder>, data: T) => void;
	orderingKeys?: z.ZodUnion<
		[z.ZodLiteral<OrderingKeys<TOrder>>, ...z.ZodLiteral<OrderingKeys<TOrder>>[]]
	>;
	data?: T | null;
}) {
	const [store] = useState(() => proxy(settings));

	const updateSettings = useDebouncedCallback((settings: ExplorerSettings<TOrder>, data: T) => {
		onSettingsChanged?.(settings, data);
	}, 500);

	useEffect(() => updateSettings.flush(), [data, updateSettings]);

	useEffect(() => {
		if (updateSettings.isPending()) return;
		Object.assign(store, settings);
	}, [settings, store, updateSettings]);

	useEffect(() => {
		if (!onSettingsChanged || !data) return;
		const unsubscribe = subscribe(store, () => {
			updateSettings(snapshot(store) as ExplorerSettings<TOrder>, data);
		});
		return () => unsubscribe();
	}, [store, updateSettings, data, onSettingsChanged]);

	return {
		useSettingsSnapshot: () => useSnapshot(store),
		useLayoutSearchFilters: () => {
			const explorerSettingsSnapshot = useSnapshot(store);
			return explorerSettingsSnapshot.layoutMode === 'media'
				? [{ object: { kind: { in: [ObjectKindEnum.Image, ObjectKindEnum.Video] } } }]
				: [];
		},
		settingsStore: store,
		orderingKeys
	};
}

export type UseExplorerSettings<TOrder extends Ordering, T> = ReturnType<
	typeof useExplorerSettings<TOrder, T>
>;

function useSelectedItems(items: ExplorerItem[] | null) {
	// Doing pointer lookups for hashes is a bit faster than assembling a bunch of strings
	// WeakMap ensures that ExplorerItems aren't held onto after they're evicted from cache
	const itemHashesWeakMap = useRef(new WeakMap<ExplorerItem, string>());

	// Store hashes of items instead as objects are unique by reference but we
	// still need to differentate between item variants
	const [selectedItemHashes, setSelectedItemHashes] = useState(() => ({
		value: new Set<string>()
	}));

	const updateHashes = useCallback(
		() => setSelectedItemHashes((h) => ({ ...h })),
		[setSelectedItemHashes]
	);

	const itemsMap = useMemo(
		() =>
			(items ?? []).reduce((items, item, i) => {
				const hash = itemHashesWeakMap.current.get(item) ?? uniqueId(item);
				itemHashesWeakMap.current.set(item, hash);
				items.set(hash, { index: i, data: item });
				return items;
			}, new Map<string, { index: number; data: ExplorerItem }>()),
		[items]
	);

	const selectedItems = useMemo(
		() =>
			[...selectedItemHashes.value].reduce((items, hash) => {
				const item = itemsMap.get(hash);
				if (item) items.add(item.data);
				return items;
			}, new Set<ExplorerItem>()),
		[itemsMap, selectedItemHashes]
	);

	const getItemUniqueId = useCallback(
		(item: ExplorerItem) => itemHashesWeakMap.current.get(item) ?? uniqueId(item),
		[]
	);

	return {
		itemsMap,
		selectedItems,
		selectedItemHashes,
		getItemUniqueId,
		addSelectedItem: useCallback(
			(item: ExplorerItem | ExplorerItem[]) => {
				const items = Array.isArray(item) ? item : [item];

				for (let i = 0; i < items.length; i++) {
					selectedItemHashes.value.add(getItemUniqueId(items[i]!));
				}

				updateHashes();
			},
			[getItemUniqueId, selectedItemHashes.value, updateHashes]
		),
		removeSelectedItem: useCallback(
			(item: ExplorerItem | ExplorerItem[]) => {
				const items = Array.isArray(item) ? item : [item];

				for (let i = 0; i < items.length; i++) {
					selectedItemHashes.value.delete(getItemUniqueId(items[i]!));
				}

				updateHashes();
			},
			[getItemUniqueId, selectedItemHashes.value, updateHashes]
		),
		resetSelectedItems: useCallback(
			(items?: ExplorerItem[]) => {
				selectedItemHashes.value.clear();
				items?.forEach((item) => selectedItemHashes.value.add(getItemUniqueId(item)));
				updateHashes();
			},
			[getItemUniqueId, selectedItemHashes.value, updateHashes]
		),
		isItemSelected: useCallback(
			(item: ExplorerItem) => selectedItems.has(item),
			[selectedItems]
		)
	};
}

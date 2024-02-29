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

import { createDefaultExplorerSettings, type Ordering, type OrderingKeys } from './store';
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
	settings: ReturnType<typeof useExplorerSettings<TOrder>>;
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

export function useExplorerSettings<TOrder extends Ordering>({
	settings,
	onSettingsChanged,
	orderingKeys,
	location
}: {
	settings: ReturnType<typeof createDefaultExplorerSettings<TOrder>>;
	onSettingsChanged?: (settings: ExplorerSettings<TOrder>, location: Location) => void;
	orderingKeys?: z.ZodUnion<
		[z.ZodLiteral<OrderingKeys<TOrder>>, ...z.ZodLiteral<OrderingKeys<TOrder>>[]]
	>;
	location?: Location | null;
}) {
	const [store] = useState(() => proxy(settings));

	const updateSettings = useDebouncedCallback(
		(settings: ExplorerSettings<TOrder>, location: Location) => {
			onSettingsChanged?.(settings, location);
		},
		500
	);

	useEffect(() => updateSettings.flush(), [location, updateSettings]);

	useEffect(() => {
		if (updateSettings.isPending()) return;
		Object.assign(store, settings);
	}, [settings, store, updateSettings]);

	useEffect(() => {
		if (!onSettingsChanged || !location) return;
		const unsubscribe = subscribe(store, () => {
			updateSettings(snapshot(store) as ExplorerSettings<TOrder>, location);
		});
		return () => unsubscribe();
	}, [store, updateSettings, location, onSettingsChanged]);

	return {
		useSettingsSnapshot: () => useSnapshot(store),
		settingsStore: store,
		orderingKeys
	};
}

export type UseExplorerSettings<TOrder extends Ordering> = ReturnType<
	typeof useExplorerSettings<TOrder>
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
			(items ?? []).reduce((items, item) => {
				const hash = itemHashesWeakMap.current.get(item) ?? uniqueId(item);
				itemHashesWeakMap.current.set(item, hash);
				items.set(hash, item);
				return items;
			}, new Map<string, ExplorerItem>()),
		[items]
	);

	const selectedItems = useMemo(
		() =>
			[...selectedItemHashes.value].reduce((items, hash) => {
				const item = itemsMap.get(hash);
				if (item) items.add(item);
				return items;
			}, new Set<ExplorerItem>()),
		[itemsMap, selectedItemHashes]
	);

	return {
		selectedItems,
		selectedItemHashes,
		addSelectedItem: useCallback(
			(item: ExplorerItem) => {
				selectedItemHashes.value.add(uniqueId(item));
				updateHashes();
			},
			[selectedItemHashes.value, updateHashes]
		),
		removeSelectedItem: useCallback(
			(item: ExplorerItem) => {
				selectedItemHashes.value.delete(uniqueId(item));
				updateHashes();
			},
			[selectedItemHashes.value, updateHashes]
		),
		resetSelectedItems: useCallback(
			(items?: ExplorerItem[]) => {
				selectedItemHashes.value.clear();
				items?.forEach((item) => selectedItemHashes.value.add(uniqueId(item)));
				updateHashes();
			},
			[selectedItemHashes.value, updateHashes]
		),
		isItemSelected: useCallback(
			(item: ExplorerItem) => selectedItems.has(item),
			[selectedItems]
		)
	};
}

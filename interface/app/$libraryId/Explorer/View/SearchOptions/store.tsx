import { useEffect, useMemo } from 'react';
import { proxy, useSnapshot } from 'valtio';
import { proxyMap } from 'valtio/utils';
import { Category, FilePathFilterArgs, ObjectFilterArgs, ObjectKindEnum } from '@sd/client';

import { inOrNotIn } from './util';

export enum FilterType {
	Location,
	Tag,
	Kind,
	Category,
	// FileContents,
	// Album,
	// Device,
	// Key,
	// Contact,
	CreatedAt,
	// ModifiedAt,
	// LastOpenedAt,
	// TakenAt,
	Hidden
}

export type SearchType = 'paths' | 'objects';
export type SearchScope = 'directory' | 'location' | 'device' | 'library';

export interface Filter {
	type: FilterType; // used to group filters
	value: string | any; // unique identifier or enum value, any allows for enum values that coerce to string
	name: string; // display name
	icon?: string; // "Folder" or "#efefef"
}

export interface SetFilter extends Filter {
	condition: boolean;
	canBeRemoved: boolean;
}

export interface GroupedFilters {
	type: FilterType;
	filters: SetFilter[];
}

const searchStore = proxy({
	isSearching: false,
	interactingWithSearchOptions: false,
	searchType: 'paths' as SearchType,
	searchQuery: null as string | null,
	registeredFilters: proxyMap() as Map<string, Filter>,
	selectedFilters: proxyMap() as Map<string, SetFilter>
});

export const useSearchFilters = <T extends SearchType>(
	searchType: T,
	fixedFilters?: Filter[]
): T extends 'objects' ? ObjectFilterArgs : FilePathFilterArgs => {
	const store = useSearchStore();

	searchStore.searchType = searchType;

	useEffect(() => {
		resetSearchStore();

		if (fixedFilters) {
			fixedFilters.forEach((filter) => {
				if (filter.name) {
					if (!filter.icon) filter.icon = filter.name;
					searchStore.registeredFilters.set(filter.value, filter);
					selectFilter(filter, true, false);
				}
				console.log(JSON.stringify(filter));
				console.log(searchStore.selectedFilters.values());
			});
		}

		return () => {
			resetSearchStore();
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [JSON.stringify(fixedFilters)]);

	const filters = useMemo(
		() => mapFiltersToQueryParams(Array.from(store.selectedFilters.values())),
		[store.selectedFilters]
	);

	useEffect(() => {
		console.log(store.searchQuery);
		if (store.searchQuery) {
			filters.queryParams.search = store.searchQuery;
		} else {
			delete filters.queryParams.search;
		}
	}, [filters.queryParams, store.searchQuery]);

	return searchType === 'objects' ? (filters.objectFilters as any) : (filters.queryParams as any);
};

export const useCreateFilter = (filters: Filter[]): (Filter & { key: string })[] => {
	const filtersWithKeys = filters.map((filter) => {
		const key = getKey(filter);
		return {
			...filter,
			key
		};
	});

	useEffect(() => {
		filtersWithKeys.forEach((filter) => {
			if (!searchStore.registeredFilters.has(filter.key)) {
				searchStore.registeredFilters.set(filter.key, filter);
			}
		});
	}, [filtersWithKeys]);

	return filtersWithKeys;
};

// key doesn't have to be a particular format, just needs to be unique
// this key is also handy for text filtering
export const getKey = (filter: Filter) => `${filter.type}-${filter.name}-${filter.value}`;

export const mapFiltersToQueryParams = (
	filters: SetFilter[]
): { queryParams: FilePathFilterArgs; objectFilters: ObjectFilterArgs } => {
	const queryParams: FilePathFilterArgs = {};
	const objectFilters: ObjectFilterArgs = {};

	filters.forEach((filter) => {
		switch (filter.type) {
			case FilterType.Location:
				queryParams.locations = inOrNotIn(
					queryParams.locations,
					parseInt(filter.value),
					filter.condition
				);
				break;

			case FilterType.Tag:
				objectFilters.tags = inOrNotIn(
					objectFilters.tags,
					parseInt(filter.value),
					filter.condition
				);
				break;

			case FilterType.Kind:
				objectFilters.kind = inOrNotIn(
					objectFilters.kind,
					parseInt(filter.value) as ObjectKindEnum,
					filter.condition
				);
				break;

			case FilterType.Category:
				objectFilters.category = inOrNotIn(
					objectFilters.category,
					filter.value as Category,
					filter.condition
				);
				break;

			case FilterType.Hidden:
				queryParams.hidden = filter.value === 'true';
				break;
		}
	});

	if (Object.keys(objectFilters).length > 0) {
		queryParams.object = objectFilters;
	}

	return { queryParams, objectFilters };
};

// return selected filters grouped by their type
export const getSelectedFiltersGrouped = (): GroupedFilters[] => {
	const groupedFilters: GroupedFilters[] = [];

	searchStore.selectedFilters.forEach((filter) => {
		const group = groupedFilters.find((group) => group.type === filter.type);
		if (group) {
			group.filters.push(filter);
		} else {
			groupedFilters.push({
				type: filter.type,
				filters: [filter]
			});
		}
	});

	return groupedFilters;
};

export const selectFilter = (filter: Filter, condition: boolean, canBeRemoved = true) => {
	const key = getKey(filter);
	searchStore.selectedFilters.set(key, {
		...filter,
		condition,
		canBeRemoved
	});
};

export const deselectFilter = (filter: Filter) => {
	const key = getKey(filter);
	searchStore.selectedFilters.delete(key);
};

export const resetSearchStore = () => {
	searchStore.searchQuery = null;
	searchStore.selectedFilters.clear();
};

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;

import { useEffect } from 'react';
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
	value: string; // unique identifier or enum value
	name: string; // display name
	icon: string; // "Folder" or "#efefef"
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
	searchQuery: null as string | null,
	searchScope: 'directory' as SearchScope,
	searchType: 'paths' as SearchType,
	registeredFilters: proxyMap() as Map<string, Filter>,
	selectedFilters: proxyMap() as Map<string, SetFilter>
});

// TODO: take props for fixed filters
export const useSearchFilters = () => {
	const store = useSearchStore();

	return mapFiltersToQueryParams(Array.from(store.selectedFilters.values()));
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

export const mapFiltersToQueryParams = (filters: SetFilter[]): FilePathFilterArgs => {
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
				queryParams.hidden = filter.condition;
				break;

			default:
				console.warn(`Unhandled filter type: ${filter.type}`);
		}
	});

	if (Object.keys(objectFilters).length > 0) {
		queryParams.object = objectFilters;
	}

	return queryParams;
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

export const selectFilter = (filter: Filter, condition: boolean) => {
	const key = getKey(filter);
	searchStore.selectedFilters.set(key, {
		...searchStore.registeredFilters.get(key)!,
		condition,
		canBeRemoved: true
	});
};

export const deselectFilter = (filter: Filter) => {
	const key = getKey(filter);
	searchStore.selectedFilters.delete(key);
};

export const resetSearchStore = () => {
	searchStore.searchQuery = null;
	searchStore.searchScope = 'directory';
	searchStore.searchType = 'paths';
	searchStore.selectedFilters.clear();
};

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;

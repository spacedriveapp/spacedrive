import { proxy, useSnapshot } from 'valtio';
import { SearchFilterArgs } from '@sd/client';
import { IconName } from '~/components/icons/Icon';

export type SearchFilters = 'locations' | 'tags' | 'name' | 'extension' | 'hidden' | 'kind';
export type SortOptionsType = {
	by:
		| 'none'
		| 'name'
		| 'sizeInBytes'
		| 'dateIndexed'
		| 'dateCreated'
		| 'dateModified'
		| 'dateAccessed'
		| 'dateTaken';
	direction: 'Asc' | 'Desc';
};

export interface FilterItem {
	id: number;
	name: string;
}

export interface TagItem {
	id: number;
	color: string;
}

export interface KindItem {
	id: number;
	name: string;
	icon: IconName;
}

export interface Filters {
	locations: FilterItem[];
	tags: TagItem[];
	name: string[];
	extension: string[];
	hidden: boolean;
	kind: KindItem[];
}

interface State {
	search: string;
	filters: Filters;
	sort: SortOptionsType;
	appliedFilters: Partial<Filters>;
	mergedFilters: SearchFilterArgs[];
	disableActionButtons: boolean;
}

const initialState: State = {
	search: '',
	filters: {
		locations: [],
		tags: [],
		name: [''],
		extension: [''],
		hidden: false,
		kind: []
	},
	sort: {
		by: 'none',
		direction: 'Asc'
	},
	appliedFilters: {},
	mergedFilters: [],
	disableActionButtons: true
};

// Utility function to safely update filter arrays or objects
function updateArrayOrObject<T>(
	array: T[],
	item: any,
	filterByKey: string = 'id',
	isObject: boolean = false
): T[] {
	if (isObject) {
		const index = (array as any).findIndex((i: any) => i.id === item[filterByKey]);
		if (index >= 0) {
			return array.filter((_, idx) => idx !== index);
		}
	} else {
		if (array.includes(item)) {
			return array.filter((i) => i !== item);
		}
	}
	return [...array, item];
}

const searchStore = proxy<
	State & {
		updateFilters: <K extends keyof State['filters']>(
			filter: K,
			value: State['filters'][K] extends Array<infer U> ? U : State['filters'][K],
			apply?: boolean,
			keepSame?: boolean
		) => void;
		searchFrom: (filter: 'tags' | 'locations', value: TagItem | FilterItem) => void;
		applyFilters: () => void;
		setSearch: (search: string) => void;
		resetFilter: <K extends keyof State['filters']>(filter: K, apply?: boolean) => void;
		resetFilters: () => void;
		setInput: (index: number, value: string, key: 'name' | 'extension') => void;
		addInput: (key: 'name' | 'extension') => void;
		removeInput: (index: number, key: 'name' | 'extension') => void;
	}
>({
	...initialState,
	//for updating the filters upon value selection
	updateFilters: (filter, value, apply = false) => {
		const currentFilter = searchStore.filters[filter];
		const arrayCheck = Array.isArray(currentFilter);

		if (filter === 'hidden') {
			// Directly assign boolean values without an array operation
			searchStore.filters['hidden'] = value as boolean;
		} else {
			// Handle array-based filters with more specific type handling
			if (arrayCheck) {
				// Cast to the correct type based on the filter being updated
				const updatedFilter = updateArrayOrObject(
					currentFilter,
					value,
					'id',
					typeof value === 'object'
				) as typeof currentFilter;
				searchStore.filters[filter] = updatedFilter;
			}
		}
		//instead of a useEffect or subscription - we can call applyFilters directly
		// useful when you want to apply the filters from another screen
		if (apply) searchStore.applyFilters();
	},
	searchFrom: (filter, value) => {
		//reset state first
		searchStore.resetFilters();
		//update the filter with the value
		switch (filter) {
			case 'locations':
				searchStore.filters[filter] = [value] as FilterItem[];
				break;
			case 'tags':
				searchStore.filters[filter] = [value] as TagItem[];
				break;
		}
		//apply the filters so it shows in the UI
		searchStore.applyFilters();
	},
	//for clicking add filters and applying the selection
	applyFilters: () => {
		// loop through all filters and apply the ones with values
		searchStore.appliedFilters = Object.entries(searchStore.filters).reduce(
			(acc, [key, value]) => {
				if (Array.isArray(value)) {
					const realValues = value.filter((v) => v !== '');
					if (realValues.length > 0) {
						acc[key as SearchFilters] = realValues;
					}
				} else if (typeof value === 'boolean') {
					// Only apply the hidden filter if it's true
					if (value) acc[key as SearchFilters] = value;
				}
				return acc;
			},
			{} as any
		);
	},
	setSearch: (search) => {
		searchStore.search = search;
	},
	resetFilter: (filter, apply = false) => {
		if (filter === 'name' || filter === 'extension') {
			searchStore.filters[(filter as 'name') || 'extension'] = [''];
		} else {
			searchStore.filters[filter] = initialState.filters[filter];
		}
		//instead of a useEffect or subscription - we can call applyFilters directly
		if (apply) searchStore.applyFilters();
	},
	resetFilters: () => {
		searchStore.filters = { ...initialState.filters };
	},
	setInput: (index, value, key) => {
		const newValues = [...searchStore.filters[key]];
		newValues[index] = value;
		searchStore.filters[key] = newValues;
	},
	//for adding more inputs to the name or extension filters
	addInput: (key) => {
		searchStore.filters[key] = [...searchStore.filters[key], ''];
	},
	//for removing inputs from the name or extension filters
	removeInput: (index, key) => {
		const filtered = searchStore.filters[key].filter((_, idx) => idx !== index);
		searchStore.filters[key] = filtered;
	}
});

/** for reading */
export const useSearchStore = () => useSnapshot(searchStore);
/** for writing */
export const getSearchStore = () => searchStore;

import { proxy, useSnapshot } from 'valtio';
import { IconName } from '~/components/icons/Icon';

export type SearchFilters = 'locations' | 'tags' | 'name' | 'extension' | 'hidden' | 'kind';

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

interface State {
	search: string;
	filters: {
		locations: FilterItem[];
		tags: TagItem[];
		name: string[];
		extension: string[];
		hidden: boolean;
		kind: KindItem[];
	};
	appliedFilters: Partial<
		Record<
			SearchFilters,
			{
				locations: FilterItem[];
				tags: TagItem[];
				name: string[];
				extension: string[];
				hidden: boolean;
				kind: KindItem[];
			}
		>
	>;
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
	appliedFilters: {},
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
			value: State['filters'][K] extends Array<infer U> ? U : State['filters'][K]
		) => void;
		applyFilters: () => void;
		setSearch: (search: string) => void;
		resetFilter: <K extends keyof State['filters']>(filter: K, apply?: boolean) => void;
		setInput: (index: number, value: string, key: 'name' | 'extension') => void;
		addInput: (key: 'name' | 'extension') => void;
		removeInput: (index: number, key: 'name' | 'extension') => void;
	}
>({
	...initialState,
	//for updating the filters upon value selection
	updateFilters: (filter, value) => {
		if (filter === 'hidden') {
			// Directly assign boolean values without an array operation
			searchStore.filters['hidden'] = value as boolean;
		} else {
			// Handle array-based filters with more specific type handling
			const currentFilter = searchStore.filters[filter];
			if (Array.isArray(currentFilter)) {
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
	},
	//for clicking add filters and applying the selection
	applyFilters: () => {
		// loop through all filters and apply the ones with values
		searchStore.appliedFilters = Object.entries(searchStore.filters).reduce(
			(acc, [key, value]) => {
				if (Array.isArray(value)) {
					if (value.length > 0 && value[0] !== '') {
						acc[key as SearchFilters] = value.filter((v) => v !== ''); // Remove empty values i.e empty inputs
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

export function useSearchStore() {
	return useSnapshot(searchStore);
}

export function getSearchStore() {
	return searchStore;
}

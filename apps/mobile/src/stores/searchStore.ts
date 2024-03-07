import { proxy, useSnapshot } from 'valtio';

export type SearchFilters = "locations" | "tags" | "name" | "extension" | "hidden" | "kind";

interface FilterItem {
    id: number;
    name: string;
}

interface TagItem {
    id: number;
    color: string;
}

interface State {
    search: string;
    filters: {
        locations: FilterItem[];
        tags: TagItem[];
        name: string[];
        extension: string[];
        hidden: boolean;
        kind: FilterItem[];
    };
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
    disableActionButtons: true
};

// Utility function to safely update filter arrays or objects
function updateArrayOrObject<T>(array: T[], item: any, filterByKey: string = 'id', isObject: boolean = false): T[] {
    if (isObject) {
        const index = (array as any).findIndex((i: any) => i.id === item[filterByKey]);
        if (index >= 0) {
            return array.filter((_, idx) => idx !== index);
        }
    } else {
        if (array.includes(item)) {
            return array.filter(i => i !== item);
        }
    }
    return [...array, item];
}

const searchStore = proxy<State & {
	updateFilters: <K extends keyof State['filters']>(
        filter: K,
        value: State['filters'][K] extends Array<infer U> ? U : State['filters'][K]
    ) => void;
	setSearch: (search: string) => void;
    resetFilter: <K extends keyof State['filters']>(filter: K) => void;
    setInput: (index: number, value: string, key: 'name' | 'extension') => void;
    addInput: (key: 'name' | 'extension') => void;
    removeInput: (index: number, key: 'name' | 'extension') => void;
}>({
    ...initialState,
	updateFilters: (filter, value) => {
        if (filter === 'hidden') {
            // Directly assign boolean values without an array operation
            searchStore.filters['hidden'] = value as boolean;
        } else {
            // Handle array-based filters with more specific type handling
            const currentFilter = searchStore.filters[filter];
            if (Array.isArray(currentFilter)) {
                // Cast to the correct type based on the filter being updated
                const updatedFilter = updateArrayOrObject(currentFilter, value, 'id', typeof value === 'object') as typeof currentFilter;
                searchStore.filters[filter] = updatedFilter;
            }
        }
    },
    setSearch: search => {
        searchStore.search = search;
    },
    resetFilter: filter => {
        if (filter === 'name' || filter === 'extension') {
            searchStore.filters[filter as 'name' || 'extension'] = [''];
        } else {
            searchStore.filters[filter] = initialState.filters[filter];
        }
    },
    setInput: (index, value, key) => {
        const newValues = [...searchStore.filters[key]];
        newValues[index] = value;
        searchStore.filters[key] = newValues;
    },
    addInput: key => {
        searchStore.filters[key] = [...searchStore.filters[key], ''];
    },
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

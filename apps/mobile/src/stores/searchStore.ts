import { proxy, useSnapshot } from 'valtio';

/**
 This is subject to change, but the idea is to have a global store for search
 that can be used by any component.

 once the new mobile design is implemented this is likely to change
 */

const searchStore = proxy({
	search: '',
	setSearch: (search: string) => {
		searchStore.search = search;
	}
});

export function useSearchStore() {
	return useSnapshot(searchStore);
}

export function getSearchStore() {
	return searchStore;
}

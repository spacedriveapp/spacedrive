import { proxy, useSnapshot } from 'valtio';

export type SearchType = 'paths' | 'objects';

const searchStore = proxy({
	interactingWithSearchOptions: false,
	searchType: 'paths' as SearchType,
	searchQuery: '' // Search query to track user input
	// Any other search-specific state can go here
});

// Hook to interact with the search store
export const useSearchStore = () => useSnapshot(searchStore);

// Function to set the search query
export const setSearchQuery = (query: string) => {
	searchStore.searchQuery = query;
};

// Function to reset search state (if needed)
export const resetSearchStore = () => {
	searchStore.interactingWithSearchOptions = false;
	searchStore.searchQuery = '';
};

// Function to retrieve the search store directly
export const getSearchStore = () => searchStore;

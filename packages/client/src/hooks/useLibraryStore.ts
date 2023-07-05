import { proxy, useSnapshot } from 'valtio';

const state = {
	onlineLocations: [] as number[][]
};

const libraryStore = proxy(state);

export function useLibraryStore() {
	return useSnapshot(libraryStore);
}

export function getLibraryStore() {
	return libraryStore;
}

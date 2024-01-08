import { createMutable } from 'solid-js/store';

import { useSolidStore } from '../solidjs-interop';

export const libraryStore = createMutable({
	onlineLocations: [] as number[][]
});

export function useLibraryStore() {
	return useSolidStore(libraryStore);
}

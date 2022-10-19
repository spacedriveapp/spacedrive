import { useBridgeQuery } from '@sd/client';
import { useMemo } from 'react';
import { useSnapshot } from 'valtio';
import proxyWithPersist, { PersistStrategy } from 'valtio-persist';
import { LibraryConfigWrapped } from '~/types/bindings';

import { StorageEngine } from './utils';

export const libraryStore = proxyWithPersist({
	initialState: {
		currentLibraryUuid: null as string | null,
		switchLibrary: (libraryUuid: string) => {
			libraryStore.currentLibraryUuid = libraryUuid;
			// Reset any other stores connected to library
		},
		initLibraries: async (libraries: LibraryConfigWrapped[]) => {
			// use first library default if none set
			if (!libraryStore.currentLibraryUuid) {
				libraryStore.currentLibraryUuid = libraries[0].uuid;
			}
		}
	},
	persistStrategies: PersistStrategy.SingleFile,
	name: 'sd-library-store',
	version: 0,
	migrations: {},
	getStorage: () => StorageEngine
});

export function useLibraryStore() {
	return useSnapshot(libraryStore);
}

export function getLibraryIdRaw(): string | null {
	return libraryStore.currentLibraryUuid;
}

// this must be used at least once in the app to correct the initial state
// is memorized and can be used safely in any component
export const useCurrentLibrary = () => {
	const store = useSnapshot(libraryStore);
	const { data: libraries } = useBridgeQuery(['library.list']);

	// memorize library to avoid re-running find function
	const currentLibrary = useMemo(() => {
		const current = libraries?.find((l: any) => l.uuid === store.currentLibraryUuid);
		// switch to first library if none set
		if (Array.isArray(libraries) && !current && libraries[0]?.uuid) {
			store.switchLibrary(libraries[0]?.uuid);
		}
		return current;
	}, [libraries, store]);

	return { currentLibrary, libraries, currentLibraryUuid: store.currentLibraryUuid };
};

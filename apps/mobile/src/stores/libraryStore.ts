import { mobileSync, useBridgeQuery } from '@sd/client';
import { useMemo } from 'react';
import { useSnapshot } from 'valtio';
import proxyWithPersist, { PersistStrategy } from 'valtio-persist';

import { StorageEngine } from './utils';

const libraryStore = proxyWithPersist({
	initialState: {
		currentLibraryUuid: null as string | null,
		switchLibrary: (libraryUuid: string) => {
			libraryStore.currentLibraryUuid = libraryUuid;
			// Reset any other stores connected to library

			// Sync with @sd/client
			mobileSync.id = libraryUuid;
		}
	},
	persistStrategies: PersistStrategy.SingleFile,
	name: 'sd-library-store',
	version: 0,
	migrations: {},
	getStorage: () => StorageEngine
});

export function getLibraryIdRaw(): string | null {
	return libraryStore.currentLibraryUuid;
}

export function useLibraryStore() {
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

	return {
		currentLibrary,
		libraries,
		switchLibrary: store.switchLibrary,
		isLoaded: store._persist.loaded
	};
}

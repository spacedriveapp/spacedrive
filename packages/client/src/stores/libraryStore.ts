import { LibraryConfigWrapped } from '@sd/core';
import { useMemo } from 'react';
import { proxy, useSnapshot } from 'valtio';
import { watch } from 'valtio/utils';

// import proxyWithPersist, { PersistStrategy } from 'valtio-persist';
import { useBridgeQuery } from '../index';
import { explorerStore } from './explorerStore';
import { storageEngine } from './util';

export const libraryStore = proxy({
	currentLibraryUuid: null as string | null
});
// export const libraryStore = proxyWithPersist({
// 	initialState: {
// 		currentLibraryUuid: null as string | null
// 	},
// 	persistStrategies: PersistStrategy.SingleFile,
// 	name: 'sd-library-store',
// 	version: 2,
// 	migrations: {},
// 	getStorage: () => storageEngine
// });

libraryStore.currentLibraryUuid = localStorage.getItem('sd-library-store');
const stop = watch((get) => {
	let uuid = get(libraryStore).currentLibraryUuid;
	if (uuid) localStorage.setItem('sd-library-store', uuid);
});

export function initLibraries(libraries: LibraryConfigWrapped[]) {
	// use first library default if none set
	if (!libraryStore.currentLibraryUuid) {
		libraryStore.currentLibraryUuid = libraries[0].uuid;
	}
}

export function switchLibrary(libraryUuid: string) {
	libraryStore.currentLibraryUuid = libraryUuid;
	explorerStore.reset();
}

// this must be used at least once in the app to correct the initial state
// is memorized and can be used safely in any component
export const useCurrentLibrary = () => {
	const store = useSnapshot(libraryStore);
	const { data: libraries } = useBridgeQuery(['library.get']);

	// memorize library to avoid re-running find function
	const currentLibrary = useMemo(() => {
		const current = libraries?.find((l: any) => l.uuid === store.currentLibraryUuid);
		// switch to first library if none set
		if (Array.isArray(libraries) && !current && libraries[0]?.uuid) {
			switchLibrary(libraries[0]?.uuid);
		}
		return current;
	}, [libraries, store.currentLibraryUuid]);

	return { currentLibrary, libraries, currentLibraryUuid: store.currentLibraryUuid };
};

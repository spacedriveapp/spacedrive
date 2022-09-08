import { useCallback, useMemo } from 'react';
import { proxy, useSnapshot } from 'valtio';

import { useBridgeQuery } from '../index';
import { explorerStore } from '../stores/explorerStore';

// The name of the localStorage key for caching library data
const libraryCacheLocalStorageKey = 'sd-library-list';

// Keep this private and use `useCurrentLibrary` hook to access or mutate it
const currentLibraryUuidStore = proxy({ id: null as string | null });

// this is a hook to get the current library loaded into the UI. It takes care of a bunch of invariants under the hood.
export const useCurrentLibrary = () => {
	const currentLibraryUuid = useSnapshot(currentLibraryUuidStore).id;
	const { data: libraries, isLoading } = useBridgeQuery(['library.get'], {
		keepPreviousData: true,
		initialData: () => {
			const cachedData = localStorage.getItem(libraryCacheLocalStorageKey);
			if (cachedData) {
				// If we fail to load cached data, it's fine
				try {
					return JSON.parse(cachedData);
				} catch (e) {
					console.error("Error loading cached 'sd-library-list' data", e);
				}
			}
			return undefined;
		},
		onSuccess: (data) => {
			localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(data));
		}
	});

	const switchLibrary = useCallback((libraryUuid: string) => {
		currentLibraryUuidStore.id = libraryUuid;
		explorerStore.reset();
	}, []);

	// memorize library to avoid re-running find function
	const library = useMemo(() => {
		const current = libraries?.find((l: any) => l.uuid === currentLibraryUuid);
		// switch to first library if none set
		if (libraries && !current && libraries[0]?.uuid) {
			switchLibrary(libraries[0]?.uuid);
		}
		return current;
	}, [libraries, currentLibraryUuid]); // TODO: This runs when the 'libraries' change causing the whole app to re-render which is cringe.

	// TODO: Redirect to onboarding flow if the user hasn't completed it. -> localStorage?

	return {
		library,
		libraries,
		isLoading,
		switchLibrary
	};
};

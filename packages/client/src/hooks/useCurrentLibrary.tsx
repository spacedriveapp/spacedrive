import { PropsWithChildren, createContext, useCallback, useContext, useMemo } from 'react';
import { proxy, subscribe, useSnapshot } from 'valtio';

import { getExplorerStore, useBridgeQuery } from '../index';

// The name of the localStorage key for caching library data
const libraryCacheLocalStorageKey = 'sd-library-list';

type OnNoLibraryFunc = () => void | Promise<void>;

// Keep this private and use `useCurrentLibrary` hook to access or mutate it
const currentLibraryUuidStore = proxy({ id: null as string | null });

// Cringe method to get rspc working on mobile.
export const mobileSync = currentLibraryUuidStore;

const CringeContext = createContext<{
	onNoLibrary: OnNoLibraryFunc;
}>(undefined!);

export const LibraryContextProvider = ({
	onNoLibrary,
	children
}: PropsWithChildren<{ onNoLibrary: OnNoLibraryFunc }>) => {
	return <CringeContext.Provider value={{ onNoLibrary }}>{children}</CringeContext.Provider>;
};

export function getLibraryIdRaw(): string | null {
	return currentLibraryUuidStore.id;
}

export function onLibraryChange(func: (newLibraryId: string | null) => void) {
	subscribe(currentLibraryUuidStore, () => func(currentLibraryUuidStore.id));
}

// this is a hook to get the current library loaded into the UI. It takes care of a bunch of invariants under the hood.
export const useCurrentLibrary = () => {
	const currentLibraryUuid = useSnapshot(currentLibraryUuidStore).id;
	const ctx = useContext(CringeContext);
	if (ctx === undefined)
		throw new Error(
			"The 'LibraryContextProvider' was not mounted and you attempted do use the 'useCurrentLibrary' hook. Please add the provider in your component tree."
		);
	const { data: libraries, isLoading } = useBridgeQuery(['library.list'], {
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

			// Redirect to the onboarding flow if the user doesn't have any libraries
			if (libraries?.length === 0) {
				ctx.onNoLibrary();
			}
		}
	});

	const switchLibrary = useCallback((libraryUuid: string) => {
		currentLibraryUuidStore.id = libraryUuid;
		getExplorerStore().reset();
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

	return {
		library,
		libraries,
		isLoading,
		switchLibrary
	};
};

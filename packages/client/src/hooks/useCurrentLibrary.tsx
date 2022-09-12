import {
	PropsWithChildren,
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useState
} from 'react';
import { proxy, useSnapshot } from 'valtio';

import { useBridgeQuery } from '../index';
import { explorerStore } from '../stores/explorerStore';

// The name of the localStorage key for caching library data
const libraryCacheLocalStorageKey = 'sd-library-list';

type OnNoLibraryFunc = () => void | Promise<void>;

// Keep this private and use `useCurrentLibrary` hook to access or mutate it
const CringeContext = createContext<{
	onNoLibrary: OnNoLibraryFunc;
	currentLibraryId: string | null;
	setCurrentLibraryId: (v: string | null) => void;
}>(undefined!);

export const LibraryContextProvider = ({
	onNoLibrary,
	children
}: PropsWithChildren<{ onNoLibrary: OnNoLibraryFunc }>) => {
	const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(null);

	return (
		<CringeContext.Provider value={{ onNoLibrary, currentLibraryId, setCurrentLibraryId }}>
			{children}
		</CringeContext.Provider>
	);
};

// this is a hook to get the current library loaded into the UI. It takes care of a bunch of invariants under the hood.
export const useCurrentLibrary = () => {
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

			// Redirect to the onboaording flow if the user doesn't have any libraries
			if (libraries?.length === 0) {
				ctx.onNoLibrary();
			}
		}
	});

	const switchLibrary = useCallback((libraryUuid: string) => {
		ctx.setCurrentLibraryId(libraryUuid);
		explorerStore.reset();
	}, []);

	// memorize library to avoid re-running find function
	const library = useMemo(() => {
		const current = libraries?.find((l: any) => l.uuid === ctx.currentLibraryId);
		// switch to first library if none set
		if (libraries && !current && libraries[0]?.uuid) {
			switchLibrary(libraries[0]?.uuid);
		}

		return current;
	}, [libraries, ctx.currentLibraryId]); // TODO: This runs when the 'libraries' change causing the whole app to re-render which is cringe.

	return {
		library,
		libraries,
		isLoading,
		switchLibrary
	};
};

import { createContext, PropsWithChildren, useContext, useEffect, useMemo } from 'react';

import { LibraryConfigWrapped } from '../core';
import { valtioPersist } from '../lib';
import { useBridgeQuery } from '../rspc';

// The name of the localStorage key for caching library data
const libraryCacheLocalStorageKey = 'sd-library-list';

export const useCachedLibraries = () => {
	const libraries = useBridgeQuery(['library.list'], {
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
		}
	});

	useEffect(() => {
		if (libraries.status === 'success') {
			localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(libraries.data));
		}
	}, [libraries.data, libraries.status]);

	return libraries;
};

export interface ClientContext {
	currentLibraryId: string | null;
	libraries: ReturnType<typeof useCachedLibraries>;
	library: LibraryConfigWrapped | null | undefined;
}

const ClientContext = createContext<ClientContext>(null!);

interface ClientContextProviderProps extends PropsWithChildren {
	currentLibraryId: string | null;
}

export const ClientContextProvider = ({
	children,
	currentLibraryId
}: ClientContextProviderProps) => {
	const libraries = useCachedLibraries();

	const library = useMemo(
		() => (libraries.data && libraries.data.find((l) => l.uuid === currentLibraryId)) || null,
		[currentLibraryId, libraries]
	);

	// Doesn't need to be in a useEffect
	currentLibraryCache.id = currentLibraryId;

	return (
		<ClientContext.Provider
			value={{
				currentLibraryId,
				libraries,
				library
			}}
		>
			{children}
		</ClientContext.Provider>
	);
};

export const useClientContext = () => {
	const ctx = useContext(ClientContext);

	if (ctx === undefined) throw new Error("'ClientContextProvider' not mounted");

	return ctx;
};

export const useCurrentLibraryId = () => useClientContext().currentLibraryId;

export const currentLibraryCache = valtioPersist('sd-current-library', {
	id: null as string | null
});

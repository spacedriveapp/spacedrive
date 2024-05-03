import { AlphaClient } from '@oscartbeaumont-sd/rspc-client/v2';
import { createContext, PropsWithChildren, useContext, useMemo } from 'react';

import { LibraryConfigWrapped, Procedures } from '../core';
import { valtioPersist } from '../lib';
import { useBridgeQuery } from '../rspc';

// The name of the localStorage key for caching library data
const libraryCacheLocalStorageKey = 'sd-library-list3'; // number is because the format of this underwent breaking changes

export const useCachedLibraries = () => {
	const result = useBridgeQuery(['library.list'], {
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
			if (data.length > 0)
				localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(data));
		}
	});

	return result;
};

export async function getCachedLibraries(client: AlphaClient<Procedures>) {
	const cachedData = localStorage.getItem(libraryCacheLocalStorageKey);

	const libraries = client.query(['library.list']).then((result) => {
		localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(result));
		return result;
	});

	if (cachedData) {
		// If we fail to load cached data, it's fine
		try {
			const data = JSON.parse(cachedData);
			return data as LibraryConfigWrapped[];
		} catch (e) {
			console.error("Error loading cached 'sd-library-list' data", e);
		}
	}

	return await libraries;
}

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

// million-ignore
export const useClientContext = () => {
	const ctx = useContext(ClientContext);

	if (ctx === undefined) throw new Error("'ClientContextProvider' not mounted");

	return ctx;
};

export const useCurrentLibraryId = () => useClientContext().currentLibraryId;

export const currentLibraryCache = valtioPersist('sd-current-library', {
	id: null as string | null
});

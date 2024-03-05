import { AlphaClient } from '@oscartbeaumont-sd/rspc-client/v2';
import { createContext, PropsWithChildren, useContext, useEffect, useMemo } from 'react';

import { NormalisedCache, useCache, useNodes } from '../cache';
import { LibraryConfigWrapped, Procedures } from '../core';
import { valtioPersist } from '../lib';
import { nonLibraryClient, useBridgeQuery } from '../rspc';

// The name of the localStorage key for caching library data
const libraryCacheLocalStorageKey = 'sd-library-list2'; // `2` is because the format of this underwent a breaking change when introducing normalised caching

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
			if (data.items.length > 0 || data.nodes.length > 0)
				localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(data));
		}
	});
	useNodes(result.data?.nodes);

	return {
		...result,
		data: useCache(result.data?.items)
	};
};

export async function getCachedLibraries(cache: NormalisedCache, client: AlphaClient<Procedures>) {
	const cachedData = localStorage.getItem(libraryCacheLocalStorageKey);

	const libraries =  client.query(['library.list']).then(result => {
		cache.withNodes(result.nodes);
		const libraries = cache.withCache(result.items);

		localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(result));

		return libraries;
	});

	if (cachedData) {
		// If we fail to load cached data, it's fine
		try {
			const data = JSON.parse(cachedData);
			cache.withNodes(data.nodes);
			return cache.withCache(data.items) as LibraryConfigWrapped[];
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

export const useClientContext = () => {
	const ctx = useContext(ClientContext);

	if (ctx === undefined) throw new Error("'ClientContextProvider' not mounted");

	return ctx;
};

export const useCurrentLibraryId = () => useClientContext().currentLibraryId;

export const currentLibraryCache = valtioPersist('sd-current-library', {
	id: null as string | null
});

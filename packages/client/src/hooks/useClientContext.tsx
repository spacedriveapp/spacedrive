import { useQueryClient } from '@tanstack/react-query';
import { createContext, PropsWithChildren, useContext, useEffect, useMemo, useState } from 'react';

import { NormalisedCache, useCache, useCacheContext } from '../cache';
import { LibraryConfigWrapped, NormalisedResults } from '../core';
import { valtioPersist } from '../lib';
import { nonLibraryClient } from '../rspc';

// The name of the localStorage key for caching library data
const libraryCacheLocalStorageKey = 'sd-library-list2'; // `2` is because the format of this underwent a breaking change when introducing normalised caching

export const useCachedLibraries = () => {
	const queryClient = useQueryClient();
	const cache = useCacheContext();
	const [state, setState] = useState(() => {
		const cachedData = localStorage.getItem(libraryCacheLocalStorageKey);
		let initialItems: NormalisedResults<LibraryConfigWrapped> | null = null;
		if (cachedData) {
			// If we fail to load cached data, it's fine
			try {
				initialItems = JSON.parse(cachedData);
			} catch (e) {
				console.error("Error loading cached 'sd-library-list' data", e);
			}
		}

		cache.withNodes(initialItems?.nodes);
		return {
			items: initialItems?.items,
			isLoading: false
		};
	});

	// We use `useCacheLibrary` high up in the React tree and React Query triggers a re-render for changes in loading state, updatedTime, etc. These are all properties we don't use.
	// This is a custom implementation of `useQuery` that only triggers a single re-render once the async data is loaded in.
	useEffect(
		() =>
			queryClient.getQueryCache().subscribe((event) => {
				if (
					event.type === 'observerResultsUpdated' &&
					// JS doesn't let us compare by reference so we have to do it like this
					event.query.queryKey.length === 1 &&
					event.query.queryKey[0] === 'library.list'
				) {
					const data: NormalisedResults<LibraryConfigWrapped> = event.query.state.data;

					cache.withNodes(data.nodes);
					setState({
						items: data.items,
						isLoading: event.query.state.status === 'loading'
					});
					localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(data));
				}
			}),
		[queryClient, cache]
	);

	return {
		isLoading: state.isLoading,
		data: useCache(state.items)
	};
};

export async function getCachedLibraries(cache: NormalisedCache) {
	const cachedData = localStorage.getItem(libraryCacheLocalStorageKey);

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

	const result = await nonLibraryClient.query(['library.list']);
	cache.withNodes(result.nodes);
	const libraries = cache.withCache(result.items);

	localStorage.setItem(libraryCacheLocalStorageKey, JSON.stringify(result));

	return libraries;
}

export interface ClientContext {
	currentLibraryId: string | null;
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
		[currentLibraryId, libraries.data]
	);

	// Doesn't need to be in a useEffect
	currentLibraryCache.id = currentLibraryId;

	return (
		<ClientContext.Provider
			value={{
				currentLibraryId,
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

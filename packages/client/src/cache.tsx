import { useQueryClient } from '@tanstack/react-query';
import {
	createContext,
	PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore
} from 'react';
import { proxy, snapshot, subscribe } from 'valtio';

import { type CacheNode } from './core';
import { getPermits } from './rspc-cursed';

declare global {
	interface Window {
		__REDUX_DEVTOOLS_EXTENSION__: any;
	}
}

type Store = ReturnType<typeof defaultStore>;
type Context = ReturnType<typeof createCache>;
export type NormalisedCache = ReturnType<typeof createCache>;

const defaultStore = () => ({
	nodes: {} as Record<string, Record<string, unknown>>
});

const Context = createContext<Context>(undefined!);

export function createCache() {
	const cache = proxy(defaultStore());
	return {
		cache,
		withNodes(data: CacheNode[] | undefined, suffix?: string) {
			updateNodes(cache, data, suffix);
		},
		withCache<T>(data: T | undefined, suffix?: string): UseCacheResult<T> {
			return restore(cache, new Map(), data, suffix) as any;
		}
	};
}

export function CacheProvider({ cache, children }: PropsWithChildren<{ cache: NormalisedCache }>) {
	useEffect(() => {
		if ('__REDUX_DEVTOOLS_EXTENSION__' in window === false) return;

		const devtools = window.__REDUX_DEVTOOLS_EXTENSION__.connect({});

		const unsub = devtools.subscribe((_message: any) => {
			// console.log(message);
		});

		devtools.init();
		subscribe(cache.cache, () => devtools.send('change', snapshot(cache.cache)));

		return () => {
			unsub();
			window.__REDUX_DEVTOOLS_EXTENSION__.disconnect();
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	const queryClient = useQueryClient();
	useEffect(() => {
		const interval = setInterval(() => {
			const permits = getPermits();
			if (permits !== 0) {
				console.warn('Not safe to cleanup cache. ${permits} permits currently held.');
				return;
			}

			const requiredKeys = new StableSet<[string, string]>();
			for (const query of queryClient.getQueryCache().getAll()) {
				if (query.state.data) scanDataForKeys(cache.cache, requiredKeys, query.state.data);
			}

			const existingKeys = new StableSet<[string, string]>();
			Object.entries(cache.cache.nodes).map(([type, value]) => {
				Object.keys(value).map((id) => existingKeys.add([type, id]));
			});

			for (const [type, id] of existingKeys.entries()) {
				// If key is not required. Eg. not in any query within the React Query cache.
				if (!requiredKeys.has([type, id])) {
					// Yeet the imposter
					delete cache.cache.nodes?.[type]?.[id];
				}
			}
		}, 60 * 1000);
		return () => clearInterval(interval);
	}, [cache.cache, queryClient]);

	return <Context.Provider value={cache}>{children}</Context.Provider>;
}

export function useCacheContext() {
	const context = useContext(Context);
	if (!context) throw new Error('Missing `CacheContext` provider!');
	return context;
}

function scanDataForKeys(cache: Store, keys: StableSet<[string, string]>, item: unknown) {
	if (item === undefined || item === null) return;
	if (Array.isArray(item)) {
		for (const v of item) {
			scanDataForKeys(cache, keys, v);
		}
	} else if (typeof item === 'object') {
		if ('__type' in item && '__id' in item) {
			if (typeof item.__type !== 'string') throw new Error('Invalid `__type`');
			if (typeof item.__id !== 'string') throw new Error('Invalid `__id`');
			keys.add([item.__type, item.__id]);
			const result = cache.nodes?.[item.__type]?.[item.__id];
			if (result) scanDataForKeys(cache, keys, result);
		}

		for (const [_k, value] of Object.entries(item)) {
			scanDataForKeys(cache, keys, value);
		}
	}
}

function restore(
	cache: Store,
	subscribed: Map<string, Set<unknown>>,
	item: unknown,
	suffix?: string
): unknown {
	if (item === undefined || item === null) {
		return item;
	} else if (Array.isArray(item)) {
		return item.map((v) => restore(cache, subscribed, v));
	} else if (typeof item === 'object') {
		if ('__type' in item && '__id' in item) {
			if (typeof item.__type !== 'string') throw new Error('Invalid `__type`');
			if (typeof item.__id !== 'string') throw new Error('Invalid `__id`');
			const ty = suffix ? `${suffix}:${item.__type}` : item.__type;

			const result = cache.nodes?.[ty]?.[item.__id];
			if (!result) throw new Error(`Missing node for id '${item.__id}' of type '${ty}'`);

			const v = subscribed.get(ty);
			if (v) {
				v.add(item.__id);
			} else {
				subscribed.set(ty, new Set([item.__id]));
			}

			// We call restore again for arrays and objects to deal with nested relations.
			return Object.fromEntries(
				Object.entries(result).map(([key, value]) => [
					key,
					restore(cache, subscribed, value)
				])
			);
		}

		return Object.fromEntries(
			Object.entries(item).map(([key, value]) => [key, restore(cache, subscribed, value)])
		);
	}

	return item;
}

export function useNodes(data: CacheNode[] | undefined) {
	const cache = useCacheContext();

	// `useMemo` instead of `useEffect` here is cursed but it needs to run before the `useMemo` in the `useCache` hook.
	useMemo(() => {
		updateNodes(cache.cache, data);
	}, [cache, data]);
}

// Methods to interact with the cache outside of the React lifecycle.
export function useNormalisedCache() {
	const cache = useCacheContext();

	return {
		'#cache': cache.cache,
		'withNodes': cache.withNodes,
		'withCache': cache.withCache
	};
}

function updateNodes(cache: Store, data: CacheNode[] | undefined, suffix?: string) {
	if (!data) return;

	for (const item of data) {
		if (!('__type' in item && '__id' in item)) throw new Error('Missing `__type` or `__id`');
		if (typeof item.__type !== 'string') throw new Error('Invalid `__type`');
		if (typeof item.__id !== 'string') throw new Error('Invalid `__id`');
		const ty = suffix ? `${suffix}:${item.__type}` : item.__type;

		const copy = { ...item } as any;
		delete copy.__type;
		delete copy.__id;

		const original = cache.nodes?.[ty]?.[item.__id];
		specialMerge(copy, original);

		if (!cache.nodes[ty]) cache.nodes[ty] = {};
		// TODO: This should be a deepmerge but that would break stuff like `size_in_bytes` or `inode` as the arrays are joined.
		cache.nodes[ty]![item.__id] = copy;
	}
}

// When using PCR's data structure if you don't fetch a relation `null` is returned.
// If two queries return a single entity but one fetches relations and the other doesn't that null might "win" over the actual data.
// Once it "wins" the normalised cache is updated causing all `useCache`'s to rerun.
//
// The `useCache` hook derives the type from  the specific React Query operation.
// Due to this the result of a `useCache` might end up as `null` even when TS says it's `T` causing crashes due to no-null checks.
//
// So this merge function causes the `null` to be replaced with the original value.
function specialMerge(copy: Record<any, any>, original: unknown) {
	if (
		original &&
		typeof original === 'object' &&
		typeof copy === 'object' &&
		!Array.isArray(original) &&
		!Array.isArray(copy)
	) {
		for (const [property, value] of Object.entries(original)) {
			copy[property] = copy[property] || value;

			if (typeof copy[property] === 'object' && !Array.isArray(copy[property]))
				specialMerge(copy[property], value);
		}
	}
}

export type UseCacheResult<T> = T extends (infer A)[]
	? UseCacheResult<A>[]
	: T extends object
		? T extends { '__type': any; '__id': string; '#type': infer U }
			? UseCacheResult<U>
			: { [K in keyof T]: UseCacheResult<T[K]> }
		: { [K in keyof T]: UseCacheResult<T[K]> };

export function useCache<T>(data: T | undefined) {
	const cache = useCacheContext();
	const subscribed = useRef(new Map<string, Set<unknown>>()).current;
	const [i, setI] = useState(0); // TODO: Remove this

	const state = useMemo(
		() => restore(cache.cache, subscribed, data) as UseCacheResult<T>,
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[cache, data, i]
	);

	return useSyncExternalStore(
		(onStoreChange) => {
			return subscribe(cache.cache, (ops) => {
				for (const [_, key] of ops) {
					const key_type = key[1] as string;
					const key_id = key[2] as string;

					const v = subscribed.get(key_type);
					if (v && v.has(key_id)) {
						setI((i) => i + 1);
						onStoreChange();

						break; // We only need to trigger re-render once so we can break
					}
				}
			});
		},
		() => state
	);
}

class StableSet<T> {
	set = new Set<string>();

	get size() {
		return this.set.size;
	}

	add(value: T) {
		this.set.add(JSON.stringify(value));
	}

	has(value: T) {
		return this.set.has(JSON.stringify(value));
	}

	*entries() {
		for (const v of this.set) {
			yield JSON.parse(v);
		}
	}
}

import {
	createContext,
	PropsWithChildren,
	useContext,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore
} from 'react';
import { proxy, subscribe } from 'valtio';

const defaultStore = {
	nodes: {} as Record<string, Record<string, unknown>>
} as const;

const Context = createContext<typeof defaultStore>(undefined!);

export function CacheProvider({ children }: PropsWithChildren) {
	const state = useRef(proxy(defaultStore)).current;
	return <Context.Provider value={state}>{children}</Context.Provider>;
}

export function useCacheContext() {
	const context = useContext(Context);
	if (!context) throw new Error('Missing `CacheContext` provider!');
	return context;
}

function restore(
	cache: typeof defaultStore,
	subscribed: Map<string, Set<unknown>>,
	item: unknown
): unknown {
	if (item === undefined || item === null) {
		return item;
	} else if (Array.isArray(item)) {
		return item.map((v) => restore(cache, subscribed, v));
	} else if (typeof item === 'object') {
		if ('__type' in item && '__id' in item) {
			if (typeof item.__type !== 'string') throw new Error('Invalid `__type`');
			if (typeof item.__id !== 'string') throw new Error('Invalid `__id`');
			const result = cache.nodes?.[item.__type]?.[item.__id];
			if (!result)
				throw new Error(`Missing node for id '${item.__id}' of type '${item.__type}'`);

			const v = subscribed.get(item.__type);
			if (v) {
				v.add(item.__id);
			} else {
				subscribed.set(item.__type, new Set([item.__id]));
			}

			return result;
		}

		return Object.fromEntries(
			Object.entries(item).map(([key, value]) => [key, restore(cache, subscribed, value)])
		);
	}

	return item;
}

type CacheNode = { '__type': string; '__id': string; '#node': any };

export function useNodes(data: CacheNode[] | undefined) {
	const cache = useCacheContext();

	// `useMemo` instead of `useEffect` here is cursed but it needs to run before the `useMemo` in the `useCache` hook.
	useMemo(() => {
		updateNodes(cache, data);
	}, [cache, data]);
}

export function useNodesCallback(): (data: CacheNode[] | undefined) => void {
	const cache = useCacheContext();

	return (data) => updateNodes(cache, data);
}

function updateNodes(cache: typeof defaultStore, data: CacheNode[] | undefined) {
	if (!data) return;

	for (const item of data) {
		if (!('__type' in item && '__id' in item)) throw new Error('Missing `__type` or `__id`');
		if (typeof item.__type !== 'string') throw new Error('Invalid `__type`');
		if (typeof item.__id !== 'string') throw new Error('Invalid `__id`');

		const copy = { ...item } as any;
		delete copy.__type;
		delete copy.__id;

		if (!cache.nodes[item.__type]) cache.nodes[item.__type] = {};
		cache.nodes[item.__type]![item.__id] = copy;
	}
}

type UseCacheResult<T> = T extends (infer A)[]
	? UseCacheResult<A>[]
	: T extends object
	? T extends { '__type': any; '__id': string; '#type': infer U }
		? U
		: { [K in keyof T]: UseCacheResult<T[K]> }
	: T;

export function useCache<T>(data: T | undefined) {
	const cache = useCacheContext();
	const subscribed = useRef(new Map<string, Set<unknown>>()).current;
	const [i, setI] = useState(0); // TODO: Remove this

	const state = useMemo(
		() => restore(cache, subscribed, data) as UseCacheResult<T>,
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[cache, data, i]
	);

	return useSyncExternalStore(
		(onStoreChange) => {
			return subscribe(cache, (ops) => {
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

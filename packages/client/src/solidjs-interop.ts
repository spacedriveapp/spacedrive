import { useEffect, useRef, useState } from 'react';
import { createReaction, createRoot } from 'solid-js';
import { type Store, type StoreNode } from 'solid-js/store';

export function useSolid<T>(fn: () => T) {
	const [, setCount] = useState(0);
	const solid = useRef(
		createRoot((dispose) => ({
			dispose,
			track: createReaction(() => setCount((c) => c + 1))
		}))
	);

	useEffect(() => solid.current.dispose, []);

	let rendering!: T;
	solid.current.track(() => (rendering = fn()));
	return rendering;
}

export function useSolidStore<T extends object = {}>(store: Store<T>) {
	return useSolid(() => ({ ...store }));
}

// `@solid-primitives/storage`'s `makePersisted` doesn't support `solid-js/store`'s `createMutable` so we roll our own.
export function createPersistedMutable<T extends StoreNode>(key: string, mutable: T) {
	try {
		const value = localStorage.getItem(key);
		if (value) {
			const persisted = JSON.parse(value);
			Object.assign(mutable, persisted);
		}
	} catch (err) {
		console.error(`Error loading persisted state from localStorage key '${key}': ${err}`);
	}

	return new Proxy(mutable, {
		get: (target, prop) => Reflect.get(target, prop),
		set: (target, prop, value) => {
			const result = Reflect.set(target, prop, value);
			localStorage.setItem(key, JSON.stringify(target));
			return result;
		}
	});
}

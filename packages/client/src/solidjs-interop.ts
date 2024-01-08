import { useObserver } from 'react-solid-state';
import { type Store, type StoreNode } from 'solid-js/store';

export function useSolidStore<T extends object = {}>(store: Store<T>) {
	return useObserver(() => ({ ...store }));
}

type CreatePersistedMutableOpts<T> = {
	onSave?: (value: T) => void;
};

// `@solid-primitives/storage`'s `makePersisted` doesn't support `solid-js/store`'s `createMutable` so we roll our own.
export function createPersistedMutable<T extends StoreNode>(
	key: string,
	mutable: T,
	opts?: CreatePersistedMutableOpts<T>
) {
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

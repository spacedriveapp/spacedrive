import { Store } from 'solid-js/store';

import { useObserver } from './useObserver';

export function useSolidStore<T extends object = {}>(store: Store<T>) {
	const state = useObserver(() => ({ ...store }));
	return new Proxy(state, {
		get: (target, prop) => Reflect.get(target, prop),
		set: (_, prop, value) => Reflect.set(store, prop, value)
	});
}

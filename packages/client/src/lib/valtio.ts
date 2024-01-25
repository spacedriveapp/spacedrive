import { deepEqual } from 'fast-equals';
import { useEffect, useMemo, useState } from 'react';
import { proxy, subscribe } from 'valtio';

export function resetStore<T extends Record<string, any>, E extends Record<string, any>>(
	store: T,
	defaults: E
) {
	for (const key in defaults) {
		// @ts-ignore
		store[key] = defaults[key];
	}
}

// The `valtio-persist` library is not working so this is a small alternative for us to use.
export function valtioPersist<T extends object>(
	localStorageKey: string,
	initialObject?: T,
	opts?: {
		saveFn?: (data: T) => any;
		restoreFn?: (data: any) => T;
	}
): T {
	const d = localStorage.getItem(localStorageKey);
	const p = proxy(
		d !== null
			? opts?.restoreFn
				? opts.restoreFn(JSON.parse(d))
				: JSON.parse(d)
			: initialObject
	);
	subscribe(p, () =>
		localStorage.setItem(localStorageKey, JSON.stringify(opts?.saveFn ? opts.saveFn(p) : p))
	);
	return p;
}

// Subscribe to a Valtio store in React with a selector function.
export function useSelector<T extends object, U>(proxyObject: T, selector: (proxyObject: T) => U) {
	const [slice, setSlice] = useState(() => selector(proxyObject));
	useEffect(
		() =>
			subscribe(proxyObject, () => {
				const newResult = selector(proxyObject);

				setSlice((prev) => {
					// We do this to ensure referential equality.
					if (deepEqual(prev, newResult)) {
						return prev;
					}

					return newResult;
				});
			}),
		[proxyObject, selector]
	);

	return slice;
}

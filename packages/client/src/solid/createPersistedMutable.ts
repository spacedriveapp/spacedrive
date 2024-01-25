import { trackDeep } from '@solid-primitives/deep';
import { createEffect, createRoot } from 'solid-js';
import { type StoreNode } from 'solid-js/store';

type CreatePersistedMutableOpts<T> = {
	onSave?: (value: T) => T;
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

	// I tried using a `Proxy` here but I couldn't get it working with arrays.
	// https://codepen.io/oscartbeaumont/pen/BabzazE
	const dispose = createRoot((dispose) => {
		createEffect(() => {
			// Subscribe to store
			trackDeep(mutable);

			let item: string;
			if (opts?.onSave) {
				item = JSON.stringify(opts.onSave(mutable));
			} else {
				item = JSON.stringify(mutable);
			}
			localStorage.setItem(key, item);
		});
		return dispose;
	});
	if ('onHotReload' in globalThis) globalThis?.onHotReload(dispose);

	return mutable;
}

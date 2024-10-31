import { trackDeep } from '@solid-primitives/deep';
import { createEffect, createRoot } from 'solid-js';
import { type StoreNode } from 'solid-js/store';

type CreatePersistedMutableOpts<T> = {
	onSave?: (value: T) => T;
	/**
	 * This function is always called after the data object's retrieval from localStorage and it getting assigned to the store.
	 *
	 * Originally intended for mutations, but can be used for other things if you have a reason to transform the data.
	 *
	 *
	 * @note This is **not** called on initial load from default values.
	 * @param value The existing data object from localStorage (or null if doesn't exist)
	 * @returns The new data object
	 */
	onLoad?: (value: T | null) => T;
};

// `@solid-primitives/storage`'s `makePersisted` doesn't support `solid-js/store`'s `createMutable` so we roll our own.
export function createPersistedMutable<T extends StoreNode>(
	key: string,
	mutable: T,
	opts?: CreatePersistedMutableOpts<T>
) {
	parsePersistedValue: try {
		const value = localStorage.getItem(key);

		if (value === null) {
			Object.assign(mutable, opts?.onLoad?.(value) ?? {});
			break parsePersistedValue;
		}

		const persisted = JSON.parse(value);
		Object.assign(
			mutable,
			// if we have a function to use to transform data on load, use its return value
			opts?.onLoad?.(persisted) ??
				// otherwise just use the data from localStorage as is
				persisted
		);
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

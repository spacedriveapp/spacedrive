export type NormiCache = Map<string /* $type */, Map<string /* $id */, any>>;

declare global {
	interface Window {
		normiCache?: NormiCache;
	}
}

export interface NormiOptions {
	contextSharing?: boolean;
}

export function getNormiCache(contextSharing: boolean): NormiCache {
	if (contextSharing) {
		if (window.normiCache === undefined) {
			window.normiCache = new Map();
		}

		return window.normiCache;
	} else {
		return new Map();
	}
}

export function getOrCreate<K, A, B>(map: Map<K, Map<A, B>>, key: K): Map<A, B> {
	let m = map.get(key);
	if (m === undefined) {
		m = new Map();
		map.set(key, m);
	}
	return m;
}

export function normaliseValue(value: any, normiCache: NormiCache): any {
	if (value === null || value === undefined) {
		return value;
	} else if (typeof value === 'object') {
		if ('$id' in value && '$type' in value) {
			getOrCreate(normiCache, value.$type).set(value.$id, normaliseValueForStorage(value, true));
			delete value.$id;
			delete value.$type;
		} else if ('$type' in value && 'edges' in value) {
			// TODO: Caching all the edges
			value = (value.edges as any[]).map((v) => normaliseValue(v, normiCache));
		}

		// TODO: Optimise this to only check fields the backend marks as normalisable or on root
		for (const [k, v] of Object.entries(value)) {
			value[k] = normaliseValue(v, normiCache);
		}
	}

	return value;
}

export function normaliseValueForStorage(value: any, rootElem: boolean): any {
	if (value === null || value === undefined) {
		return value;
	} else if (typeof value === 'object') {
		if ('$id' in value && '$type' in value) {
			if (rootElem) {
				let v = Object.assign({}, value);
				delete v.$id;
				delete v.$type;

				// TODO: Optimise this to only check fields the backend marks as normalisable or on root
				for (const [k, vv] of Object.entries(v)) {
					v[k] = normaliseValueForStorage(vv, false);
				}

				return v;
			}

			// TODO: Optimise this to only check fields the backend marks as normalisable or on root
			for (const [k, v] of Object.entries(value)) {
				value[k] = normaliseValueForStorage(v, false);
			}

			return {
				$id: value.$id,
				$type: value.$type
			};
		} else if ('$type' in value && 'edges' in value) {
			return {
				$type: value.$type,
				edges: Object.values(value.edges as any[]).map((v) => v.$id)
			};
		}

		// TODO: Optimise this to only check fields the backend marks as normalisable or on root
		for (const [k, v] of Object.entries(value)) {
			value[k] = normaliseValueForStorage(v, false);
		}
	}

	return value;
}

export function recomputeNormalisedValueFromStorage(value: any, normiCache: NormiCache): any {
	if (value === null || value === undefined) {
		return value;
	} else if (typeof value === 'object') {
		if ('$id' in value && '$type' in value) {
			value = normiCache.get(value.$type)!.get(value.$id); // TODO: Handle `undefined`
		} else if ('$type' in value && 'edges' in value) {
			value = (value.edges as any[]).map(
				(id) => normiCache.get(value.$type)!.get(id) // TODO: Handle `undefined`
			);
		}

		// TODO: Optimise this to only check fields the backend marks as normalisable or on root
		for (const [k, v] of Object.entries(value)) {
			value[k] = recomputeNormalisedValueFromStorage(v, normiCache);
		}
	}

	return value;
}

// export function recomputeRQCache(queryClient: QueryClient, normiCache: NormiCache) {
//   let c = queryClient.getQueryCache();

//   // c.getAll().forEach((query) => {
//   //   const d = query.state.data;
//   //   if (Array.isArray(d)) {
//   //     queryClient.setQueryData(
//   //       query.queryKey,
//   //       d.map((f) => {
//   //         if (typeof f?.$id == "string" && normyCache.has(f?.$id)) {
//   //           return normyCache.get(f.$id);
//   //         }
//   //         return f;
//   //       })
//   //     );
//   //   }
//   // });
// }

export function loadDataFromCache(value: any, normiCache: NormiCache): any {
	// TODO: If can't be pulled out of the cache refetch

	if (value === null || value === undefined) {
		return value;
	} else if (typeof value === 'object') {
		if ('$id' in value && '$type' in value) {
			// if (rootElem) {
			let v = Object.assign({}, value);
			delete v.$id;
			delete v.$type;

			// 	// TODO: Optimise this to only check fields the backend marks as normalisable or on root
			// 	for (const [k, vv] of Object.entries(v)) {
			// 		v[k] = normaliseValueForStorage(vv, false);
			// 	}

			// 	return v;
			// }

			// TODO: Optimise this to only check fields the backend marks as normalisable or on root
			for (const [k, v] of Object.entries(value)) {
				value[k] = normaliseValueForStorage(v, false);
			}

			return v; // normiCache.get(v.$id)!;
		} else if ('$type' in value && 'edges' in value) {
			// TODO: This needs to be replicated in Typescript types
			return [];
			// {
			// 	$type: value.$type,
			// 	edges: Object.values(value.edges as any[]).map((v) => v.$id)
			// };
		}

		// TODO: Optimise this to only check fields the backend marks as normalisable or on root
		for (const [k, v] of Object.entries(value)) {
			value[k] = normaliseValueForStorage(v, false);
		}
	}

	return value;
}

// TODO: Optimistic updates

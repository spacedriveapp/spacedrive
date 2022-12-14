// TODO(@Oscar): I wanna move Normi out of this repo and into rspc because it will make the code way more maintainable but right now I am unsure on the public API to make that possible.
import { CustomHooks } from '@rspc/client';
// @ts-expect-error: // TODO(@Oscar): Fix types
import { __useMutation, __useQuery } from '@rspc/react/internal';
import { useMemo } from 'react';

import { NormiOptions, getNormiCache, loadDataFromCache } from './utils';

export function normiCustomHooks(
	{ contextSharing }: NormiOptions,
	nextHooks?: () => CustomHooks
): () => CustomHooks {
	const normiCache = getNormiCache(contextSharing ?? false);
	const next = nextHooks?.();

	// TODO: Handle manual modifications to the query cache
	//   // queryClient.getQueryCache().subscribe(({ type, query }) => {
	//   //   if (type === "added") {
	//   //     console.log("ADDED", query.queryKey, query.state.data);
	//   //   } else if (type === "updated") {
	//   //     console.log("UPDATE", query.queryKey, query.state.data);

	//   //     const d = query.state.data;
	//   //     if (Array.isArray(d)) {
	//   //       d.forEach((f) => {
	//   //         if (typeof f?.$id == "string") normyCache.set(f.$id, f);
	//   //       });
	//   //     }
	//   //   } else if (type === "removed") {
	//   //     console.log("REMOVED", query.queryKey, query.state.data);
	//   //   }
	//   // });

	// TODO: Subscribe to backend for updates when things change
	// - Subscribe for active queries

	return () => ({
		mapQueryKey: next?.mapQueryKey,
		doQuery: next?.doQuery,
		doMutation: next?.doMutation
		// dangerous: {
		// 	useQuery(keyAndInput, handler, opts) {
		// 		const hook = __useQuery(keyAndInput, handler, opts);
		// 		const data = useMemo(() => {
		// 			return loadDataFromCache(hook.data, normiCache);
		// 		}, [hook.data]);

		// 		return {
		// 			...hook,
		// 			data
		// 		};
		// 	},
		// 	useMutation(handler, opts) {
		// 		const hook = __useMutation(handler, opts);
		// 		// TODO: Normalize data before `onSuccess` or returning from `hook.data`
		// 		return hook;
		// 	}
		// }
	});
}

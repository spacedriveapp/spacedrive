import { useEffect, useState } from 'react';
import type { z } from 'zod';
import { useRouter } from '~/RoutingContext';

// This is hook basically implements a custom version of `useParams`.
// If we use `useParams` directly, every time *any* param changes the current component will rerender so the hook reruns.
//
// With this improved implementation the component will only rerender if the change in parameter causes a change in the output of the Zod schema.
//
// We use this hook to get the library ID high up in the React tree so this reduces unnecessary rerenders of a large portion of the app.
export function useZodRouteParams<Z extends z.AnyZodObject>(schema: Z): z.infer<Z> {
	const router = useRouter();
	const [result, setResult] = useState(() => {
		const params = router.state.matches[router.state.matches.length - 1]?.params || {};
		return schema.parse(params);
	});

	useEffect(
		() =>
			router.subscribe(({ matches }) => {
				const routeMatch = matches[matches.length - 1];
				const params = routeMatch ? (routeMatch.params as any) : {};
				setResult(schema.parse(params));
			}),
		[router, schema, setResult]
	);

	return result;
}

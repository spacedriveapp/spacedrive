import { initRspc, wsBatchLink, type AlphaClient } from '@oscartbeaumont-sd/rspc-client/v2';
import { Suspense, useDeferredValue, useEffect, useState } from 'react';
import { z } from 'zod';
import { context, useRspcContext, type Procedures } from '@sd/client';
import { useRouteTitle, useZodRouteParams, useZodSearchParams } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { PathParamsSchema } from '../route-schemas';
import { EphemeralExplorer, EphemeralNotice } from './ephemeral';

const ParamsSchema = z.object({ node: z.string() });

export const Component = () => {
	const platform = usePlatform();
	const params = useZodRouteParams(ParamsSchema);
	const [pathParams] = useZodSearchParams(PathParamsSchema);

	const path = useDeferredValue(pathParams);

	useRouteTitle(path.path ?? '');

	const [rspcClient, setRspcClient] = useState<AlphaClient<Procedures>>();

	useEffect(() => {
		const endpoint = platform.getRemoteRspcEndpoint(params.node);
		const ws = initRspc<Procedures>({
			links: [
				wsBatchLink({
					url: endpoint.url
				})
			]
		});
		setRspcClient(ws);

		return () => {
			// TODO: We *really* need to cleanup `ws` so we aren't leaking all the resources.
		};
	}, [params.node, platform]);

	const ctx = useRspcContext();

	return (
		<Suspense>
			{rspcClient && (
				<context.Provider
					value={{
						// @ts-expect-error
						client: rspcClient,
						queryClient: ctx.queryClient
					}}
				>
					{/* TODO: Probs also wrap in the library context provider??? */}
					<EphemeralNotice path={path.path ?? ''} />
					<EphemeralExplorer args={path} />
				</context.Provider>
			)}
		</Suspense>
	);
};

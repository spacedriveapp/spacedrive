import { useMemo } from 'react';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import { NodeIdParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useRouteTitle, useZodRouteParams } from '~/hooks';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { createDefaultExplorerSettings } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { TopBarPortal } from '../TopBar/Portal';

export const Component = () => {
	const { id: nodeId } = useZodRouteParams(NodeIdParamsSchema);

	const query = useLibraryQuery(['nodes.listLocations', nodeId]);

	const nodeState = useBridgeQuery(['nodeState']);

	const title = useRouteTitle(nodeState.data?.name || 'Node');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<never>({
					order: null
				}),
			[]
		)
	});

	const explorer = useExplorer({
		items: query.data || null,
		parent: nodeState.data
			? {
					type: 'Node',
					node: nodeState.data
				}
			: undefined,
		settings: explorerSettings,
		showPathBar: false,
		layouts: { media: false }
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<Icon name="Laptop" size={24} className="-mt-px" />
						<span className="truncate text-sm font-medium">{title}</span>
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>

			<Explorer />
		</ExplorerContextProvider>
	);
};

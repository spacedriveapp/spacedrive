import { Laptop } from '@sd/assets/icons';
import { useMemo } from 'react';
import { ExplorerItem, useBridgeQuery, useLibraryQuery } from '@sd/client';
import { NodeIdParamsSchema } from '~/app/route-schemas';
import { useZodRouteParams } from '~/hooks';

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

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<never>({
					order: null
				}),
			[]
		),
		onSettingsChanged: () => {}
	});

	const jeff = query.data
		? [
				...query.data,
				...([
					{
						type: 'Object',
						item: {
							pub_id: [],

							file_paths: [
								{
									name: 'Fake Test',
									kind: 5,
									is_dir: false,
									size_in_bytes_bytes: [0, 0, 0, 0, 0, 3, 244, 30]
								}
							]
						}
					}
				] as unknown as ExplorerItem[])
		  ]
		: [];

	const explorer = useExplorer({
		items: jeff || null,
		parent: nodeState.data
			? {
					type: 'Node',
					node: nodeState.data
			  }
			: undefined,
		settings: explorerSettings,
		showPathBar: false
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<img src={Laptop} className="mt-[-1px] h-6 w-6" />
						<span className="truncate text-sm font-medium">
							{nodeState.data?.name || 'Node'}
						</span>
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>

			<Explorer />
		</ExplorerContextProvider>
	);
};

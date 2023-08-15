import { Laptop } from '@sd/assets/icons';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import { NodeIdParamsSchema } from '~/app/route-schemas';
import { useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';
import { ExplorerContext } from '../Explorer/Context';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer } from '../Explorer/useExplorer';
import { TopBarPortal } from '../TopBar/Portal';

export const Component = () => {
	const { id: nodeId } = useZodRouteParams(NodeIdParamsSchema);

	const query = useLibraryQuery(['nodes.listLocations', nodeId]);

	const nodeState = useBridgeQuery(['nodeState']);

	const explorer = useExplorer({
		items: query.data || null,
		parent: nodeState.data
			? {
					type: 'Node',
					node: nodeState.data
			  }
			: undefined
	});

	return (
		<ExplorerContext.Provider value={explorer}>
			<TopBarPortal
				left={
					<div className="group flex flex-row items-center space-x-2">
						<span className="flex flex-row items-center">
							<img
								src={Laptop}
								className="ml-3 mr-2 mt-[-1px] inline-block h-6 w-6"
							/>
							<span className="overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium">
								{nodeState.data?.name || 'Node'}
							</span>
						</span>
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>

			<Explorer />
		</ExplorerContext.Provider>
	);
};

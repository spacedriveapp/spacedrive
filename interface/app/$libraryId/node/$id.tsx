import { Laptop, Node } from '@sd/assets/icons';
import { useLoaderData } from 'react-router';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import type { NodeIdParams } from '~/app/$libraryId';
import Explorer from '~/app/$libraryId/Explorer';
import { TopBarPortal } from '~/app/$libraryId/TopBar/Portal';
import TopBarOptions from '~/app/$libraryId/TopBar/TopBarOptions';
import { useExplorerTopBarOptions } from '~/hooks';

export const Component = () => {
	const { id: nodeId } = useLoaderData() as NodeIdParams;

	const locations = useLibraryQuery(['nodes.listLocations', nodeId]);

	const nodeState = useBridgeQuery(['nodeState']);

	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	return (
		<>
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
				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>

			{locations.data && <Explorer items={locations.data} />}
		</>
	);
};

import { Laptop, Node } from '@sd/assets/icons';
import { z } from 'zod';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import { useExplorerTopBarOptions, useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';

const PARAMS = z.object({
	id: z.string()
});

export const Component = () => {
	// const [{ path }] = useExplorerSearchParams();
	const { id: node_id } = useZodRouteParams(PARAMS);

	const locations = useLibraryQuery(['nodes.listLocations', node_id]);

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

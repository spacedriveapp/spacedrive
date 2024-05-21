import { HardwareModel, usePeers } from '@sd/client';
import { NodeIdParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useRouteTitle, useZodRouteParams } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import { TopBarPortal } from '../TopBar/Portal';

export const Component = () => {
	const { id: _nodeId } = useZodRouteParams(NodeIdParamsSchema);
	// we encode/decode because nodeId has special characters and I'm not willing to change that rn
	const nodeId = decodeURIComponent(_nodeId);

	const peers = usePeers();

	const peer = peers.get(nodeId);

	const title = useRouteTitle(peer?.metadata?.name || 'Peer');

	return (
		<div className="flex w-full items-center justify-center">
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<span className="truncate text-sm font-medium">{title}</span>
					</div>
				}
			/>

			{peer?.metadata.device_model && (
				<div className="flex flex-col items-center justify-center gap-3">
					<Icon
						name={hardwareModelToIcon(peer?.metadata.device_model as HardwareModel)}
						size={150}
						className=""
					/>
					<h3 className="text-lg font-bold">{peer?.metadata.name}</h3>
					<h3 className="text-sm text-ink-dull">
						{peer?.metadata.operating_system?.toString()}
					</h3>
					<h3 className="text-sm text-ink-dull">{nodeId}</h3>
					<div className="mt-8 flex h-28 w-96 items-center justify-center rounded-lg border border-dashed border-app-line p-4 text-sm font-medium text-ink-dull">
						Drop files here to send with Spacedrop
					</div>
				</div>
			)}
		</div>
	);
};

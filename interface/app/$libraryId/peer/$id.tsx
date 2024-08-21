import { t } from 'i18next';
import { HardwareModel, usePeers } from '@sd/client';
import { NodeIdParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useOperatingSystem, useRouteTitle, useZodRouteParams } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import StarfieldEffect from './StarfieldEffect'; // Import the StarfieldEffect component

export const Component = () => {
	const { id: _nodeId } = useZodRouteParams(NodeIdParamsSchema);
	const nodeId = decodeURIComponent(_nodeId);
	const peers = usePeers();
	const peer = peers.get(nodeId);
	const title = useRouteTitle(peer?.metadata?.name || 'Peer');
	const os = useOperatingSystem();

	return (
		<div className="flex w-full items-center justify-center">
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<span className="truncate text-sm font-medium">{title}</span>
					</div>
				}
				right={os === 'windows' && <TopBarOptions />}
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

					<div className="relative mt-8 flex h-28 w-96 items-center justify-center rounded-lg border border-solid border-app-line p-4 text-sm font-medium text-ink-dull">
						<StarfieldEffect />
						<div className="pointer-events-none absolute inset-0 flex items-center justify-center">
							{t('drop_files_here_to_send_with')}
						</div>
					</div>
				</div>
			)}
		</div>
	);
};

import { HardwareModel, useBridgeQuery, usePeers } from '@sd/client';
import { Button, toast, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';

export default function PeersSection() {
	const { t } = useLocale();
	const peers = usePeers();

	return (
		<Section name={t('peers')}>
			{Array.from(peers).map(([id, node]) => (
				<SidebarLink className="group relative w-full" to={`todo`} key={id}>
					{node.metadata.device_model ? (
						<Icon
							name={hardwareModelToIcon(node.metadata.device_model as HardwareModel)}
							size={20}
							className="mr-1"
						/>
					) : (
						<Icon name="Laptop" className="mr-1" />
					)}

					<span className="truncate">{node.metadata.name}</span>
				</SidebarLink>
			))}
		</Section>
	);
}

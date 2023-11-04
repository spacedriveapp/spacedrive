import { Laptop } from '@sd/assets/icons';
import { Link } from 'react-router-dom';
import { useBridgeQuery, useFeatureFlag } from '@sd/client';
import { Button, Tooltip } from '@sd/ui';
import { SubtleButton } from '~/components';

import SidebarLink from '../Link';
import Section from '../Section';

export const Devices = () => {
	const { data: node } = useBridgeQuery(['nodeState']);
	const isPairingEnabled = useFeatureFlag('p2pPairing');

	return (
		<Section
			name="Devices"
			actionArea={
				isPairingEnabled && (
					<Link to="settings/library/nodes">
						<SubtleButton />
					</Link>
				)
			}
		>
			{node && (
				<SidebarLink className="group relative w-full" to={`node/${node.id}`} key={node.id}>
					<img src={Laptop} className="mr-1 h-5 w-5" />
					<span className="truncate">{node.name}</span>
				</SidebarLink>
			)}

			<Tooltip
				label="Coming soon! This alpha release doesn't include library sync, it will be ready very soon."
				position="right"
			>
				<Button disabled variant="dotted" className="mt-1 w-full">
					Add Device
				</Button>
			</Tooltip>
		</Section>
	);
};

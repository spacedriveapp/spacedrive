import { Link } from 'react-router-dom';
import { useBridgeQuery, useFeatureFlag } from '@sd/client';
import { Button, Tooltip } from '@sd/ui';
import { Icon, SubtleButton } from '~/components';
import { useLocale } from '~/hooks';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';

export default function DevicesSection() {
	const { data: node } = useBridgeQuery(['nodeState']);
	const isPairingEnabled = useFeatureFlag('p2pPairing');

	const { t } = useLocale();

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
					<Icon name="Laptop" className="mr-1 size-5" />
					<span className="truncate">{node.name}</span>
				</SidebarLink>
			)}

			<Tooltip label={t('devices_coming_soon_tooltip')} position="right">
				<Button disabled variant="dotted" className="mt-1 w-full">
					{t('add_device')}
				</Button>
			</Tooltip>
		</Section>
	);
}

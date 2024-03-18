import { ArrowsClockwise, Cloud, Database, Factory, ShareNetwork } from '@phosphor-icons/react';
import { useFeatureFlag } from '@sd/client';

import Icon from '../../SidebarLayout/Icon';
import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';

export default function DebugSection() {
	const debugRoutes = useFeatureFlag('debugRoutes');

	if (!debugRoutes) return <></>;

	return (
		<Section name="Debug">
			<div className="space-y-0.5">
				<SidebarLink to="debug/sync">
					<Icon component={ArrowsClockwise} />
					Sync
				</SidebarLink>
				<SidebarLink to="debug/cloud">
					<Icon component={Cloud} />
					Cloud
				</SidebarLink>
				<SidebarLink to="debug/cache">
					<Icon component={Database} />
					Cache
				</SidebarLink>
				<SidebarLink to="debug/actors">
					<Icon component={Factory} />
					Actors
				</SidebarLink>
				<SidebarLink to="debug/p2p/overview">
					<Icon component={ShareNetwork} />
					P2P
				</SidebarLink>
			</div>
		</Section>
	);
}

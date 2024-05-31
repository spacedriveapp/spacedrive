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
				<SidebarLink to="debug/cloud">
					<Icon component={Cloud} />
					Cloud
				</SidebarLink>
			</div>
		</Section>
	);
}

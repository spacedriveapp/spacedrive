import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function NodesSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Nodes" description="Manage the nodes in your Spacedrive network." />
		</SettingsContainer>
	);
}

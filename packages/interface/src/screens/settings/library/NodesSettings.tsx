import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function NodesSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader
				title="Nodes"
				description="Manage the nodes connected to this library. A node is an instance of Spacedrive's backend, running on a device or server. Each node carries a copy of the database and synchronizes via peer-to-peer connections in realtime."
			/>
		</SettingsContainer>
	);
}

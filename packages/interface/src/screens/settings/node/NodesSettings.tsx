import { useBridgeQuery } from '@sd/client';
import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function NodesSettings() {
	const { data: connectedPeers } = useBridgeQuery('ConnectedPeers'); // TODO: Show offline peers. This should be a library scoped query!

	return (
		<SettingsContainer>
			<>
				<SettingsHeader title="Nodes" description="Manage the nodes in your Spacedrive network." />
				<div>
					{/* TODO: Style this list */}
					{Object.keys(connectedPeers || []).map((peer_id) => (
						<>
							<h1 className="text-xl">{connectedPeers![peer_id].name}</h1>
							<p className="text-xs">{peer_id}</p>
							{/* TODO: Unpair button */}
						</>
					))}
				</div>
			</>
		</SettingsContainer>
	);
}

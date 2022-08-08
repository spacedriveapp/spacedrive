import { useBridgeQuery } from '@sd/client';
import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function NodesSettings() {
	const { data: connectedPeers } = useBridgeQuery(['p2p.connectedPeers']); // TODO: Show offline peers. This should be a library scoped query!

	return (
		<SettingsContainer>
			<>
				<SettingsHeader title="Nodes" description="Manage the nodes in your Spacedrive network." />
				<div>
					{/* TODO: Style this list */}
					{Object.entries(connectedPeers ?? {}).map(([peerId, metadata]) => (
						<>
							<h1 className="text-xl">{metadata.name}</h1>
							<p className="text-xs">{peerId}</p>
							{/* TODO: Unpair button */}
						</>
					))}
				</div>
			</>
		</SettingsContainer>
	);
}

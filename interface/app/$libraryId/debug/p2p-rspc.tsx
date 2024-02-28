import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useDiscoveredPeers, useLibraryContext } from '@sd/client';
import { Button } from '@sd/ui';

export const Component = () => {
	// TODO: Handle if P2P is disabled
	return (
		<div className="p-4">
			<PeerSelector />
		</div>
	);
};

function PeerSelector() {
	const peers = useDiscoveredPeers();
	const navigate = useNavigate();
	const library = useLibraryContext();

	return (
		<>
			<h1>Nodes:</h1>
			{peers.size === 0 ? (
				<p>No peers found...</p>
			) : (
				<ul>
					{[...peers.entries()].map(([id, _node]) => (
						<li key={id}>
							{id}
							<Button
								onClick={() =>
									navigate(
										`/${library.library.uuid}/ephemeral/remote/${id}/0-0?path=/System/Volumes/Data`
									)
								}
							>
								Open Explorer
							</Button>
						</li>
					))}
				</ul>
			)}
		</>
	);
}

import { useNavigate } from 'react-router';
import { useCache, useDiscoveredPeers, useLibraryQuery, useNodes } from '@sd/client';
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
							<Button onClick={() => navigate(`/remote/${id}/browse`)}>
								Open Library Browser
							</Button>
						</li>
					))}
				</ul>
			)}
		</>
	);
}

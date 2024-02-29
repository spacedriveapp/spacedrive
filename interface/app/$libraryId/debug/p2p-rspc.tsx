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
	const result = useLibraryQuery(['locations.list']);
	useNodes(result.data?.nodes);
	const locations = useCache(result.data?.items);

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

			<h1>Local:</h1>
			{locations?.map((l) => (
				<Button
					key={l.id}
					variant="accent"
					onClick={() => navigate(`/local/${l.id}/browse`)}
				>
					{l.name}
				</Button>
			))}
		</>
	);
}

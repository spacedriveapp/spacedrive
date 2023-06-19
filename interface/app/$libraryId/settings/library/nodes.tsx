import { useDiscoveredPeers, useFeatureFlag, useLibraryMutation } from '@sd/client';
import { Button } from '@sd/ui';
import { Heading } from '../Layout';

export const Component = () => {
	const isPairingEnabled = useFeatureFlag('p2pPairing');

	return (
		<>
			<Heading
				title="Nodes"
				description="Manage the nodes connected to this library. A node is an instance of Spacedrive's backend, running on a device or server. Each node carries a copy of the database and synchronizes via peer-to-peer connections in realtime."
			/>

			{/* TODO: Show paired nodes + unpair button */}

			{isPairingEnabled && <IncorrectP2PPairingPane />}
		</>
	);
};

// TODO: This entire component shows a UI which is pairing by node but that is just not how it works.
function IncorrectP2PPairingPane() {
	const onlineNodes = useDiscoveredPeers();
	const p2pPair = useLibraryMutation('p2p.pair', {
		onSuccess(data) {
			console.log(data);
		}
	});

	console.log(onlineNodes);

	return (
		<>
			<h1>Pairing</h1>
			{[...onlineNodes.entries()].map(([id, node]) => (
				<div key={id} className="flex space-x-2">
					<p>{node.name}</p>

					<Button onClick={() => p2pPair.mutate(id)}>Pair</Button>
				</div>
			))}
		</>
	);
}

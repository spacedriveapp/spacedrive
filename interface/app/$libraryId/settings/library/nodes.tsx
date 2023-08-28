import {
	isEnabled,
	useBridgeMutation,
	useBridgeQuery,
	useConnectedPeers,
	useDiscoveredPeers,
	useFeatureFlag
} from '@sd/client';
import { Button } from '@sd/ui';
import { startPairing } from '~/app/p2p/pairing';
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

			{/* TODO: Replace with modal */}
			{isPairingEnabled && <IncorrectP2PPairingPane />}
		</>
	);
};

// TODO: This entire component shows a UI which is pairing by node but that is just not how it works.
function IncorrectP2PPairingPane() {
	const onlineNodes = useDiscoveredPeers();
	const connectedNodes = useConnectedPeers();
	const p2pPair = useBridgeMutation('p2p.pair', {
		onSuccess(data) {
			console.log(data);
		}
	});
	const nlmState = useBridgeQuery(['p2p.nlmState'], {
		refetchInterval: 1000
	});
	const libraries = useBridgeQuery(['library.list']);

	return (
		<>
			<div className="flex-space-4 flex w-full">
				<div className="flex-[50%]">
					<h1>Pairing</h1>
					{[...onlineNodes.entries()].map(([id, node]) => (
						<div key={id} className="flex space-x-2">
							<p>{node.name}</p>

							<Button
								onClick={() => {
									// TODO: This is not great
									p2pPair.mutateAsync(id).then((id) =>
										startPairing(id, {
											name: node.name,
											os: node.operating_system
										})
									);
								}}
							>
								Pair
							</Button>
						</div>
					))}
				</div>
				<div className="flex-[50%]">
					<h1 className="mt-4">Connected</h1>
					{[...connectedNodes.entries()].map(([id, node]) => (
						<div key={id} className="flex space-x-2">
							<p>{id}</p>
						</div>
					))}
				</div>
			</div>
			<div>
				<p>NLM State:</p>
				<pre className="pl-5">{JSON.stringify(nlmState.data || {}, undefined, 2)}</pre>
			</div>
			<div>
				<p>Libraries:</p>
				{libraries.data?.map((v) => (
					<div key={v.uuid} className="pb-2">
						<p>
							{v.config.name} - {v.uuid}
						</p>
						<div className="pl-5">
							<p>Instance: {`${v.config.instance_id}/${v.instance_id}`}</p>
							<p>Instance PK: {`${v.instance_public_key}`}</p>
						</div>
					</div>
				))}
			</div>
		</>
	);
}

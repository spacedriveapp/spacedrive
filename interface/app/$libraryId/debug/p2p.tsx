import {
	useBridgeMutation,
	useBridgeQuery,
	useCache,
	useConnectedPeers,
	useDiscoveredPeers,
	useNodes,
	useP2PContextRaw
} from '@sd/client';

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);

	return (
		<div className="p-4">
			{/* // TODO: Fix this */}
			{/* {node.data?.p2p_enabled === false ? (
				<h1 className="text-red-500">P2P is disabled. Please enable it in settings!</h1>
			) : (
				<Page />
			)} */}
			<Page />
		</div>
	);
};

function Page() {
	const p2pState = useBridgeQuery(['p2p.state'], {
		refetchInterval: 1000
	});
	const result = useBridgeQuery(['library.list']);
	const connectedPeers = useConnectedPeers();
	const discoveredPeers = useDiscoveredPeers();
	useNodes(result.data?.nodes);
	const libraries = useCache(result.data?.items);
	const debugConnect = useBridgeMutation(['p2p.debugConnect']);

	return (
		<div className="flex flex-col space-y-8">
			<div className="flex justify-around">
				<div>
					<h1 className="mt-4">Discovered:</h1>
					{discoveredPeers.size === 0 && <p className="pl-2">None</p>}
					{[...discoveredPeers.entries()].map(([id, node]) => (
						<div key={id} className="flex space-x-2">
							<p>{id}</p>
							<button onClick={() => debugConnect.mutate(id)}>Connect</button>
						</div>
					))}
				</div>
				<div>
					<h1 className="mt-4">Connected to:</h1>
					{connectedPeers.size === 0 && <p className="pl-2">None</p>}
					{[...connectedPeers.entries()].map(([id, node]) => (
						<div key={id} className="flex space-x-2">
							<p>{id}</p>
						</div>
					))}
				</div>
			</div>

			<div>
				<p>Current nodes libraries:</p>
				{libraries.map((v) => (
					<div key={v.uuid} className="pb-2 pl-3">
						<p>
							{v.config.name} - {v.uuid}
						</p>
						<div className="pl-8">
							<p>Instance: {`${v.config.instance_id}/${v.instance_id}`}</p>
							<p>Instance PK: {`${v.instance_public_key}`}</p>
						</div>
					</div>
				))}
			</div>

			<div>
				<p>NLM State:</p>
				<pre>{JSON.stringify(p2pState.data || {}, undefined, 2)}</pre>
			</div>
		</div>
	);
}

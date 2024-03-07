import { useState } from 'react';
import { Outlet, useNavigate } from 'react-router';
import {
	useBridgeMutation,
	useBridgeQuery,
	useCache,
	useConnectedPeers,
	useDiscoveredPeers,
	useNodes
} from '@sd/client';
import { Button, toast } from '@sd/ui';
import { useZodRouteParams, useZodSearchParams } from '~/hooks';

export const Component = () => {
	const navigate = useNavigate();
	// TODO: Handle if P2P is disabled
	// const node = useBridgeQuery(['nodeState']);
	// {node.data?.p2p_enabled === false ? (
	// 	<h1 className="text-red-500">P2P is disabled. Please enable it in settings!</h1>
	// ) : (
	// 	<Page />
	// )}

	return (
		<div>
			<div className="flex space-x-4">
				<Button variant="accent" onClick={() => navigate('overview')}>
					Overview
				</Button>
				<Button variant="accent" onClick={() => navigate('remote')}>
					Remote Peers
				</Button>
				<Button variant="accent" onClick={() => navigate('instances')}>
					Instances
				</Button>
			</div>
			<div className="p-4">
				<Outlet />
			</div>
		</div>
	);
};

export function Overview() {
	const p2pState = useBridgeQuery(['p2p.state'], {
		refetchInterval: 1000
	});
	const result = useBridgeQuery(['library.list']);
	const connectedPeers = useConnectedPeers();
	const discoveredPeers = useDiscoveredPeers();
	useNodes(result.data?.nodes);
	const libraries = useCache(result.data?.items);
	const debugConnect = useBridgeMutation(['p2p.debugConnect'], {
		onSuccess: () => {
			toast.success('Connected!');
		},
		onError: (e) => {
			toast.error(`Error connecting '${e.message}'`);
		}
	});

	return (
		<div className="flex flex-col space-y-8">
			<div className="flex justify-around">
				<div>
					<h1 className="mt-4">Discovered:</h1>
					{discoveredPeers.size === 0 && <p className="pl-2">None</p>}
					{[...discoveredPeers.entries()].map(([id, _node]) => (
						<div key={id} className="flex space-x-2">
							<p>{id}</p>
							<Button
								variant="accent"
								onClick={() => debugConnect.mutate(id)}
								disabled={debugConnect.isLoading}
							>
								Connect
							</Button>
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

export function RemotePeers() {
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

export const Instances = () => {
	const debugGetLibraryPeers = useBridgeQuery(['p2p.debugGetLibraryPeers']);
	const debugConnect = useBridgeMutation(['p2p.debugConnect'], {
		onSuccess: () => {
			toast.success('Connected!');
		},
		onError: (e) => {
			toast.error(`Error connecting '${e.message}'`);
		}
	});

	return (
		<>
			<h1>TODO</h1>
			<div>
				{!debugGetLibraryPeers.data ? (
					<p>Loading...</p>
				) : (
					<>
						{debugGetLibraryPeers.data.map(([key, instances]) => (
							<div key={key}>
								<p>{key}</p>
								<div className="pl-2">
									{instances.map((instanceId) => (
										<div key={instanceId} className="flex space-x-2 pb-2 pl-3">
											<p>{instanceId}</p>

											<Button
												variant="accent"
												onClick={() => debugConnect.mutate(instanceId)}
												disabled={debugConnect.isLoading}
											>
												Connect
											</Button>
										</div>
									))}
								</div>
							</div>
						))}
					</>
				)}
			</div>
		</>
	);
};

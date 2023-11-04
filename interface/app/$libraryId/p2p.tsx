import { useBridgeQuery } from '@sd/client';

export const Component = () => {
	const p2pState = useBridgeQuery(['p2p.state'], {
		refetchInterval: 1000
	});

	return (
		<div className="p-4">
			<p>NLM State:</p>
			<pre>{JSON.stringify(p2pState.data || {}, undefined, 2)}</pre>
		</div>
	);
};

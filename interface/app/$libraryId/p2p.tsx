import { useBridgeQuery } from '@sd/client';

export const Component = () => {
	const p2pState = useBridgeQuery(['p2p.state'], {
		refetchInterval: 1000
	});
	const libraries = useBridgeQuery(['library.list']);

	return (
		<div className="p-4">
			<p>NLM State:</p>
			<pre>{JSON.stringify(p2pState.data || {}, undefined, 2)}</pre>

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
		</div>
	);
};

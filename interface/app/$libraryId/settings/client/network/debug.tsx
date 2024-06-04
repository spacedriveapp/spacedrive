import { useBridgeQuery } from '@sd/client';
import { useLocale } from '~/hooks';

import { Heading } from '../../Layout';

export const Component = () => {
	const { t } = useLocale();
	const p2pState = useBridgeQuery(['p2p.state'], {
		refetchInterval: 1000
	});
	const result = useBridgeQuery(['library.list']);

	return (
		<div>
			<Heading
				title={t('network_settings_advanced')}
				description={t('network_settings_advanced_description')}
			/>

			<pre>{JSON.stringify(p2pState.data || {}, undefined, 2)}</pre>
			<div className="h-8" />
			<pre>
				{JSON.stringify(
					result.data?.map((lib) => ({
						id: lib.uuid,
						name: lib.config.name,
						instance: `${lib.config.instance_id}/${lib.instance_id}`,
						instanceRemotePk: `${lib.instance_public_key}`
					})) || {},
					undefined,
					2
				)}
			</pre>
		</div>
	);
};

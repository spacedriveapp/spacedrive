import { useEffect, useState } from 'react';
import { useBridgeQuery, useFeatureFlag, useP2PEvents, withFeatureFlag } from '@sd/client';
import { toast } from '@sd/ui';

import { startPairing } from './pairing';
import { SpacedropUI } from './Spacedrop';

export const SpacedropUI2 = withFeatureFlag('spacedrop', SpacedropUI);

// Entrypoint of P2P UI
export function P2P() {
	const pairingEnabled = useFeatureFlag('p2pPairing');
	useP2PEvents((data) => {
		if (data.type === 'PairingRequest' && pairingEnabled) {
			startPairing(data.id, {
				name: data.name,
				os: data.os
			});
		}
	});

	return (
		<>
			<SpacedropUI2 />
		</>
	);
}

export function useP2PErrorToast() {
	const nodeState = useBridgeQuery(['nodeState']);
	const [didShowError, setDidShowError] = useState({
		ipv4: false,
		ipv6: false
	});

	useEffect(() => {
		const ipv4Error =
			(nodeState.data?.p2p_enabled && nodeState.data?.p2p.ipv4.status === 'Error') || false;
		const ipv6Error =
			(nodeState.data?.p2p_enabled && nodeState.data?.p2p.ipv6.status === 'Error') || false;

		if (didShowError.ipv4 && ipv4Error)
			toast.error({
				title: 'Error starting up P2P!',
				body: 'Error creating the IPv4 listener. Please check your firewall settings!'
			});

		if (didShowError.ipv6 && ipv6Error)
			toast.error({
				title: 'Error starting up P2P!',
				body: 'Error creating the IPv6 listener. Please check your firewall settings!'
			});

		setDidShowError({
			ipv4: ipv4Error,
			ipv6: ipv6Error
		});
	}, [nodeState.data, didShowError]);

	return null;
}

import { useEffect, useRef, useState } from 'react';
import { useBridgeQuery } from '@sd/client';
import { toast } from '@sd/ui';

export function useP2PErrorToast() {
	const listeners = useBridgeQuery(['p2p.listeners']);
	const didShowError = useRef(false);

	useEffect(() => {
		if (!listeners.data) return;
		if (didShowError.current) return;

		let body: JSX.Element | undefined;
		if (listeners.data.ipv4.type === 'Error' && listeners.data.ipv6.type === 'Error') {
			body = (
				<div>
					<p>
						Error creating the IPv4 and IPv6 listeners. Please check your firewall
						settings!
					</p>
					<p>{listeners.data.ipv4.error}</p>
				</div>
			);
		} else if (listeners.data.ipv4.type === 'Error') {
			body = (
				<div>
					<p>Error creating the IPv4 listeners. Please check your firewall settings!</p>
					<p>{listeners.data.ipv4.error}</p>
				</div>
			);
		} else if (listeners.data.ipv6.type === 'Error') {
			body = (
				<div>
					<p>Error creating the IPv6 listeners. Please check your firewall settings!</p>
					<p>{listeners.data.ipv6.error}</p>
				</div>
			);
		}

		if (body) {
			toast.error(
				{
					title: 'Error starting up networking!',
					body
				},
				{
					id: 'p2p-listener-error'
				}
			);
			didShowError.current = true;
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [listeners.data]);

	return null;
}

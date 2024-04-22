import { useEffect, useState } from 'react';
import { useBridgeQuery } from '@sd/client';
import { toast } from '@sd/ui';

export function useP2PErrorToast() {
	const listeners = useBridgeQuery(['p2p.listeners']);
	const [didShowError, setDidShowError] = useState(false);

	useEffect(() => {
		if (!listeners.data) return;
		if (didShowError) return;

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
			// This timeout is required so the toast is triggered after the app renders or else it might not show up.
			setTimeout(() => {
				toast.error(
					{
						title: 'Error starting up networking!',
						body
					},
					{
						id: 'p2p-listener-error'
					}
				);
			}, 500);
			setDidShowError(true);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [listeners.data]);

	return null;
}

import { useEffect, useRef } from 'react';
import { useBridgeQuery } from '@sd/client';
import { toast } from '@sd/ui';
import { useLocale } from '~/hooks';

const errorMessages = {
	ipv4_ipv6: 'ipv4_ipv6_listeners_error',
	ipv4: 'ipv4_listeners_error',
	ipv6: 'ipv6_listeners_error',
	relay: 'relay_listeners_error'
};

export function useP2PErrorToast() {
	const listeners = useBridgeQuery(['p2p.listeners']);
	const didShowError = useRef(false);
	const { t } = useLocale();

	useEffect(() => {
		if (!listeners.data || didShowError.current) return;

		const getErrorBody = (type: keyof typeof errorMessages, error: string) => (
			<div>
				<p>{t(errorMessages[type])}</p>
				<p>{error}</p>
			</div>
		);

		let body: JSX.Element | undefined;

		switch (true) {
			case listeners.data.ipv4.type === 'Error' && listeners.data.ipv6.type === 'Error':
				body = getErrorBody('ipv4_ipv6', listeners.data.ipv4.error);
				break;
			case listeners.data.ipv4.type === 'Error':
				body = getErrorBody('ipv4', listeners.data.ipv4.error);
				break;
			case listeners.data.ipv6.type === 'Error':
				body = getErrorBody('ipv6', listeners.data.ipv6.error);
				break;
			case listeners.data.relay.type === 'Error':
				body = getErrorBody('relay', listeners.data.relay.error);
				break;
			default:
				break;
		}

		if (body) {
			toast.error(
				{
					title: t('networking_error'),
					body
				},
				{
					id: 'p2p-listener-error'
				}
			);
			didShowError.current = true;
		}

		if (body) {
			toast.error(
				{
					title: t('networking_error'),
					body
				},
				{
					id: 'p2p-listener-error'
				}
			);
			didShowError.current = true;
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [listeners.data, t]);

	return null;
}

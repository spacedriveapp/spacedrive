import { useEffect, useRef, useState } from 'react';
import { useBridgeQuery } from '@sd/client';
import { toast } from '@sd/ui';
import { useLocale } from '~/hooks';

export function useP2PErrorToast() {
	const listeners = useBridgeQuery(['p2p.listeners']);
	const didShowError = useRef(false);
	const { t } = useLocale();

	useEffect(() => {
		if (!listeners.data) return;
		if (didShowError.current) return;

		let body: JSX.Element | undefined;
		if (listeners.data.ipv4.type === 'Error' && listeners.data.ipv6.type === 'Error') {
			body = (
				<div>
					<p>{t('ipv4_ipv6_listeners_error')}</p>
					<p>{listeners.data.ipv4.error}</p>
				</div>
			);
		} else if (listeners.data.ipv4.type === 'Error') {
			body = (
				<div>
					<p>{t('ipv4_listeners_error')}</p>
					<p>{listeners.data.ipv4.error}</p>
				</div>
			);
		} else if (listeners.data.ipv6.type === 'Error') {
			body = (
				<div>
					<p>{t('ipv6_listeners_error')}</p>
					<p>{listeners.data.ipv6.error}</p>
				</div>
			);
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
	}, [listeners.data]);

	return null;
}

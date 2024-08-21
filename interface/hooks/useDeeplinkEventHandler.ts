import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { DeeplinkEvent } from '~/util/events';

export const useDeeplinkEventHandler = () => {
	const navigate = useNavigate();
	useEffect(() => {
		const handler = (e: DeeplinkEvent) => {
			e.preventDefault();

			const url = e.detail.url;
			if (!url) return;
			// If the URL has search params, we need to navigate to the URL with the search params
			const [path, search] = url.split('?');
			if (search) {
				navigate({ pathname: path, search });
			} else {
				navigate(url);
			}
		};

		document.addEventListener('deeplink', handler);
		return () => document.removeEventListener('deeplink', handler);
	}, [navigate]);
};

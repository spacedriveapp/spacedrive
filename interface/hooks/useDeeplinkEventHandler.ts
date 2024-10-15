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
			// If hash is present, we need to split it from the search params, and remove it from the search value
			const [searchParams, hash] = search ? search.split('#') : ['', ''];
			const searchParamsObj = new URLSearchParams(searchParams);
			const searchParamsString = searchParamsObj.toString();
			console.log('Navigating to', {
				path,
				searchParamsString,
				hash
			});

			navigate({ pathname: path, search: searchParamsString, hash });

			// if (search) {
			// 	navigate({ pathname: path, search, hash });
			// } else {
			// 	navigate(url);
			// }
		};

		document.addEventListener('deeplink', handler);
		return () => document.removeEventListener('deeplink', handler);
	}, [navigate]);
};

import { PageContextBuiltIn } from 'vite-plugin-ssr';

import { getDoc } from './api';

export async function onBeforeRender(pageContext: PageContextBuiltIn) {
	const slug = pageContext.routeParams['*'];
	const data = getDoc(slug);

	return {
		pageContext: {
			pageProps: {
				data
			}
		}
	};
}

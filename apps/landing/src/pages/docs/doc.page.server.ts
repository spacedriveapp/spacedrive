import { PageContextBuiltIn } from 'vite-plugin-ssr';

import { getDoc, getSidebar } from './api';

export async function onBeforeRender(pageContext: PageContextBuiltIn) {
	const doc = getDoc(pageContext.routeParams['*']);
	const sidebar = getSidebar();

	return {
		pageContext: {
			pageProps: {
				doc,
				sidebar
			}
		}
	};
}

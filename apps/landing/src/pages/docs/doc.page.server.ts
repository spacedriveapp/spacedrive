import { PageContextBuiltIn } from 'vite-plugin-ssr';

import { getDoc } from './api';
import config from './docs';

export const passToClient = ['pageProps'];

export async function onBeforeRender(pageContext: PageContextBuiltIn) {
	return {
		pageContext: {
			pageProps: getDoc(pageContext.routeParams['*'], config)
		}
	};
}

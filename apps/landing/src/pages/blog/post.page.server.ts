import { PageContextBuiltIn } from 'vite-plugin-ssr/types';
import { getBlogPost } from './api';

export const passToClient = ['pageProps'];

export async function onBeforeRender(pageContext: PageContextBuiltIn) {
	const post = getBlogPost(pageContext.routeParams['*']!);

	return {
		pageContext: {
			pageProps: { post }
		}
	};
}

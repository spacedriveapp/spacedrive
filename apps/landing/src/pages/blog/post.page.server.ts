import { PageContextBuiltIn } from 'vite-plugin-ssr';
import { getBlogPost, getBlogPosts } from './api';

export const passToClient = ['pageProps'];

export async function onBeforeRender(pageContext: PageContextBuiltIn) {
	const post = getBlogPost(pageContext.routeParams['*']!);

	return {
		pageContext: {
			pageProps: { post }
		}
	};
}

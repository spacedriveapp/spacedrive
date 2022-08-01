import { getPosts } from './api';

export async function onBeforeRender() {
	const posts = await getPosts();

	return {
		pageContext: {
			pageProps: {
				posts
			}
		}
	};
}

export async function prerender() {
	const posts = await getPosts();

	return {
		url: '/blog',
		pageContext: { pageProps: { posts } }
	};
}

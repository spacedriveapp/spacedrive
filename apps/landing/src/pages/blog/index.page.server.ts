import { getBlogPosts } from './api';

export async function onBeforeRender() {
	const posts = getBlogPosts();

	return {
		pageContext: {
			pageProps: {
				posts
			}
		}
	};
}

// pre-render all doc pages at the same time as index
export async function prerender() {
	const posts = getBlogPosts();

	const docsArray = Object.values(posts).map((post) => ({
		url: `/blog/${post.slug}/`,
		pageContext: { pageProps: { post } }
	}));

	return [
		...docsArray,
		{
			url: '/blog',
			pageContext: { pageProps: { posts } }
		}
	];
}

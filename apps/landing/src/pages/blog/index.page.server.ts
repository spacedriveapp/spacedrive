import { blogEnabled, getPosts } from './blog';

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

	const individualPosts = posts.map((post) => ({
		url: `/blog/${post.slug}`,
		pageContext: { pageProps: { post } }
	}));

	return [
		...individualPosts,
		{
			url: '/blog',
			pageContext: { pageProps: { posts } }
		}
	];
}

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

	const postPages = posts.map((post) => ({
		url: `/blog/${post.slug}`,
		pageContext: { pageProps: { post } }
	}));

	const postListPage = {
		url: '/blog',
		pageContext: { pageProps: { posts } }
	};

	return [postListPage, ...postPages];
}

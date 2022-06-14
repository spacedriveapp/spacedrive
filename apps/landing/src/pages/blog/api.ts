import GhostContentAPI from '@tryghost/content-api';

// Ghost key is a public key
const ghostKey = import.meta.env.VITE_CONTENT_API_KEY;
const ghostURL = import.meta.env.VITE_API_URL;

export const blogEnabled = !!(ghostURL && ghostKey);

export const api = blogEnabled
	? new GhostContentAPI({
			url: ghostURL,
			key: ghostKey,
			version: 'v4'
	  })
	: null;

export async function getPosts() {
	if (!api) {
		return [];
	}
	const posts = await api.posts
		.browse({
			include: ['tags', 'authors']
		})
		.catch(() => []);
	return posts;
}

export async function getPost(slug: string) {
	if (!api) {
		return null;
	}
	return await api.posts
		.read(
			{ slug },
			{
				include: ['tags', 'authors']
			}
		)
		.catch(() => null);
}

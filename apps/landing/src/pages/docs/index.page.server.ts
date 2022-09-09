import { getAllDocs } from './api';

export async function onBeforeRender() {
	const docs = await getAllDocs();

	return {
		pageContext: {
			pageProps: {
				docs
			}
		}
	};
}

export async function prerender() {
	const docs = await getAllDocs();

	const individualDocs = docs.map((doc) => ({
		url: `/docs/${doc.url}`,
		pageContext: { pageProps: { doc } }
	}));

	return [
		...individualDocs,
		{
			url: '/docs',
			pageContext: { pageProps: { docs } }
		}
	];
}

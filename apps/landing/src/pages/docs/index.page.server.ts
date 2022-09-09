import { getDocs, getDocsList } from './api';

export async function onBeforeRender() {
	const docsList = getDocsList();
	console.log({ docsList });
	return {
		pageContext: {
			pageProps: {
				// index page renders its own markdown
				// only give it the sidebar data
				docsList
			}
		}
	};
}

// pre-render all doc pages at the same time as index
export async function prerender() {
	console.log('Prerendering');
	const docs = getDocs();
	const docsList = getDocsList(docs);

	const individualDocs = docs.map((doc) => ({
		url: `/docs/${doc.url}`,
		pageContext: { pageProps: { data: { doc, docsList } } }
	}));

	return [
		...individualDocs,
		{
			url: '/docs',
			pageContext: { pageProps: { docsList } }
		}
	];
}

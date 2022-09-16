import { getDocs, getDocsNavigation } from './api';
import config from './docs';

export async function onBeforeRender() {
	const navigation = getDocsNavigation(config);

	return {
		pageContext: {
			pageProps: {
				// index page renders its own content/markdown
				// only give it the sidebar data
				navigation
			}
		}
	};
}

// pre-render all doc pages at the same time as index
export async function prerender() {
	const docs = getDocs(config);
	const navigation = getDocsNavigation(config, docs);

	const docsArray = Object.keys(docs).map((url) => ({
		url: `/docs/${url}/`,
		pageContext: { pageProps: { data: { doc: docs[url], navigation } } }
	}));

	return [
		...docsArray,
		{
			url: '/docs',
			pageContext: { pageProps: { navigation } }
		}
	];
}

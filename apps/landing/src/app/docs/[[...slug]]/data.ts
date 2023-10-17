import { allDocs } from '@contentlayer/generated';
import { getDocsNavigation } from '~/utils/contentlayer';

const navigation = getDocsNavigation(allDocs);

export function getDoc(params: string[]) {
	const slug = params.join('/');

	const doc = allDocs.find((doc) => doc.slug === slug);

	if (!doc) {
		return {
			notFound: true
		};
	}

	const docNavigation = getDocsNavigation(allDocs);

	// TODO: Doesn't work properly (can't skip categories)
	const docIndex = docNavigation
		.find((sec) => sec.slug == doc.section)
		?.categories.find((cat) => cat.slug == doc.category)
		?.docs.findIndex((d) => d.slug == doc.slug);

	const nextDoc =
		docNavigation
			.find((sec) => sec.slug == doc.section)
			?.categories.find((cat) => cat.slug == doc.category)?.docs[(docIndex || 0) + 1] || null;

	return {
		navigation: docNavigation,
		doc,
		nextDoc
	};
}

export const navigationMeta = navigation.map((section) => ({
	slug: section.slug,
	categories: section.categories.map((category) => ({
		...category,
		docs: category.docs.map((doc) => ({
			url: doc.url,
			slug: doc.slug,
			title: doc.title
		}))
	}))
}));

import { Doc, DocumentTypes } from '@contentlayer/generated';
import { type Icon } from '@phosphor-icons/react';
import { Circle, Cube, Sparkle, Star } from '@phosphor-icons/react/dist/ssr';

import { toTitleCase } from './misc';

type DocsCategory = {
	title: string;
	slug: string;
	categoryIndex: number;
	docs: CoreContent<Doc>[];
};

type DocsSection = {
	slug: string;
	categories: DocsCategory[];
};

export type DocsNavigation = DocsSection[];

export function getDocsNavigation(docs: Doc[]): DocsNavigation {
	const coreDocs = allCoreContent(docs);

	const docsNavigation: DocsNavigation = [];

	const docsBySection = coreDocs.reduce(
		(acc, doc) => {
			const section = doc.section;
			acc[section] = acc[section] || [];
			acc[section].push(doc);
			return acc;
		},
		{} as Record<string, CoreContent<Doc>[]>
	);

	// console.log('docsBySection', docsBySection);

	for (const section in docsBySection) {
		const docs = docsBySection[section];
		const docsByCategory = docs.reduce(
			(acc, doc) => {
				const category = doc.category;
				acc[category] = acc[category] || [];
				acc[category].push(doc);
				return acc;
			},
			{} as Record<string, CoreContent<Doc>[]>
		);

		// console.log('docsByCategory', docsByCategory);

		const sectionNavigation: DocsCategory[] = [];
		for (const category in docsByCategory) {
			const docs = docsByCategory[category];
			docs.sort((a, b) => a.index - b.index);
			const categoryIndex = docs[0].index;
			sectionNavigation.push({
				title: toTitleCase(category),
				slug: category,
				categoryIndex,
				docs
			});
		}

		sectionNavigation.sort((a, b) => a.categoryIndex - b.categoryIndex);

		docsNavigation.push({
			slug: section,
			categories: sectionNavigation
		});
	}

	// Sort the sections using the order on iconConfig
	docsNavigation.sort((a, b) => {
		const aIndex = Object.keys(iconConfig).indexOf(a.slug);
		const bIndex = Object.keys(iconConfig).indexOf(b.slug);
		return aIndex - bIndex;
	});

	return docsNavigation;
}

// Used to get the icon from section name and for sorting
export const iconConfig: Record<string, Icon> = {
	product: Sparkle,
	developers: Cube,
	company: Circle,
	changelog: Star
};

export function getSortedDocs(docs: Doc[]) {
	return docs.sort((a, b) => a.index - b.index);
}

// This function is used to omit the fields that are not needed in the sidebar navigation
export const omit = <Obj, Keys extends keyof Obj>(obj: Obj, keys: Keys[]): Omit<Obj, Keys> => {
	const result = Object.assign({}, obj);
	keys.forEach((key) => {
		delete result[key];
	});
	return result;
};

export type CoreContent<T> = Omit<T, 'body' | '_raw' | '_id'>;

export function coreContent<T extends DocumentTypes>(content: T) {
	return omit(content, ['body', '_raw', '_id']);
}

export function allCoreContent<T extends DocumentTypes>(contents: T[]) {
	return contents.map((c) => coreContent(c));
}

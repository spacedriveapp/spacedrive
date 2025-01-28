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
	if (!Array.isArray(docs)) return [];

	const coreDocs = allCoreContent(docs);
	if (!coreDocs.length) return [];

	const docsNavigation: DocsNavigation = [];

	const docsBySection = coreDocs.reduce(
		(acc, doc) => {
			if (!doc?.section) return acc;
			const section = doc.section;
			acc[section] = acc[section] || [];
			acc[section].push(doc);
			return acc;
		},
		{} as Record<string, CoreContent<Doc>[]>
	);

	for (const section in docsBySection) {
		if (!docsBySection[section]?.length) continue;

		const docs = docsBySection[section];
		const docsByCategory = docs.reduce(
			(acc, doc) => {
				if (!doc?.category) return acc;
				const category = doc.category;
				acc[category] = acc[category] || [];
				acc[category].push(doc);
				return acc;
			},
			{} as Record<string, CoreContent<Doc>[]>
		);

		const categories = Object.entries(docsByCategory)
			.filter(([_, docs]) => Array.isArray(docs) && docs.length > 0)
			.map(([category, docs]) => ({
				title: toTitleCase(category),
				slug: category,
				categoryIndex: docs[0]?.index ?? 0,
				docs: docs.sort((a, b) => (a.index ?? 0) - (b.index ?? 0))
			}))
			.sort((a, b) => a.categoryIndex - b.categoryIndex);

		if (categories.length > 0) {
			docsNavigation.push({
				slug: section,
				categories
			});
		}
	}

	return docsNavigation;
}

// Used to get the icon from section name and for sorting
export const iconConfig: Record<string, Icon> = {
	product: Sparkle,
	developers: Cube,
	company: Star,
	changelog: Circle
};

export function getSortedDocs(docs: Doc[]) {
	return docs.sort((a, b) => (a.index ?? 0) - (b.index ?? 0));
}

function omit<Obj extends object, Keys extends keyof Obj>(obj: Obj, keys: Keys[]): Omit<Obj, Keys> {
	const result = { ...obj };
	keys.forEach((key) => delete result[key]);
	return result;
}

type CoreContent<T> = Omit<T, 'body' | '_raw' | '_id'>;

function coreContent<T extends DocumentTypes>(content: T): CoreContent<T> {
	return omit(content, ['body', '_raw', '_id']);
}

function allCoreContent<T extends DocumentTypes>(contents: T[]) {
	if (!Array.isArray(contents)) return [];
	return contents.map((c) => coreContent(c));
}

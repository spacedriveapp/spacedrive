import { Component } from 'react';
import { parseMarkdown } from '../../utils/markdownParse';

export interface Doc {
	title: string;
	slug: string;
	sortByIndex: number;
	url: string;
	active?: boolean;
	html?: string;
	categoryName: string;
}

export interface DocSectionConfig {
	title: string;
	slug: string;
	icon?: Component | any;
}

export interface DocsConfig {
	sections: DocSectionConfig[];
	docs: Record<string, string>;
}

// Just the metadata for a single doc
export type DocMetadata = Omit<Doc, 'html'>;

export interface DocCategory {
	title: string;
	slug: string;
	index: number;
	category: DocMetadata[];
}

export interface DocSection extends Omit<DocSectionConfig, 'icon'> {
	section: DocCategory[];
}

// this is consumed by the frontend
export type DocsNavigation = DocSection[];

const DEFAULT_INDEX = 100;

export function getDocs(config: DocsConfig): Record<string, Doc> {
	const parsedDocs: Record<string, Doc> = {};
	Object.keys(config.docs).forEach((path) => {
		const url = parsePath(path);
		if (!url) return null;

		const { render, metadata } = parseMarkdown(config.docs[path]);

		parsedDocs[url] = {
			title: metadata?.name ?? toTitleCase(url.split('/')[2]),
			slug: url.split('/')[2],
			url,
			categoryName: toTitleCase(url.split('/')[1]),
			sortByIndex: metadata?.index ?? DEFAULT_INDEX,
			html: render
		};
	});

	return parsedDocs;
}

export function getDocsNavigation(config: DocsConfig, docs?: Record<string, Doc>): DocsNavigation {
	docs = docs ?? getDocs(config);

	const navigation: DocsNavigation = [];

	for (const section of config.sections) {
		let categories: DocCategory[] = [];
		for (const [url, doc] of Object.entries(docs)) {
			if (!url.startsWith(section.slug)) continue;

			const clonedDoc = { ...doc };
			// remove html so the sidebar doesn't have all the doc data
			delete clonedDoc.html;

			const category = url.split('/')[1],
				title = toTitleCase(category),
				existingCategory = categories.findIndex((i) => i.slug === category);

			if (existingCategory != -1) {
				categories[existingCategory].category.push(clonedDoc);
			} else {
				categories.push({
					title,
					slug: category,
					index: DEFAULT_INDEX,
					category: [clonedDoc]
				});
			}
		}
		categories = categories
			.map((cat) => {
				// sort by index
				cat.category.sort((a, b) => a.sortByIndex - b.sortByIndex);
				return cat;
			})
			// sort categories smallest first doc's index
			.sort((a, b) => a.category[0].sortByIndex - b.category[0].sortByIndex);

		navigation.push({
			title: section.title,
			slug: section.slug,
			section: categories
		});
	}

	return navigation;
}

export interface SingleDocResponse {
	doc?: Doc;
	navigation: DocsNavigation;
	nextDoc?: { url: string; title: string };
}
// get a single doc, and the sidebar data
export function getDoc(url: string, config: DocsConfig): SingleDocResponse {
	const docs = getDocs(config),
		navigation = getDocsNavigation(config, docs),
		nextDoc = getNextDoc(navigation, url);

	return {
		doc: docs[url],
		navigation,
		nextDoc: nextDoc
			? {
					url: nextDoc.url,
					title: nextDoc.title
			  }
			: undefined
	};
}

function parsePath(path: string): string | null {
	const url = path.split('docs/')[1].split('.md')[0];
	if (!url.includes('/')) return null;
	return url;
}

export function toTitleCase(str: string) {
	return str
		.toLowerCase()
		.replace(/(?:^|[\s-/])\w/g, function (match) {
			return match.toUpperCase();
		})
		.replaceAll('-', ' ');
}

type DocUrls = { url: string; title: string }[];

export function getNextDoc(
	navigation: DocsNavigation,
	docUrl: string
): { url: string; title: string } {
	const docUrls: DocUrls = [];
	// flatten the navigation
	for (const section of navigation) {
		for (const category of section.section) {
			for (const doc of category.category) {
				docUrls.push({ url: doc.url, title: doc.title });
			}
		}
	}
	const currentDocIndex = docUrls.findIndex((doc) => doc.url === docUrl);
	return docUrls[currentDocIndex + 1];
}

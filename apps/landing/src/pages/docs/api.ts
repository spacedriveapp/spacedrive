import React from 'react';

import { parseMarkdown } from '../../utils/markdownParse';

export interface Doc {
	title: string;
	name?: string;
	sortByIndex: number;
	url: string;
	active?: boolean;
	html?: string;
	categoryName?: string;
}

export interface DocSectionConfig {
	title: string;
	slug: string;
	color?: string;
	icon?: React.Component | any;
}

export interface DocsConfig {
	sections: DocSectionConfig[];
	docs: Record<string, string>;
}

// Just the metadata for a single doc
export type DocMetadata = Omit<Doc, 'html'>;

export interface DocCategory {
	name: string;
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
			title: metadata?.name ?? cap(url.split('/')[2]),
			name: url.split('/')[2],
			url,
			categoryName: cap(url.split('/')[1]),
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
				existingCategory = categories.findIndex((i) => i.name === cap(category));

			if (existingCategory != -1) {
				categories[existingCategory].category.push(clonedDoc);
			} else {
				categories.push({
					name: cap(category),
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
			color: section.color,
			section: categories
		});
	}

	return navigation;
}

// get a single doc, and the sidebar data
export function getDoc(url: string, config: DocsConfig): { doc?: Doc; navigation: DocsNavigation } {
	const docs = getDocs(config);

	const doc = docs[url];

	return {
		doc,
		navigation: getDocsNavigation(config, docs)
	};
}

function parsePath(path: string): string | null {
	const url = path.split('docs/')[1].split('.md')[0];
	if (!url.includes('/')) return null;
	return url;
}
function cap(string: string) {
	return string.charAt(0).toUpperCase() + string.slice(1);
}

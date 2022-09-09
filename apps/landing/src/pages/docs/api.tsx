import GhostContentAPI from '@tryghost/content-api';
import React from 'react';
import ReactDOMServer from 'react-dom/server';

import { parseMarkdown } from '../../utils/markdownParse';

export interface Doc {
	title: string;
	url: string;
	active?: boolean;
	html?: string;
}

export type DocItem = Omit<Doc, 'html'>;

export interface SidebarCategory {
	name: string;
	items: DocItem[];
}

export type DocsList = SidebarCategory[];

export interface SingleDocResponse {
	doc: Doc | undefined;
	docsList: DocsList;
}

// load all docs from /docs directory and parse markdown
export function getDocs(): Doc[] {
	const docs = import.meta.globEager('../../../../../docs/**/*.md');
	const docsRaw = import.meta.globEager('../../../../../docs/**/*.md', { as: 'raw' });

	const paths = Object.keys(docs);

	return paths
		.map((path) => {
			const url = parsePath(path);
			if (!url) return null;

			const Component: any = docs[path]?.ReactComponent;

			const markdown = ReactDOMServer.renderToString(<Component />);

			const { render } = parseMarkdown(markdown, docsRaw[path] as unknown as string);

			return {
				title: url.split('/')[1],
				url: url,
				html: render
			};
		})
		.filter((i) => i) as Doc[];
}

// build a list of docs sorted by category, excluded html data
// can accept docs if already fetched (see usage index.page.server.ts)
export function getDocsList(docs?: Doc[]) {
	if (!docs) docs = getDocs();
	const categories: DocsList = [];
	for (let doc of docs) {
		doc = { ...doc };
		// remove html so the sidebar doesn't have all the doc data
		delete doc.html;
		const { category } = docInfo(doc.url),
			existingCategory = categories.findIndex((i) => i.name === cap(category));

		if (existingCategory != -1) {
			categories[existingCategory].items.push(doc);
		} else {
			categories.push({
				name: cap(category),
				items: [doc]
			});
		}
	}
	return categories;
}

// get a single doc, and the sidebar data
export function getDoc(slug: string): SingleDocResponse {
	const { name } = docInfo(slug),
		docs = getDocs(),
		doc = docs.find((d) => d.title === name);

	return {
		doc,
		docsList: getDocsList(docs)
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
function docInfo(url: string) {
	return { category: url.split('/')[0], name: url.split('/')[1] };
}

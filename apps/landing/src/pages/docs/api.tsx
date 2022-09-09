import GhostContentAPI from '@tryghost/content-api';
import React from 'react';
import ReactDOMServer from 'react-dom/server';

const _docs = import.meta.globEager('../../../../../docs/**/*.md');

export interface Doc {
	title: string;
	url: string;
	html: string;
}
export interface SidebarItem {
	title: string;
	path: string;
	active: boolean;
}
export interface SidebarCategory {
	name: string;
	items: SidebarItem[];
}

const paths = Object.keys(_docs);

const docs: Doc[] = paths.map((path) => {
	const url = path.split('docs/')[1].split('.md')[0];

	const Component: any = _docs[path]?.ReactComponent;
	return {
		title: url.split('/')[1],
		url: url.replaceAll('/index', ''),
		html: ReactDOMServer.renderToString(<Component />)
	};
});

const sidebar: SidebarCategory[] = [];

function cap(string: string) {
	return string.charAt(0).toUpperCase() + string.slice(1);
}

console.log({ paths });

for (const path of paths) {
	const url = path.split('docs/')[1].split('.md')[0];
	const category = cap(url.split('/')[0]);
	const name = cap(url.split('/')[1]);
	const existingCategory = sidebar.findIndex((i) => i.name === category);
	if (existingCategory != -1) {
		sidebar[existingCategory].items.push({
			title: name,
			path: url,
			active: false
		});
	} else {
		sidebar.push({
			name: category,
			items: [
				{
					title: name,
					path: url,
					active: false
				}
			]
		});
	}
}

export function getAllDocs() {
	return docs;
}

export function getDoc(slug: string) {
	const doc = docs.find((d) => d.url === slug);

	return doc;
}

export function getSidebar() {
	return sidebar;
}

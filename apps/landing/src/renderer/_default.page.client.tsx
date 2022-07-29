import React from 'react';
import { Root, createRoot, hydrateRoot } from 'react-dom/client';
import type { PageContextBuiltInClient } from 'vite-plugin-ssr/client/router';

import { App } from '../App';
import { PageContext } from './types';

export const clientRouting = true;

let root: Root;
export async function render(pageContext: PageContextBuiltInClient & PageContext) {
	const { Page, pageProps } = pageContext;
	const page = (
		<App pageContext={pageContext as any}>
			<Page {...pageProps} />
		</App>
	);
	const container = document.getElementById('page-view')!;
	if (pageContext.isHydration) {
		root = hydrateRoot(container, page);
	} else {
		if (!root) {
			root = createRoot(container);
		}
		root.render(page);
	}
}

import React from 'react';
import ReactDOM from 'react-dom';
import { Root, createRoot, hydrateRoot } from 'react-dom/client';
import { useClientRouter } from 'vite-plugin-ssr/client/router';

import { App } from '../main';

let root: Root;

const pageContext = useClientRouter({
	async render(pageContext) {
		const { Page, pageProps } = pageContext;
		const app = <App />;
		const container = document.getElementById('root')!;
		if (pageContext.isHydration) {
			root = hydrateRoot(container, app);
		} else {
			if (!root) {
				root = createRoot(container);
			}
			root.render(app);
		}
	}
});

import React from 'react';
import { Root, createRoot, hydrateRoot } from 'react-dom/client';
import { useClientRouter } from 'vite-plugin-ssr/client/router';
import type { PageContextBuiltInClient } from 'vite-plugin-ssr/client/router';

import { App } from '../App';
import type { PageContext } from './types';

let root: Root;
const { hydrationPromise } = useClientRouter({
	render(pageContext: PageContextBuiltInClient & PageContext) {
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
	// onTransitionStart,
	// onTransitionEnd
});

hydrationPromise.then(() => {
	console.log('Hydration finished; page is now interactive.');
});

// function onTransitionStart() {
// 	console.log('Page transition start');
// 	document.getElementById('page-view')!.classList.add('page-transition');
// }
// function onTransitionEnd() {
// 	console.log('Page transition end');
// 	document.getElementById('#page-content')!.classList.remove('page-transition');
// }

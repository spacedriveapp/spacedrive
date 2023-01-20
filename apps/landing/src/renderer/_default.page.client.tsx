import { hydrateRoot } from 'react-dom/client';
import type { PageContextBuiltInClient } from 'vite-plugin-ssr/client';
import App from '../App';
import type { PageContext } from './types';

export { render };

// Enable Client Routing
export const clientRouting = true;

// See `Link prefetching` section below. Default value: `{ when: 'HOVER' }`.
export const prefetchStaticAssets = { when: 'HOVER' };

async function render(pageContext: PageContextBuiltInClient & PageContext) {
	const { Page, pageProps } = pageContext;
	hydrateRoot(
		document.getElementById('page-view')!,
		<App pageContext={pageContext as any}>
			<Page {...pageProps} />
		</App>
	);
}

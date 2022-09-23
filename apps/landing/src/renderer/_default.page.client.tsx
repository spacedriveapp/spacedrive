import { hydrateRoot } from 'react-dom/client';
import type { PageContextBuiltInClient } from 'vite-plugin-ssr/client';

import App from '../App';
import type { PageContext } from './types';

export { render };

async function render(pageContext: PageContextBuiltInClient & PageContext) {
	const { Page, pageProps } = pageContext;
	hydrateRoot(
		document.getElementById('page-view')!,
		<App pageContext={pageContext as any}>
			<Page {...pageProps} />
		</App>
	);
}

export const clientRouting = true;

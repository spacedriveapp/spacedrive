import React from 'react';
import ReactDOMServer from 'react-dom/server';
import { dangerouslySkipEscape, escapeInject } from 'vite-plugin-ssr';
import type { PageContextBuiltIn } from 'vite-plugin-ssr';

import { App } from '../App';
import type { PageContext } from './types';

export { render };
// See https://vite-plugin-ssr.com/data-fetching
export const passToClient = ['pageProps', 'urlPathname'];

async function render(pageContext: PageContextBuiltIn & PageContext) {
	const { Page, pageProps } = pageContext;
	const pageHtml = ReactDOMServer.renderToString(
		<App pageContext={pageContext}>
			<Page {...pageProps} />
		</App>
	);

	// See https://vite-plugin-ssr.com/head
	const { documentProps } = pageContext;
	const title = (documentProps && documentProps.title) || 'Vite SSR app';
	const desc = (documentProps && documentProps.description) || 'App using Vite + vite-plugin-ssr';

	const documentHtml = escapeInject`<!DOCTYPE html>
    <html lang="en" class="dark">
      <head>
		<meta charset="UTF-8" />
		<link rel="icon" type="image/svg+xml" href="/favicon.ico" />
		<meta name="viewport" content="width=device-width, initial-scale=1.0" />
		<title>Spacedrive — A file manager from the future.</title>
		<meta
			name="description"
			content="Combine your drives and clouds into one database that you can organize and explore from any device. Designed for creators, hoarders and the painfully disorganized."
		/>
		<meta
			name="og:image"
			content="https://raw.githubusercontent.com/spacedriveapp/.github/main/profile/spacedrive_icon.png"
		/>
		<meta name="theme-color" content="#E751ED" media="not screen" />
		<meta
			name="keywords"
			content="files,file manager,spacedrive,file explorer,vdfs,distributed filesystem,cas,content addressable storage,virtual filesystem,photos app, video organizer,video encoder,tags,tag based filesystem"
		/>
		<meta name="author" content="Spacedrive Technology Inc." />
		<meta name="robots" content="index, follow" />
      </head>
      <body>
        <div id="page-view">${dangerouslySkipEscape(pageHtml)}</div>
      </body>
    </html>`;

	return {
		documentHtml,
		pageContext: {
			// We can add some `pageContext` here, which is useful if we want to do page redirection https://vite-plugin-ssr.com/page-redirection
		}
	};
}

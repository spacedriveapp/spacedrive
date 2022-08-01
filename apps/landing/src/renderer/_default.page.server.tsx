import React from 'react';
import ReactDOMServer from 'react-dom/server';
import { Helmet } from 'react-helmet';
import { dangerouslySkipEscape, escapeInject } from 'vite-plugin-ssr';
import type { PageContextBuiltIn } from 'vite-plugin-ssr';

import App from '../App';
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

	const helmet = Helmet.renderStatic();

	// See https://vite-plugin-ssr.com/head
	const { documentProps } = pageContext;

	const documentHtml = escapeInject`
		<!DOCTYPE html>
	    <html lang="en" class="dark" ${dangerouslySkipEscape(helmet.htmlAttributes.toString())}>
	    <head>
				<meta charset="UTF-8" />
				<link rel="icon" type="image/svg+xml" href="/favicon.ico" />
				<meta name="viewport" content="width=device-width, initial-scale=1.0" />
				<meta name="theme-color" content="#E751ED" media="not screen" />
				<meta name="robots" content="index, follow" />
				${dangerouslySkipEscape(helmet.title.toString())}
				${dangerouslySkipEscape(helmet.meta.toString())}
				${dangerouslySkipEscape(helmet.link.toString())}
	    </head>
	      <body ${dangerouslySkipEscape(helmet.bodyAttributes.toString())}>
	        <div id="page-view">${dangerouslySkipEscape(pageHtml)}</div>
	      </body>
	    </html>
		`;

	return {
		documentHtml,
		pageContext: {
			// We can add some `pageContext` here, which is useful if we want to do page redirection https://vite-plugin-ssr.com/page-redirection
		}
	};
}

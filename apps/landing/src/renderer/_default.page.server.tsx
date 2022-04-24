import ReactDOMServer from 'react-dom/server';
import { Shell } from './Shell';
import { escapeInject, dangerouslySkipEscape } from 'vite-plugin-ssr';
import logoUrl from './logo.svg';
import type { PageContext } from './types';
import type { PageContextBuiltIn } from 'vite-plugin-ssr';

export { render };
// See https://vite-plugin-ssr.com/data-fetching
export const passToClient = ['pageProps', 'urlPathname'];

async function render(pageContext: PageContextBuiltIn & PageContext) {
  const { Page, pageProps } = pageContext;
  const pageHtml = ReactDOMServer.renderToString(
    <Shell pageContext={pageContext}>
      <Page {...pageProps} />
    </Shell>
  );

  // See https://vite-plugin-ssr.com/head
  const { documentProps } = pageContext;
  const title = (documentProps && documentProps.title) || 'Vite SSR app';
  const desc = (documentProps && documentProps.description) || 'App using Vite + vite-plugin-ssr';

  const documentHtml = escapeInject`<!DOCTYPE html>
    <html lang="en">
      <head>
        <meta charset="UTF-8" />
        <link rel="icon" href="${logoUrl}" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <meta name="description" content="${desc}" />
        <title>${title}</title>
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

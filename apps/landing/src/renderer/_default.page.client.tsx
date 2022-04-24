import ReactDOM from 'react-dom';
import React from 'react';
import { getPage } from 'vite-plugin-ssr/client';
import { Shell } from './Shell';
import type { PageContext } from './types';
import type { PageContextBuiltInClient } from 'vite-plugin-ssr/client';

hydrate();

async function hydrate() {
  // We do Server Routing, but we can also do Client Routing by using `useClientRouter()`
  // instead of `getPage()`, see https://vite-plugin-ssr.com/useClientRouter
  const pageContext = await getPage<PageContextBuiltInClient & PageContext>();
  const { Page, pageProps } = pageContext;
  ReactDOM.hydrate(
    <Shell pageContext={pageContext}>
      <Page {...pageProps} />
    </Shell>,
    document.getElementById('page-view')
  );
}

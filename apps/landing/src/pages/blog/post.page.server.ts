import { PageContextBuiltIn } from 'vite-plugin-ssr';

import { getPost, getPosts } from './api';

export async function onBeforeRender(pageContext: PageContextBuiltIn) {
  const post = await getPost(pageContext.routeParams['slug']);

  return {
    pageContext: {
      pageProps: {
        post
      }
    }
  };
}

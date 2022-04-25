import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
//@ts-expect-error
import svg from 'vite-plugin-svgr';
import pages from 'vite-plugin-pages';
import md, { Mode } from 'vite-plugin-markdown';
import generateSitemap from 'vite-plugin-pages-sitemap';

// https://vitejs.dev/config/
export default defineConfig({
  // @ts-ignore
  plugins: [
    react(),
    svg({ svgrOptions: { icon: true } }),
    pages({
      dirs: 'src/pages'
      // onRoutesGenerated: (routes) => generateSitemap({ routes })
    }),
    md({ mode: [Mode.REACT] })
  ],
  server: {
    port: 8003
  },
  publicDir: 'public'
});

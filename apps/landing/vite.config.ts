import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
// @ts-expect-error
import svg from 'vite-plugin-svgr';
import ssr from 'vite-plugin-ssr/plugin';
// https://vitejs.dev/config/
export default defineConfig({
  // @ts-ignore
  plugins: [react(), svg({ svgrOptions: { icon: true } }), ssr()],
  server: {
    port: 8003
  },
  publicDir: 'public'
});

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { name, version } from './package.json';
import svg from 'vite-plugin-svgr';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8002
  },
  plugins: [
    //@ts-ignore - no idea why one moment this errors, next its fine. all on same version.
    react({
      jsxRuntime: 'classic'
    }),
    svg({ svgrOptions: { icon: true } })
  ],
  root: 'src',
  publicDir: '../../packages/interface/src/assets',
  define: {
    pkgJson: { name, version }
  },
  build: {
    outDir: '../dist',
    assetsDir: '.'
  }
});

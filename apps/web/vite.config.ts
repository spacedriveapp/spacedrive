import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from 'vite-plugin-tsconfig-paths';

import { name, version } from './package.json';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8002
  },
  plugins: [
    // @ts-ignore
    react({
      jsxRuntime: 'classic'
    }),
    svg({ svgrOptions: { icon: true } }),
    tsconfigPaths()
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

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { name, version } from './package.json';
import * as path from 'path';
import svgr from '@honkhonk/vite-plugin-svgr';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8001
  },
  plugins: [
    react({
      jsxRuntime: 'classic'
    }),
    svgr()
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

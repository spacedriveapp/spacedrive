import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';
import react from '@vitejs/plugin-react';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8085
  },
  plugins: [
    react({
      jsxRuntime: 'classic'
    }),
    tsconfigPaths()
  ],
  esbuild: {
    jsxInject: 'import {jsx as _jsx} from "react/jsx-runtime"'
  },
  root: 'src',
  publicDir: '../../packages/interface/src/assets',
  build: {
    outDir: '../dist',
    emptyOutDir: false,
    assetsDir: '.'
  },
  base: ''
});

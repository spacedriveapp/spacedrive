import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import svgr from '@honkhonk/vite-plugin-svgr';

import * as path from 'path';

// Vite configured in "Library Mode", it will not run as a server.
// https://vitejs.dev/config/
export default defineConfig({
  // for developer purposes only
  server: {
    port: 8003
  },
  plugins: [
    react({
      jsxRuntime: 'classic'
    }),
    svgr()
  ],
  esbuild: {
    jsxInject: 'import {jsx as _jsx} from "react/jsx-runtime"'
  },
  root: 'src',
  publicDir: './assets',
  build: {
    lib: {
      entry: path.resolve(__dirname, 'src', 'index.ts'),
      formats: ['es', 'cjs'],
      fileName: (ext) => `index.${ext}.js`,
      name: 'SpacedriveInterface'
    },
    outDir: path.resolve(__dirname, 'dist'),
    rollupOptions: {
      input: {
        index: path.resolve(__dirname, 'src', 'index.ts')
      },
      external: ['react', 'react-dom'],
      output: {
        globals: {
          'react': 'React',
          'react-dom': 'ReactDOM'
        }
      }
    },
    target: 'esnext',
    sourcemap: true
  }
});

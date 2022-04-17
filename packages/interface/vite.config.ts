import { defineConfig } from 'vite';
import reactSvgPlugin from 'vite-plugin-react-svg';
import react from '@vitejs/plugin-react';

import * as path from 'path';

// Vite configured in "Library Mode", it will not run as a server.
// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    react({
      jsxRuntime: 'classic'
    }),
    reactSvgPlugin()
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

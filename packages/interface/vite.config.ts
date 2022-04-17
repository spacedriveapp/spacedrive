import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';
import filterReplace from 'vite-plugin-filter-replace';
import reactRefresh from '@vitejs/plugin-react-refresh';
import reactSvgPlugin from 'vite-plugin-react-svg';
import react from '@vitejs/plugin-react';

import * as path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8085
  },
  plugins: [
    react({
      jsxRuntime: 'classic'
    })
    // reactRefresh(),
    // tsconfigPaths(),
    // reactSvgPlugin(),
    // filterReplace([
    //   {
    //     filter: /\.js$/,
    //     replace: {
    //       // this is a hotfix for broken import in react-virtualized
    //       from: `import { bpfrpt_proptype_WindowScroller } from "../WindowScroller.js";`,
    //       to: ''
    //     }
    //   }
    // ])
  ],
  esbuild: {
    jsxInject: 'import {jsx as _jsx} from "react/jsx-runtime"'
  },
  root: 'src',
  publicDir: './assets',
  build: {
    lib: {
      entry: path.resolve(__dirname, 'src', 'index.ts'),
      formats: ['es'],
      fileName: (ext) => `index.${ext}.js`
    },
    outDir: path.resolve(__dirname, 'dist'),
    rollupOptions: {
      input: {
        index: path.resolve(__dirname, 'src', 'index.ts')
      }
    },
    target: 'esnext',
    sourcemap: true
  },
  base: ''
});

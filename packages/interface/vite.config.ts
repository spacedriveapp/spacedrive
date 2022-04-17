import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';
import filterReplace from 'vite-plugin-filter-replace';
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
    tsconfigPaths(),
    reactSvgPlugin()
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
      formats: ['es', 'cjs'],
      fileName: (ext) => `index.${ext}.js`,
      name: 'SpacedriveInterface'
    },
    outDir: path.resolve(__dirname, 'dist'),
    rollupOptions: {
      input: {
        index: path.resolve(__dirname, 'src', 'index.ts')
      },
      external: ['react', 'react-dom']
    },
    target: 'esnext',
    sourcemap: true
  },
  base: ''
});

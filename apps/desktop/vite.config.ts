import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';
import filterReplace from 'vite-plugin-filter-replace';
import reactRefresh from '@vitejs/plugin-react-refresh';
import reactSvgPlugin from 'vite-plugin-react-svg';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8085
  },
  plugins: [
    reactRefresh(),
    tsconfigPaths(),
    reactSvgPlugin(),
    filterReplace([
      {
        filter: /\.js$/,
        replace: {
          // this is a hotfix for broken import in react-virtualized
          from: `import { bpfrpt_proptype_WindowScroller } from "../WindowScroller.js";`,
          to: ''
        }
      }
    ])
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

import { defineConfig } from 'vite';
import reactRefresh from '@vitejs/plugin-react-refresh';
import tsconfigPaths from 'vite-tsconfig-paths';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8085
  },
  plugins: [reactRefresh(), tsconfigPaths()],
  esbuild: {
    jsxInject: 'import {jsx as _jsx} from "react/jsx-runtime"'
  },
  root: 'src',
  publicDir: 'assets',
  build: {
    outDir: '../dist',
    emptyOutDir: false,
    assetsDir: '.'
  },
  base: ''
});

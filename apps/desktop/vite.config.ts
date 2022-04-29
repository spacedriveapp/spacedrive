import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { name, version } from './package.json';
import svg from '@sd/vite';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8001
  },
  plugins: [
    // @ts-ignore
    react({
      jsxRuntime: 'classic'
    }),
    //@ts-ignore
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

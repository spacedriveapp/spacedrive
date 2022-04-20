import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { name, version } from './package.json';
import * as path from 'path';
import svgr from '@honkhonk/vite-plugin-svgr';

function resolvePackage(name: string) {
  return path.resolve(require.resolve(`${name}/package.json`), '../src');
}
// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8002
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
  optimizeDeps: {
    include: ['@sd/interface', '@sd/ui', '@sd/client']
  },
  resolve: {
    alias: {
      '@sd/interface': resolvePackage('@sd/interface'),
      '@sd/ui': resolvePackage('@sd/ui'),
      '@sd/client': resolvePackage('@sd/client')
    }
  },
  build: {
    outDir: '../dist',
    assetsDir: '.'
  }
});

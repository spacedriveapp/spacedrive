import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    port: 8002
  },
  plugins: [
    react({
      jsxRuntime: 'classic'
    })
  ],
  publicDir: 'public',
  build: {
    outDir: 'build',
    assetsDir: '.'
  }
});

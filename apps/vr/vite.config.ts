import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import basicSsl from '@vitejs/plugin-basic-ssl';
import { resolve } from 'path';

export default defineConfig({
  plugins: [react(), basicSsl()],
  server: {
    host: '0.0.0.0',
    port: 5173,
    https: true,
  },
  resolve: {
    dedupe: ['react', 'react-dom', 'three'],
    alias: {
      '@sd/interface': resolve(__dirname, '../../packages/interface/src'),
      '@sd/ts-client': resolve(__dirname, '../../packages/ts-client/src'),
    },
  },
  optimizeDeps: {
    include: ['react', 'react-dom', 'three'],
    exclude: ['@sd/interface', '@sd/ts-client'],
  },
});

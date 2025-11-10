import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import path from 'path';

const COMMANDS = [
	'initialize_core',
	'core_rpc',
	'subscribe_events'
];

export default defineConfig(async () => ({
	plugins: [react()],

	css: {
		postcss: './postcss.config.cjs',
	},

	resolve: {
		alias: {
			'@sd/interface': path.resolve(__dirname, '../../packages/interface/src'),
			'@sd/ts-client': path.resolve(__dirname, '../../packages/ts-client/src'),
			'@sd/ui': path.resolve(__dirname, '../../packages/ui/src'),
		}
	},

	optimizeDeps: {
		include: ['rooks'],
	},

	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		watch: {
			ignored: ['**/src-tauri/**']
		}
	},
	envPrefix: ['VITE_', 'TAURI_ENV_*'],
	build: {
		target: ['es2021', 'chrome100', 'safari13'],
		minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
		sourcemap: !!process.env.TAURI_ENV_DEBUG
	}
}));

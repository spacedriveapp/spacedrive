import { createRequire } from 'node:module';

import baseConfig from '../../packages/config/vite';

const require = createRequire(import.meta.url);

// https://vitejs.dev/config/
export default {
	...baseConfig,
	resolve: {
		alias: {
			crypto: require.resolve('rollup-plugin-node-builtins')
		}
	}
};

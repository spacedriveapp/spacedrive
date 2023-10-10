import { visualizer } from 'rollup-plugin-visualizer';
import { mergeConfig } from 'vite';

import baseConfig from '../../packages/config/vite';
import relativeAliasResolver from '../../packages/config/vite/relativeAliasResolver';

export default mergeConfig(baseConfig, {
	server: {
		port: 8002
	},
	resolve: {
		// BE REALLY DAMN CAREFUL MODIFYING THIS: https://github.com/spacedriveapp/spacedrive/pull/1353
		alias: [relativeAliasResolver]
	},
	plugins: [
		visualizer({
			gzipSize: true,
			brotliSize: true
		})
	]
});

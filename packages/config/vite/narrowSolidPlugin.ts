// Source: https://github.com/merged-js/react-solid/blob/master/src/narrowSolidPlugin.ts
//
// We vendor it due to: https://github.com/merged-js/react-solid/issues/1
//

import { createFilter } from '@rollup/pluginutils';
import solidPlugin, { Options } from 'vite-plugin-solid';

export interface NarrowSolidPluginOptions extends Partial<Options> {
	include?: string | RegExp | Array<string> | Array<RegExp>;
	exclude?: string | RegExp | Array<string> | Array<RegExp>;
}

export function narrowSolidPlugin({ include, exclude, ...rest }: NarrowSolidPluginOptions = {}) {
	const plugin = solidPlugin(rest);
	const originalConfig = plugin.config!.bind(plugin);
	const filter = createFilter(include, exclude);
	plugin.config = (...args) => {
		const baseConfig = originalConfig(...args);
		return {
			...baseConfig,
			esbuild: {
				include: exclude,
				exclude: include
			}
		};
	};

	const originalTransform = (plugin.transform! as any).bind(plugin);
	plugin.transform = (source, id, ssr) => {
		if (!filter(id)) {
			return null;
		}

		return originalTransform(source, id, ssr);
	};

	return plugin;
}

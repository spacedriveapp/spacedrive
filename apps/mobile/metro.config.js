const { makeMetroConfig, resolveUniqueModule, exclusionList } = require('@rnx-kit/metro-config');

const path = require('path');

// Needed for transforming svgs from @sd/assets
const [reactSVGPath, reactSVGExclude] = resolveUniqueModule('react-native-svg');

const { getDefaultConfig } = require('expo/metro-config');
const expoDefaultConfig = getDefaultConfig(__dirname);

const projectRoot = __dirname;
const workspaceRoot = path.resolve(projectRoot, '../..');

const metroConfig = makeMetroConfig({
	...expoDefaultConfig,
	projectRoot,
	watchFolders: [workspaceRoot],
	resolver: {
		...expoDefaultConfig.resolver,
		extraNodeModules: {
			'react-native-svg': reactSVGPath
		},
		blockList: exclusionList([reactSVGExclude]),
		sourceExts: [...expoDefaultConfig.resolver.sourceExts, 'svg'],
		assetExts: expoDefaultConfig.resolver.assetExts.filter((ext) => ext !== 'svg'),
		disableHierarchicalLookup: true,
		nodeModulesPaths: [
			path.resolve(projectRoot, 'node_modules'),
			path.resolve(workspaceRoot, 'node_modules')
		],
		resolveRequest: (context, moduleName, platform) => {
			if (moduleName.startsWith('@rspc/client/v2')) {
				return {
					filePath: path.resolve(
						workspaceRoot,
						'node_modules',
						'@rspc',
						'client',
						'dist',
						'v2.js'
					),
					type: 'sourceFile'
				};
			}
			if (moduleName.startsWith('@rspc/react/v2')) {
				return {
					filePath: path.resolve(
						projectRoot,
						'node_modules',
						'@rspc',
						'react',
						'dist',
						'v2.js'
					),
					type: 'sourceFile'
				};
			}
			// Optionally, chain to the standard Metro resolver.
			return context.resolveRequest(context, moduleName, platform);
		},
		platforms: ['ios', 'android']
	},
	transformer: {
		...expoDefaultConfig.transformer,
		// Metro default is "uglify-es" but terser should be faster and has better defaults.
		minifierPath: 'metro-minify-terser',
		minifierConfig: {
			compress: {
				drop_console: true,
				// Sometimes improves performance?
				reduce_funcs: false
			},
			format: {
				ascii_only: true,
				wrap_iife: true,
				quote_style: 3
			}
		},
		getTransformOptions: async () => ({
			transform: {
				experimentalImportSupport: false,
				inlineRequires: true
			}
		}),
		babelTransformerPath: require.resolve('react-native-svg-transformer')
	}
});

module.exports = metroConfig;

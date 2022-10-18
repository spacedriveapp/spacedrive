const { makeMetroConfig, resolveUniqueModule, exclusionList } = require('@rnx-kit/metro-config');
const MetroSymlinksResolver = require('@rnx-kit/metro-resolver-symlinks');

// Might not need these anymore.
const [SDAssetsPath, SDAssetsPathExclude] = resolveUniqueModule('@sd/assets', '.');
const [babelRuntimePath, babelRuntimeExclude] = resolveUniqueModule('@babel/runtime');
const [reactPath, reactExclude] = resolveUniqueModule('react');

const path = require('path');

// Needed for transforming svgs from @sd/assets
const [reactSVGPath, reactSVGExclude] = resolveUniqueModule('react-native-svg');

const { getDefaultConfig } = require('expo/metro-config');
const expoDefaultConfig = getDefaultConfig(__dirname);

const projectRoot = __dirname;
const workspaceRoot = path.resolve(projectRoot, '../..');

const metroConfig = makeMetroConfig({
	projectRoot,
	watchFolders: [workspaceRoot],
	resolver: {
		...expoDefaultConfig.resolver,
		// resolveRequest: MetroSymlinksResolver(),
		extraNodeModules: {
			'@babel/runtime': babelRuntimePath,
			'@sd/assets': SDAssetsPath,
			'react': reactPath,
			'react-native-svg': reactSVGPath
		},
		blockList: exclusionList([
			babelRuntimeExclude,
			SDAssetsPathExclude,
			reactExclude,
			reactSVGExclude
		]),
		sourceExts: [...expoDefaultConfig.resolver.sourceExts, 'svg'],
		assetExts: expoDefaultConfig.resolver.assetExts.filter((ext) => ext !== 'svg'),
		disableHierarchicalLookup: true,
		nodeModulesPaths: [
			path.resolve(projectRoot, 'node_modules'),
			path.resolve(workspaceRoot, 'node_modules')
		]
	},
	transformer: {
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

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
		disableHierarchicalLookup: false,
		nodeModulesPaths: [
			path.resolve(projectRoot, 'node_modules'),
			path.resolve(workspaceRoot, 'node_modules')
		],
		platforms: ['ios', 'android']
	},
	transformer: {
		...expoDefaultConfig.transformer,
		getTransformOptions: async () => ({
			transform: {
				// What does this do?
				experimentalImportSupport: false,
				inlineRequires: true
			}
		}),
		babelTransformerPath: require.resolve('react-native-svg-transformer')
	}
});

module.exports = metroConfig;

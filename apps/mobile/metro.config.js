const { getDefaultConfig } = require("expo/metro-config");
const { withNativeWind } = require("nativewind/metro");
const path = require("path");

const projectRoot = __dirname;
const workspaceRoot = path.resolve(projectRoot, "../..");

const config = getDefaultConfig(projectRoot);

// Watch only the app sources and hoisted workspace deps Metro needs to resolve.
// Expo Router can resolve to files in the hoisted Bun node_modules tree.
config.watchFolders = [
	path.resolve(projectRoot, "src"),
	path.resolve(workspaceRoot, "packages"),
	path.resolve(workspaceRoot, "node_modules"),
];

// Configure resolver for monorepo and SVG support
config.resolver = {
	...config.resolver,

	// Treat SVG as source files (not assets)
	sourceExts: [...config.resolver.sourceExts, "svg"],
	assetExts: config.resolver.assetExts.filter((ext) => ext !== "svg"),

	// Critical for Bun monorepo - resolve node_modules from local and workspace root
	// Local node_modules takes priority to ensure correct React version
	nodeModulesPaths: [
		path.resolve(projectRoot, "node_modules"),
		path.resolve(workspaceRoot, "node_modules"),
	],

	// Exclude build outputs
	blockList: [
		/\/apps\/mobile\/ios\/build\/.*/,
		/\/apps\/mobile\/android\/build\/.*/,
	],

	// Dynamically resolve React/React Native from wherever the package manager installed them
	extraNodeModules: {
		react: path.dirname(require.resolve("react/package.json", { paths: [projectRoot, workspaceRoot] })),
		"react-native": path.dirname(
			require.resolve("react-native/package.json", { paths: [projectRoot, workspaceRoot] })
		),
	},
};

// SVG transformer for @sd/assets SVGs
config.transformer = {
	...config.transformer,
	babelTransformerPath: require.resolve("react-native-svg-transformer"),
	getTransformOptions: async () => ({
		transform: {
			experimentalImportSupport: false,
			inlineRequires: true,
		},
	}),
};

// Add NativeWind support
module.exports = withNativeWind(config, { input: "./src/global.css" });

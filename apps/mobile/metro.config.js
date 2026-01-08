const { getDefaultConfig } = require("expo/metro-config");
const { withNativeWind } = require("nativewind/metro");
const path = require("path");

const projectRoot = import.meta.dirname;
const workspaceRoot = path.resolve(projectRoot, "../..");

const config = getDefaultConfig(projectRoot);

// Watch entire monorepo for hot reload
config.watchFolders = [workspaceRoot];

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

  // Exclude build outputs and prevent loading wrong React version from root
  blockList: [
    /\/apps\/mobile\/ios\/build\/.*/,
    /\/apps\/mobile\/android\/build\/.*/,
    // Block React from workspace root to force local version
    new RegExp(`^${workspaceRoot}/node_modules/react/.*`),
  ],

  // Force React resolution from mobile app's node_modules
  extraNodeModules: {
    react: path.resolve(projectRoot, "node_modules/react"),
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

/*
 * If you add an asset you need to run `npx expo prebuild`
 * If you rename or delete an asset you need to run `npx expo prebuild --clean` to delete them in your android and ios folder as well.
 */

const { withDangerousMod, withXcodeProject, IOSConfig } = require('@expo/config-plugins');
const fs = require('fs');
const path = require('path');

// Specify the source directory of your assets
const ASSET_SOURCE_DIR = 'assets/rive';

const IOS_GROUP_NAME = 'Rivassets';

const withRiveAssets = (config) => {
	config = addAndroidResources(config);
	config = addIOSResources(config);
	return config;
};

// Code inspired by https://github.com/rive-app/rive-react-native/issues/185#issuecomment-1593396573
function addAndroidResources(config) {
	return withDangerousMod(config, [
		'android',
		async (config) => {
			// Get the path to the Android project directory
			const projectRoot = config.modRequest.projectRoot;

			// Get the path to the Android resources directory
			const resDir = path.join(projectRoot, 'android', 'app', 'src', 'main', 'res');

			// Create the 'raw' directory if it doesn't exist
			const rawDir = path.join(resDir, 'raw');
			fs.mkdirSync(rawDir, { recursive: true });

			// Get the path to the assets directory
			const assetSourcePath = path.join(projectRoot, ASSET_SOURCE_DIR);

			// Retrieve all files in the assets directory
			const assetFiles = fs.readdirSync(assetSourcePath);

			// Move each asset file to the resources 'raw' directory
			for (const assetFile of assetFiles) {
				const srcAssetPath = path.join(assetSourcePath, assetFile);
				const destAssetPath = path.join(rawDir, assetFile);
				fs.copyFileSync(srcAssetPath, destAssetPath);
			}

			return config;
		}
	]);
}

// Code inspired by https://github.com/expo/expo/blob/61f8cf8d4b3cf5f8bf61f346476ebdb4aff40545/packages/expo-font/plugin/src/withFontsIos.ts
function addIOSResources(config) {
	return withXcodeProject(config, async (config) => {
		const project = config.modResults;
		const platformProjectRoot = config.modRequest.platformProjectRoot;

		// Create Assets group in project
		IOSConfig.XcodeUtils.ensureGroupRecursively(project, IOS_GROUP_NAME);

		// Get riv filepaths
		const projectRoot = config.modRequest.projectRoot;
		const assetSourcePath = path.join(projectRoot, ASSET_SOURCE_DIR);
		const assetFiles = fs.readdirSync(assetSourcePath);
		const assetFilesPaths = assetFiles.map((assetFile) => `${assetSourcePath}/${assetFile}`);

		// Add assets to group
		addIOSResourceFile(project, platformProjectRoot, assetFilesPaths);

		return config;
	});

	function addIOSResourceFile(project, platformRoot, assetFilesPaths) {
		for (const riveFile of assetFilesPaths) {
			const riveFilePath = path.relative(platformRoot, riveFile);
			IOSConfig.XcodeUtils.addResourceFileToGroup({
				filepath: riveFilePath,
				groupName: IOS_GROUP_NAME,
				project,
				isBuildFile: true,
				verbose: true
			});
		}
	}
}

module.exports = withRiveAssets;

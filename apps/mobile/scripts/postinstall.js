// @ts-check
let fs = require('fs-extra');
let path = require('path');

async function copyReactNativeCodegen() {
	const sourcePath = path.join(__dirname, '../../../node_modules/react-native-codegen');
	const destPath = path.join(__dirname, '../node_modules/react-native-codegen');

	await fs.remove(destPath).catch(() => {});
	await fs.move(sourcePath, destPath);
}

copyReactNativeCodegen();

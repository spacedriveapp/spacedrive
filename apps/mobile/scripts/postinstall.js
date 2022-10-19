// @ts-check
let fs = require('fs-extra');
let path = require('path');

async function copyReactNativeCodegen() {
	const paths = [
		['../../../node_modules/react-native-codegen', '../node_modules/react-native-codegen'],
		['../../../node_modules/jsc-android', '../node_modules/jsc-android']
	];

	for (const pathTuple of paths) {
		const [src, dest] = [path.join(__dirname, pathTuple[0]), path.join(__dirname, pathTuple[1])];
		await fs.remove(dest).catch(() => {});
		await fs.move(src, dest).catch(() => {});
	}
}

copyReactNativeCodegen();

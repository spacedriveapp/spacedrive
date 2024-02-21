const { withAndroidManifest } = require('@expo/config-plugins');

// NOTE: Can be extended if needed (https://forums.expo.dev/t/how-to-edit-android-manifest-was-build/65663/4)
function modifyAndroidManifest(androidManifest) {
	const { manifest } = androidManifest;

	const intent = manifest['queries'][0]['intent'][0];

	if (intent) {
		// Adds <data android:mimeType="*/*" /> to the intents
		intent['data'].push({
			$: {
				'android:mimeType': '*/*'
			}
		});
	}

	return androidManifest;
}

module.exports = function withAndroidIntent(config) {
	return withAndroidManifest(config, (config) => {
		config.modResults = modifyAndroidManifest(config.modResults);
		return config;
	});
};

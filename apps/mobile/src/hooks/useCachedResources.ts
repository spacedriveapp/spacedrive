import { FontAwesome } from '@expo/vector-icons';
import * as Font from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';
import { useEffect, useState } from 'react';
import { Platform } from 'react-native';

/* 
	https://github.com/facebook/hermes/issues/23
	
	We are using "Hermes" on Android & IOS, which for the current version (0.11),
	IOS does not support the Intl fully so we need pollyfill it.

	NOTE: We can be picky about what we "pollyfill" to optimize but for now this works.
*/

if (Platform.OS === 'ios') {
	require('intl'); // import intl object
	require('intl/locale-data/jsonp/en');
}

export default function useCachedResources() {
	const [isLoadingComplete, setLoadingComplete] = useState(false);

	// Load any resources or data that we need prior to rendering the app
	useEffect(() => {
		async function loadResourcesAndDataAsync() {
			try {
				SplashScreen.preventAutoHideAsync();

				// Load fonts, icons etc.
				await Font.loadAsync({
					...FontAwesome.font
				});
			} catch (e) {
				// We might want to provide this error information to an error reporting service
				console.warn(e);
			} finally {
				setLoadingComplete(true);
				SplashScreen.hideAsync();
			}
		}

		loadResourcesAndDataAsync();
	}, []);

	return isLoadingComplete;
}

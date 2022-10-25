import * as SplashScreen from 'expo-splash-screen';
import { useEffect, useState } from 'react';
import { Platform } from 'react-native';
import { syncWithClient, useLibraryStore } from '~/stores/libraryStore';

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

SplashScreen.preventAutoHideAsync();

// Loads any resources or data that we need prior to rendering the app
// Like library store, onboarding, etc.
export default function useLoadApp() {
	const [isLoadingComplete, setLoadingComplete] = useState(false);

	const { currentLibrary, isLoaded: isLibraryLoaded } = useLibraryStore();

	useEffect(() => {
		async function loadResourcesAndDataAsync() {
			try {
				if (!isLibraryLoaded) return;
				currentLibrary && syncWithClient(currentLibrary.uuid);

				if (isLoadingComplete) return;
				SplashScreen.hideAsync();
				setLoadingComplete(true);
			} catch (e) {
				console.warn(e);
			}
		}

		loadResourcesAndDataAsync();
	}, [currentLibrary, isLibraryLoaded, isLoadingComplete]);

	return { isLoadingComplete };
}

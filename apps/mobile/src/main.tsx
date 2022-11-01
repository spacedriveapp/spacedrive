import AsyncStorage from '@react-native-async-storage/async-storage';
import * as SplashScreen from 'expo-splash-screen';
import { lazy, useEffect } from 'react';
import { Suspense } from 'react';
import { Platform } from 'react-native';

const _localStorage = new Map<string, string>();

// We patch stuff onto `globalThis` so that `@sd/client` can use it. This is super hacky but as far as I can tell, there's no better way to do this.
globalThis.isDev = true;

// Custom polyfill for browser `localStorage`
globalThis.localStorage = {
	setItem: (key, value) => {
		_localStorage.set(key, value);

		// This is promise and we intentionally don't await it.
		// This localStorage patch is eventually consistent and for our use case that is worth it for the DX improvements.
		AsyncStorage.setItem(key, value);
	},
	getItem: (key) => _localStorage.get(key) ?? null,
	removeItem: (key) => {
		_localStorage.delete(key);

		// This is promise and we intentionally don't await it.
		// This localStorage patch is eventually consistent and for our use case that is worth it for the DX improvements.
		AsyncStorage.removeItem(key);
	},
	key: (index) => Array.from(_localStorage.keys())[index] ?? null,
	clear: () => {
		_localStorage.clear();

		// This is promise and we intentionally don't await it.
		// This localStorage patch is eventually consistent and for our use case that is worth it for the DX improvements.
		AsyncStorage.clear();
	},
	length: _localStorage.size
};

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

// Enable the splash screen
SplashScreen.preventAutoHideAsync();

// This is insane. We load all data from `AsyncStorage` into the `_localStorage` global and then once complete we import the app.
// This way the polyfilled `localStorage` implementation has its data populated before the global stores within `@sd/client` are initialised (as they are initialised on import).
const App = lazy(async () => {
	const keys = await AsyncStorage.getAllKeys();
	const values = await AsyncStorage.multiGet(keys);
	values.forEach(([key, value]) => _localStorage.set(key, value));

	return await import('./App');
});

export function AppWrapper() {
	return (
		<Suspense>
			<App />
			<ShowSplashScreen />
		</Suspense>
	);
}

function ShowSplashScreen() {
	useEffect(() => {
		SplashScreen.hideAsync();
	}, []);

	return null;
}

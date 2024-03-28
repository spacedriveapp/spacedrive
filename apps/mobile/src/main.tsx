import AsyncStorage from '@react-native-async-storage/async-storage';

import 'event-target-polyfill';

import * as SplashScreen from 'expo-splash-screen';
import { lazy, Suspense } from 'react';
import { Dimensions } from 'react-native';

import { reactNativeLink } from '../modules/sd-core/src';

// Enable the splash screen
SplashScreen.preventAutoHideAsync();

// The worlds worse pollyfill for "CustomEvent". I tried "custom-event-pollyfill" from npm but it uses `document` :(
if (typeof globalThis.CustomEvent !== 'function') {
	// @ts-expect-error
	globalThis.CustomEvent = (event, params) => {
		const evt = new Event(event, params);
		// @ts-expect-error
		evt.detail = params.detail;
		return evt;
	};
}

globalThis.confirm = () => {
	throw new Error("TODO: Implement 'confirm' for mobile");
};

const _localStorage = new Map<string, string>();

// We patch stuff onto `globalThis` so that `@sd/client` can use it. This is super hacky but as far as I can tell, there's no better way to do this.
// TODO: Add env value to automatically set this to `true` in development and false in production builds.
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

globalThis.rspcLinks = [
	// TODO
	// loggerLink({
	// 	enabled: () => getDebugState().rspcLogger
	// }),
	reactNativeLink()
];

// Polyfill for Plausible to work properly (@sd/client/hooks/usePlausible)

window.location = {
	// @ts-ignore
	ancestorOrigins: {},
	href: 'https://spacedrive.com',
	origin: 'https://spacedrive.com',
	protocol: 'https:',
	host: 'spacedrive.com',
	hostname: 'spacedrive.com',
	port: '',
	pathname: '/',
	search: '',
	hash: ''
};
// @ts-ignore
window.document = {};

const { width, height } = Dimensions.get('window');

//@ts-ignore
window.screen = {
	width,
	height
};

// This is insane. We load all data from `AsyncStorage` into the `_localStorage` global and then once complete we import the app.
// This way the polyfilled `localStorage` implementation has its data populated before the global stores within `@sd/client` are initialized (as they are initialized on import).
const App = lazy(async () => {
	const keys = await AsyncStorage.getAllKeys();
	const values = await AsyncStorage.multiGet(keys);
	values.forEach(([key, value]) => _localStorage.set(key, value!));

	return await import('./App');
});

export function AppWrapper() {
	return (
		<Suspense>
			<App />
		</Suspense>
	);
}

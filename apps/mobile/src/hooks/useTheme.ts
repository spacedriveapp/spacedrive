import { useEffect, useReducer } from 'react';
import { Appearance, NativeEventSubscription } from 'react-native';
import { useDeviceContext } from 'twrnc';
import { themeStore, useSubscribeToThemeStore } from '@sd/client';
import { changeTwTheme, tw } from '~/lib/tailwind';

export function useTheme() {
	// Enables screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { initialColorScheme: 'light', observeDeviceColorSchemeChanges: false });

	const [_, forceUpdate] = useReducer((x) => x + 1, 0);

	useSubscribeToThemeStore(() => {
		changeTwTheme(themeStore.theme);
		forceUpdate();
	});

	useEffect(() => {
		let systemThemeListener: NativeEventSubscription | undefined;
		if (themeStore.syncThemeWithSystem === true) {
			systemThemeListener = Appearance.addChangeListener(({ colorScheme }) => {
				changeTwTheme(colorScheme === 'dark' ? 'dark' : 'vanilla');
				forceUpdate();
			});
		}

		return () => {
			systemThemeListener?.remove();
		};
	}, []);
}

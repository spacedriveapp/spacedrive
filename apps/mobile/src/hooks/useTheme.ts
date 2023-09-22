import { useEffect, useReducer } from 'react';
import { Appearance, NativeEventSubscription } from 'react-native';
import { useDeviceContext } from 'twrnc';
import { subscribe } from 'valtio';
import { getThemeStore } from '@sd/client';
import { changeTwTheme, tw } from '~/lib/tailwind';

export function useTheme() {
	// Enables screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { withDeviceColorScheme: false });

	const [_, forceUpdate] = useReducer((x) => x + 1, 0);

	useEffect(() => {
		const unsubscribe = subscribe(getThemeStore(), () => {
			changeTwTheme(getThemeStore().theme);
			forceUpdate();
		});

		return () => {
			unsubscribe();
		};
	}, []);

	useEffect(() => {
		let systemThemeListener: NativeEventSubscription | undefined;
		if (getThemeStore().syncThemeWithSystem === true) {
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

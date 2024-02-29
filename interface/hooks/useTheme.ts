import { useEffect } from 'react';
import { useThemeStore } from '@sd/client';

import { usePlatform } from '..';

export function useTheme() {
	const themeStore = useThemeStore();
	const { lockAppTheme } = usePlatform();
	const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');

	useEffect(() => {
		const handleThemeChange = () => {
			if (themeStore.syncThemeWithSystem) {
				lockAppTheme?.('Auto');
				if (systemTheme.matches) {
					document.documentElement.classList.remove('vanilla-theme');
					document.documentElement.style.setProperty(
						'--dark-hue',
						themeStore.hueValue.toString()
					);
					themeStore.theme = 'dark';
				} else {
					document.documentElement.classList.add('vanilla-theme');
					document.documentElement.style.setProperty(
						'--light-hue',
						themeStore.hueValue.toString()
					);
					themeStore.theme = 'vanilla';
				}
			} else {
				if (themeStore.theme === 'dark') {
					document.documentElement.classList.remove('vanilla-theme');
					document.documentElement.style.setProperty(
						'--dark-hue',
						themeStore.hueValue.toString()
					);
					lockAppTheme?.('Dark');
				} else if (themeStore.theme === 'vanilla') {
					document.documentElement.classList.add('vanilla-theme');
					document.documentElement.style.setProperty(
						'--light-hue',
						themeStore.hueValue.toString()
					);
					lockAppTheme?.('Light');
				}
			}
		};

		handleThemeChange();

		systemTheme.addEventListener('change', handleThemeChange);

		return () => {
			systemTheme.removeEventListener('change', handleThemeChange);
		};
	}, [themeStore, lockAppTheme, systemTheme]);
}

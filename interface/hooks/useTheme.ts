import { useThemeStore, getThemeStore } from '@sd/client';
import { useEffect } from 'react';

export function useTheme() {
  const themeStore = useThemeStore();
  const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');

  useEffect(() => {
    const handleThemeChange = () => {
      if (themeStore.syncThemeWithSystem) {
        if (systemTheme.matches) {
          document.documentElement.classList.remove('vanilla-theme');
		  document.documentElement.style.setProperty('--dark-hue', getThemeStore().hueValue.toString());
		  getThemeStore().theme = 'dark';
        } else {
          document.documentElement.classList.add('vanilla-theme');
		  document.documentElement.style.setProperty('--light-hue', getThemeStore().hueValue.toString());
		  getThemeStore().theme = 'vanilla';
        }
      } else {
        if (themeStore.theme === 'dark') {
          document.documentElement.classList.remove('vanilla-theme');
		  document.documentElement.style.setProperty('--dark-hue', getThemeStore().hueValue.toString());
        } else if (themeStore.theme === 'vanilla') {
          document.documentElement.classList.add('vanilla-theme');
		  document.documentElement.style.setProperty('--light-hue', getThemeStore().hueValue.toString());

        }
      }
    };

    handleThemeChange();

    systemTheme.addEventListener('change', handleThemeChange);

    return () => {
      systemTheme.removeEventListener('change', handleThemeChange);
    };
  }, [themeStore, systemTheme]);
}

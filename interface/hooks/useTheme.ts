import { useThemeStore } from '@sd/client';
import { useEffect } from 'react';

export function useTheme() {
  const themeStore = useThemeStore();
  const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');

  useEffect(() => {
    const handleThemeChange = () => {
      if (themeStore.syncThemeWithSystem) {
        if (systemTheme.matches) {
          document.documentElement.classList.remove('vanilla-theme');
        } else {
          document.documentElement.classList.add('vanilla-theme');
        }
      } else {
        if (themeStore.theme === 'dark') {
          document.documentElement.classList.remove('vanilla-theme');
        } else if (themeStore.theme === 'vanilla') {
          document.documentElement.classList.add('vanilla-theme');
        }
      }
    };

    handleThemeChange();

    systemTheme.addEventListener('change', handleThemeChange);

    return () => {
      systemTheme.removeEventListener('change', handleThemeChange);
    };
  }, [themeStore.syncThemeWithSystem, themeStore.theme, systemTheme]);
}

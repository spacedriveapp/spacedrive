import { useEffect } from 'react';
import { useNavigate } from 'react-router';

import { KeybindEvent } from '../util/keybind';
import { getWindowState } from './useWindowState';

export const useKeybindEventHandler = (libraryId?: string) => {
	const navigate = useNavigate();
	const windowState = getWindowState();

	useEffect(() => {
		const handler = (e: KeybindEvent) => {
			e.preventDefault();

			switch (e.detail.action) {
				case 'open_settings':
					libraryId && navigate(`/${libraryId}/settings/client/general`);
					break;
				case 'open_overview':
					libraryId && navigate(`/${libraryId}/overview`);
					break;
				case 'window_maximized':
					windowState.isMaximized = true;
					break;
				case 'window_not_maximized':
					windowState.isMaximized = false;
					break;
			}
		};

		document.addEventListener('keybindexec', handler);
		return () => document.removeEventListener('keybindexec', handler);
	}, [navigate, libraryId, windowState]);
};

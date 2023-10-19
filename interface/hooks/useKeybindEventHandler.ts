import { useEffect } from 'react';
import { useNavigate } from 'react-router';

import { KeybindEvent } from '../util/keybind';

export const useKeybindEventHandler = (props: {
	libraryId?: string;
	setIsWindowMaximized: (maximized: boolean) => void;
}) => {
	const navigate = useNavigate();
	const { libraryId, setIsWindowMaximized } = props;

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
					setIsWindowMaximized(true);
					break;
				case 'window_not_maximized':
					setIsWindowMaximized(false);
					break;
			}
		};

		document.addEventListener('keybindexec', handler);
		return () => document.removeEventListener('keybindexec', handler);
	}, [navigate, libraryId, setIsWindowMaximized]);
};

import { useEffect } from 'react';
import { useNavigate } from 'react-router';

import { KeybindEvent } from '../util/keybind';

export const useKeybindEventHandler = (libraryId?: string) => {
	const navigate = useNavigate();

	useEffect(() => {
		const handler = (e: KeybindEvent) => {
			if (e.detail.action === 'open_settings') {
				libraryId && navigate(`/${libraryId}/settings/client/general`);
				e.preventDefault();
				return;
			} else if (e.detail.action === 'open_overview') {
				libraryId && navigate(`/${libraryId}/overview`);
				e.preventDefault();
				return;
			}
		};

		document.addEventListener('keybindexec', handler);
		return () => document.removeEventListener('keybindexec', handler);
	}, [navigate, libraryId]);
};

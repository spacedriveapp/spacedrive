import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { KeybindEvent } from '../util/keybind';

export function useKeybindHandler() {
	const navigate = useNavigate();

	useEffect(() => {
		const handler = (e: KeybindEvent) => {
			if (e.detail.action === 'open_settings') {
				navigate('/settings');
				e.preventDefault();
				return;
			}
		};

		document.addEventListener('keybindexec', handler);
		return () => document.removeEventListener('keybindexec', handler);
	}, [navigate]);
}

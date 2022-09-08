import { useEffect } from 'react';
import { useNavigate } from 'react-router';

export function useKeyboardHandler() {
	const navigate = useNavigate();

	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if (e.metaKey && e.key === ',') {
				navigate('/settings');
				e.preventDefault();
				return;
			}
		};

		document.addEventListener('keydown', handler);
		return () => document.removeEventListener('keydown', handler);
	}, [navigate]);
}

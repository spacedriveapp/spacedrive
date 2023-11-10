import { MouseEvent } from 'react';
import { useNavigate } from 'react-router';
import { useOperatingSystem } from '~/hooks';

import { useSearchStore } from '../app/$libraryId/Explorer/Search/store';

export const useMouseNavigate = () => {
	const idx = history.state.idx as number;
	const navigate = useNavigate();

	const { isSearching } = useSearchStore();
	const os = useOperatingSystem();
	const realOs = useOperatingSystem(true);

	const handler = (e: MouseEvent) => {
		if (os === 'browser' || realOs !== 'macOS') return;
		if (e.buttons === 8) {
			if (idx === 0 || isSearching) return;
			navigate(-1);
		} else if (e.buttons === 16) {
			if (idx === history.length - 1 || isSearching) return;

			navigate(1);
		}
	};

	return handler;
};

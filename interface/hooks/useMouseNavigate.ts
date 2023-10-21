import { MouseEvent } from 'react';
import { useNavigate } from 'react-router';

import { useSearchStore } from '../app/$libraryId/Explorer/View/SearchOptions/store';

export const useMouseNavigate = () => {
	const idx = history.state.idx as number;
	const navigate = useNavigate();
	const { isSearching } = useSearchStore();

	const handler = (e: MouseEvent) => {
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

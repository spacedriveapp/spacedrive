import { RefObject, useMemo } from 'react';
import useResizeObserver from 'use-resize-observer';

export const useIsTextTruncated = (element: RefObject<HTMLElement>, text: string | null) => {
	const { width } = useResizeObserver({ ref: element });

	return useMemo(() => {
		if (!element.current) return false;
		return element.current.scrollWidth > element.current.clientWidth;
	}, [element, width, text]);
};

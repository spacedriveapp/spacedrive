import { RefObject, useState } from 'react';
import { useMutationObserver } from 'rooks';
import useResizeObserver from 'use-resize-observer';

export const useIsTextTruncated = (element: RefObject<HTMLElement>) => {
	const [truncated, setTruncated] = useState(false);

	const handleIsTruncated = () => {
		if (!element.current) return;
		setTruncated(element.current.scrollWidth > element.current.clientWidth);
	};

	useMutationObserver(element, handleIsTruncated, {
		attributes: true
	});

	useResizeObserver({
		ref: element,
		onResize: handleIsTruncated
	});

	return truncated;
};

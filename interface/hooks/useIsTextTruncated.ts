import { RefObject, useCallback, useEffect, useState } from 'react';
import { useCallbackToWatchResize } from './useCallbackToWatchResize';

export const useIsTextTruncated = (
	element: RefObject<HTMLElement>,
	text: string | null
): boolean => {
	const determineIsTruncated = useCallback((): boolean => {
		if (!element.current) return false;
		return element.current.scrollWidth > element.current.clientWidth;
	}, [element]);

	const [isTruncated, setIsTruncated] = useState<boolean>(determineIsTruncated());

	useCallbackToWatchResize(
		() => setIsTruncated(determineIsTruncated()),
		[determineIsTruncated],
		element
	);

	useEffect(() => {
		setIsTruncated(determineIsTruncated());
	}, [element, determineIsTruncated, text]);

	return isTruncated;
};

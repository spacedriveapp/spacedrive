import { useEffect, useState } from 'react';

/*
 A simple hook that returns the window size
 */

type hookReturn = {
	width: number | null;
	height: number | null;
};

export const useWindowSize = (): hookReturn => {
	const [windowSize, setWindowSize] = useState<hookReturn>({
		width: null,
		height: null
	});

	useEffect(() => {
		if (typeof window === 'undefined') {
			return;
		}
		const handleResize = () => {
			setWindowSize({
				width: window.innerWidth,
				height: window.innerHeight
			});
		};

		window.addEventListener('load', handleResize);
		window.addEventListener('resize', handleResize);

		return () => {
			window.removeEventListener('resize', handleResize);
			window.removeEventListener('load', handleResize);
		};
	}, [windowSize.width, windowSize.height]);

	return windowSize;
};

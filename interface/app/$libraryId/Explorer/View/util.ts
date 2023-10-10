import { useCallback } from 'react';

import { ExplorerViewPadding } from '.';

export const useExplorerViewPadding = (padding?: number | ExplorerViewPadding) => {
	const getPadding = useCallback(
		(key: keyof ExplorerViewPadding) => (typeof padding === 'object' ? padding[key] : padding),
		[padding]
	);

	return {
		top: getPadding('top') ?? getPadding('y'),
		bottom: getPadding('bottom') ?? getPadding('y'),
		left: getPadding('left') ?? getPadding('x'),
		right: getPadding('right') ?? getPadding('x')
	};
};

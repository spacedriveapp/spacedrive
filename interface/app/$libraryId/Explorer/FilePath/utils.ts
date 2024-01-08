import { useMemo, useState, type CSSProperties, type RefObject } from 'react';
import { useCallbackToWatchResize } from '~/hooks';

export function useSize(ref: RefObject<Element>) {
	const [size, setSize] = useState({ width: 0, height: 0 });

	useCallbackToWatchResize(({ width, height }) => setSize({ width, height }), [], ref);

	return size;
}

export function useBlackBars(videoSize: { width: number; height: number }, blackBarsSize?: number) {
	return useMemo(() => {
		const { width, height } = videoSize;

		const orientation = height > width ? 'vertical' : 'horizontal';

		const barSize =
			blackBarsSize ||
			Math.floor(Math.ceil(orientation === 'vertical' ? height : width) / 10);

		const xBarSize = orientation === 'vertical' ? barSize : 0;
		const yBarSize = orientation === 'horizontal' ? barSize : 0;

		return {
			size: {
				x: xBarSize,
				y: yBarSize
			},
			style: {
				borderLeftWidth: xBarSize,
				borderRightWidth: xBarSize,
				borderTopWidth: yBarSize,
				borderBottomWidth: yBarSize,
				borderColor: 'black',
				borderRadius: 4
			} satisfies CSSProperties
		};
	}, [videoSize, blackBarsSize]);
}

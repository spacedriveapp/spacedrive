import { useMemo, useRef, useState, type CSSProperties, type RefObject } from 'react';
import { useCallbackToWatchResize } from '~/hooks';

export function useSize(ref: RefObject<Element>) {
	const [size, setSize] = useState({ width: 0, height: 0 });

	useCallbackToWatchResize(({ width, height }) => setSize({ width, height }), [], ref);

	return size;
}

export function useBlackBars(
	node: RefObject<HTMLElement>,
	size: ReturnType<typeof useSize>,
	options: { size?: number; disabled?: boolean } = {}
) {
	const previousNodeSize = useRef<typeof size>();
	const previousParentSize = useRef<typeof size>();
	const previousBarSize = useRef<{ x?: number; y?: number }>();

	return useMemo(() => {
		if (options.disabled) return {};

		const orientation = size.height > size.width ? 'vertical' : 'horizontal';

		const getBarSize = () => {
			return Math.floor(
				Math.ceil(orientation === 'vertical' ? size.height : size.width) / 10
			);
		};

		let barSize = options.size;

		if (barSize === undefined) {
			let parentSize = { width: 0, height: 0 };

			const parent = node.current?.parentElement;

			if (parent) {
				const style = getComputedStyle(parent);

				const paddingX = parseFloat(style.paddingLeft) + parseFloat(style.paddingRight);
				const paddingY = parseFloat(style.paddingTop) + parseFloat(style.paddingBottom);

				parentSize = {
					width: parent.clientWidth - paddingX,
					height: parent.clientHeight - paddingY
				};
			}

			if (
				parentSize.width !== previousParentSize.current?.width ||
				parentSize.height !== previousParentSize.current?.height
			) {
				barSize = getBarSize();
			} else if (previousNodeSize.current && previousBarSize.current) {
				const previousNodeWidth =
					previousNodeSize.current.width + (previousBarSize.current.x ?? 0) * 2;

				const previousNodeHeight =
					previousNodeSize.current.height + (previousBarSize.current.y ?? 0) * 2;

				const nodeWidth =
					previousNodeSize.current.width -
					Math.max(0, previousNodeWidth - parentSize.width);

				const nodeHeight =
					previousNodeSize.current.height -
					Math.max(0, previousNodeHeight - parentSize.height);

				if (
					(orientation === 'vertical' && nodeWidth === size.width) ||
					(orientation === 'horizontal' && nodeHeight === size.height)
				) {
					barSize = previousBarSize.current.x ?? previousBarSize.current.y;
				} else {
					barSize = getBarSize();
				}
			} else {
				barSize = getBarSize();
			}

			previousParentSize.current = parentSize;
		}

		const xBarSize = orientation === 'vertical' ? barSize : undefined;
		const yBarSize = orientation === 'horizontal' ? barSize : undefined;

		previousNodeSize.current = { width: size.width, height: size.height };
		previousBarSize.current = { x: xBarSize, y: yBarSize };

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
	}, [options.disabled, options.size, size.height, size.width, node]);
}

import { useCallback, useEffect, useRef } from 'react';

import { useExplorerContext } from '../Context';
import { explorerStore } from '../store';

/**
 * Custom explorer dnd scroll handler as the default auto-scroll from dnd-kit is presenting issues
 */
export const useDragScrollable = ({ direction }: { direction: 'up' | 'down' }) => {
	const explorer = useExplorerContext();

	const node = useRef<HTMLElement | null>(null);

	const timeout = useRef<number | null>(null);
	const interval = useRef<number | null>(null);

	useEffect(() => {
		const element = node;
		const scrollElement = explorer.scrollRef.current;
		if (!element || !scrollElement) return;

		const reset = () => {
			if (timeout.current) {
				clearTimeout(timeout.current);
				timeout.current = null;
			}

			if (interval.current) {
				clearInterval(interval.current);
				interval.current = null;
			}
		};

		const handleMouseMove = ({ clientX, clientY }: MouseEvent) => {
			if (explorerStore.drag?.type !== 'dragging') return reset();

			const node = element.current;
			if (!node) return reset();

			const rect = node.getBoundingClientRect();

			const isInside =
				clientX >= rect.left &&
				clientX <= rect.right &&
				clientY >= rect.top &&
				clientY <= rect.bottom;

			if (!isInside) return reset();

			if (timeout.current) return;

			timeout.current = setTimeout(() => {
				interval.current = setInterval(() => {
					scrollElement.scrollBy({ top: direction === 'up' ? -10 : 10 });
				}, 5);
			}, 1000);
		};

		window.addEventListener('mousemove', handleMouseMove);
		return () => window.removeEventListener('mouseover', handleMouseMove);
	}, [direction, explorer.scrollRef]);

	const ref = useCallback((nodeElement: HTMLElement | null) => (node.current = nodeElement), []);

	return { ref };
};

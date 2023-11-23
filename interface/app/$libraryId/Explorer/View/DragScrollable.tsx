import { useCallback, useEffect, useRef, useState } from 'react';
import { useExplorerLayoutStore } from '@sd/client';
import { tw } from '@sd/ui';

import { useTopBarContext } from '../../TopBar/Layout';
import { useExplorerContext } from '../Context';
import { PATH_BAR_HEIGHT } from '../ExplorerPath';
import { getExplorerStore } from '../store';

const Trigger = tw.div`absolute inset-x-0 h-10 pointer-events-none`;

export const DragScrollable = () => {
	const topBar = useTopBarContext();
	const explorer = useExplorerContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	const layoutStore = useExplorerLayoutStore();
	const showPathBar = explorer.showPathBar && layoutStore.showPathBar;

	const { ref: dragScrollableUpRef } = useDragScrollable({ direction: 'up' });
	const { ref: dragScrollableDownRef } = useDragScrollable({ direction: 'down' });

	return (
		<>
			{explorerSettings.layoutMode !== 'list' && (
				<Trigger ref={dragScrollableUpRef} style={{ top: topBar.topBarHeight }} />
			)}
			<Trigger
				ref={dragScrollableDownRef}
				style={{ bottom: showPathBar ? PATH_BAR_HEIGHT : 0 }}
			/>
		</>
	);
};

// Custom explorer dnd scroll handler as the default auto-scroll from dnd-kit is presenting issues
export const useDragScrollable = ({ direction }: { direction: 'up' | 'down' }) => {
	const explorer = useExplorerContext();

	const [node, setNode] = useState<HTMLElement | null>(null);

	const timeout = useRef<NodeJS.Timeout | null>(null);
	const interval = useRef<NodeJS.Timer | null>(null);

	useEffect(() => {
		const element = node;
		const scroll = explorer.scrollRef.current;
		if (!element || !scroll) return;

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
			if (getExplorerStore().drag?.type !== 'dragging') return reset();

			const rect = element.getBoundingClientRect();

			const isInside =
				clientX >= rect.left &&
				clientX <= rect.right &&
				clientY >= rect.top &&
				clientY <= rect.bottom;

			if (!isInside) return reset();

			if (timeout.current) return;

			timeout.current = setTimeout(() => {
				interval.current = setInterval(() => {
					scroll.scrollBy({ top: direction === 'up' ? -10 : 10 });
				});
			}, 1000);
		};

		window.addEventListener('mousemove', handleMouseMove);
		return () => window.removeEventListener('mouseover', handleMouseMove);
	}, [direction, explorer.scrollRef, node]);

	const ref = useCallback((nodeElement: HTMLElement | null) => setNode(nodeElement), []);

	return { ref };
};

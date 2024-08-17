import { DragOverlay as DragOverlayPrimitive, type ClientRect, type Modifier } from '@dnd-kit/core';
import { getEventCoordinates } from '@dnd-kit/utilities';
import clsx from 'clsx';
import { memo, useEffect, useRef } from 'react';
import { ExplorerItem, useSelector } from '@sd/client';
import { useIsDark } from '~/hooks';

import { FileThumb } from './FilePath/Thumb';
import { explorerStore } from './store';
import { RenamableItemText } from './View/RenamableItemText';

const useSnapToCursorModifier = () => {
	const drag = useSelector(explorerStore, (s) => s.drag);

	const initialRect = useRef<ClientRect | null>(null);

	const modifier: Modifier = ({ activatorEvent, activeNodeRect, transform }) => {
		if (!activeNodeRect || !activatorEvent) return transform;

		const activatorCoordinates = getEventCoordinates(activatorEvent);
		if (!activatorCoordinates) return transform;

		const rect = initialRect.current ?? activeNodeRect;

		if (!initialRect.current) initialRect.current = activeNodeRect;

		// Default offset so during drag the cursor doesn't overlap the overlay
		// which can cause issues with mouse events on other elements
		const offset = 12;

		const offsetX = activatorCoordinates.x - rect.left;
		const offsetY = activatorCoordinates.y - rect.top;

		return {
			...transform,
			x: transform.x + offsetX + offset,
			y: transform.y + offsetY + offset
		};
	};

	useEffect(() => {
		if (!drag) initialRect.current = null;
	}, [drag]);

	return modifier;
};

export const DragOverlay = memo(() => {
	const isDark = useIsDark();

	const modifier = useSnapToCursorModifier();
	const drag = useSelector(explorerStore, (s) => s.drag);

	return (
		<DragOverlayPrimitive
			dropAnimation={null}
			modifiers={[modifier]}
			className="!h-auto !w-full max-w-md"
		>
			{!drag || drag.type === 'touched' ? null : (
				<div className="space-y-[2px] pl-0.5 pt-0.5 duration-300 animate-in fade-in">
					{drag.items.length > 1 && (
						<div className="absolute right-full top-1.5 mr-2 flex h-6 min-w-[24px] items-center justify-center rounded-full bg-accent px-1 text-sm text-white">
							{drag.items.length}
						</div>
					)}

					{(drag.items.slice(0, 8) as ExplorerItem[]).map((item, i, items) => (
						<div
							key={i}
							className={clsx(
								'flex items-center gap-2',
								drag.items.length > 7 && [
									i + 1 === items.length && 'opacity-10',
									i + 2 === items.length && 'opacity-50',
									i + 3 === items.length && 'opacity-90'
								]
							)}
						>
							<FileThumb
								data={item}
								size={32}
								frame
								frameClassName={clsx(
									'!border-DEFAULT shadow-md',
									isDark ? 'shadow-app-shade/50' : 'shadow-app-shade/25'
								)}
							/>
							<RenamableItemText item={item} highlight={true} />
						</div>
					))}
				</div>
			)}
		</DragOverlayPrimitive>
	);
});

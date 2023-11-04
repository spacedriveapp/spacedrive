import type { Modifier } from '@dnd-kit/core';
import { DragOverlay as DragOverlayPrimitive } from '@dnd-kit/core';
import { getEventCoordinates } from '@dnd-kit/utilities';
import clsx from 'clsx';
import { memo } from 'react';
import { ExplorerItem } from '@sd/client';
import { useIsDark } from '~/hooks';

import { FileThumb } from '../FilePath/Thumb';
import { useExplorerStore } from '../store';
import { RenamableItemText } from './RenamableItemText';

const snapToCursor: Modifier = ({ activatorEvent, draggingNodeRect, transform }) => {
	if (draggingNodeRect && activatorEvent) {
		const activatorCoordinates = getEventCoordinates(activatorEvent);

		if (!activatorCoordinates) {
			return transform;
		}

		const offsetX = activatorCoordinates.x - draggingNodeRect.left;
		const offsetY = activatorCoordinates.y - draggingNodeRect.top;

		return {
			...transform,
			x: transform.x + offsetX,
			y: transform.y + offsetY
		};
	}

	return transform;
};

export const DragOverlay = memo(() => {
	const isDark = useIsDark();

	const { drag } = useExplorerStore();

	return (
		<DragOverlayPrimitive
			dropAnimation={null}
			modifiers={[snapToCursor]}
			className="!h-auto !w-full max-w-md"
		>
			{!drag || drag.type === 'touched' ? null : (
				<div className="space-y-[2px] pl-2.5 pt-2.5 duration-300 animate-in fade-in">
					{drag.items.length > 1 && (
						<div className="absolute -left-5 top-3 flex h-5 w-5 items-center justify-center rounded-full bg-accent text-xs text-white">
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
									'!border-[1px] shadow-md',
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

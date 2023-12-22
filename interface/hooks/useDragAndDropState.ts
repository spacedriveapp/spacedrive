import { RefObject, useEffect, useId, useState } from 'react';
import { proxy, useSnapshot } from 'valtio';

import { usePlatform } from '..';

const dndState = proxy({
	renderRects: false
});

export const toggleRenderRects = () => (dndState.renderRects = !dndState.renderRects);

type UseDroppedOnProps = {
	// A ref to used to detect when the element is being hovered.
	// If the file drop's mouse position is above this ref it will return a 'hovered' state.
	// If none is set the 'hovered' state will never be returned.
	ref?: RefObject<HTMLDivElement>;
	// Handle the final file drop event.
	// If `ref === undefined` this will be called for every drop event.
	// If `ref !== undefined` this will only be called if the drop event is within the bounds of the ref.
	onDrop?: (paths: string[]) => void;
	// Added to the bounds of the shape and if the mouse is within it's counted as hovered.
	// This allows for the dropzone to be bigger than the actual element to make it easier to drop on.
	extendBoundsBy?: number;
};

function isWithinRect(x: number, y: number, rect: DOMRect) {
	return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

function expandRect(rect: DOMRect, by: number) {
	return new DOMRect(rect.left - by, rect.top - by, rect.width + by * 2, rect.height + by * 2);
}

export function useDragAndDrop(opts?: UseDroppedOnProps) {
	const id = useId();
	const platform = usePlatform();
	const [state, setState] = useState('idle' as 'idle' | 'active' | 'hovered');
	const debugRect = useSnapshot(dndState).renderRects;

	useEffect(() => {
		if (!platform.subscribeToDragAndDropEvents) return;

		let elemBounds = opts?.ref?.current?.getBoundingClientRect();
		if (elemBounds && opts?.extendBoundsBy)
			elemBounds = expandRect(elemBounds, opts.extendBoundsBy);

		const existingDebugRectElem = document.getElementById(id);
		if (existingDebugRectElem) existingDebugRectElem.remove();

		if (debugRect) {
			const div = document.createElement('div');
			div.id = id;
			div.style.position = 'absolute';
			div.style.left = `${elemBounds?.left}px`;
			div.style.top = `${elemBounds?.top}px`;
			div.style.width = `${elemBounds?.width}px`;
			div.style.height = `${elemBounds?.height}px`;
			div.style.backgroundColor = 'rgba(255, 0, 0, 0.5)';
			div.style.pointerEvents = 'none';
			div.style.zIndex = '999';
			document.body.appendChild(div);
		}

		let finished = false;
		const unsub = platform.subscribeToDragAndDropEvents((event) => {
			if (finished) return;

			if (event.type === 'Hovered') {
				const isHovered = elemBounds ? isWithinRect(event.x, event.y, elemBounds) : false;
				setState(isHovered ? 'hovered' : 'active');
			} else if (event.type === 'Dropped') {
				setState('idle');

				if (elemBounds && !isWithinRect(event.x, event.y, elemBounds)) return;
				if (opts?.onDrop) opts.onDrop(event.paths);
			} else if (event.type === 'Cancelled') {
				setState('idle');
			}
		});

		return () => {
			finished = true;
			void unsub.then((unsub) => unsub());
		};
	}, [platform, opts, debugRect, id]);

	return state;
}

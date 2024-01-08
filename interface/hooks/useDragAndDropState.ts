import { RefObject, useEffect, useId, useLayoutEffect, useState } from 'react';
import { proxy, useSnapshot } from 'valtio';

import { usePlatform } from '..';

const dndState = proxy({
	renderRects: false
});

export const toggleRenderRects = () => (dndState.renderRects = !dndState.renderRects);

type UseDropzoneProps = {
	// A ref to used to detect when the element is being hovered.
	// If the file drop's mouse position is above this ref it will return a 'hovered' state.
	// If none is set the 'hovered' state will never be returned.
	ref?: RefObject<HTMLDivElement>;
	// Handle the final file drop event.
	// If `ref === undefined` this will be called for every drop event.
	// If `ref !== undefined` this will only be called if the drop event is within the bounds of the ref.
	onDrop?: (paths: string[]) => void;
	// Called only once per each hover event.
	onHover?: () => void;
	// On each position of the move
	onMove?: (x: number, y: number) => void;
	// Added to the bounds of the shape and if the mouse is within it's counted as hovered.
	// This allows for the dropzone to be bigger than the actual element to make it easier to drop on.
	extendBoundsBy?: number;
};

export function isWithinRect(x: number, y: number, rect: DOMRect) {
	return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

export function expandRect(rect: DOMRect, by: number) {
	return new DOMRect(rect.left - by, rect.top - by, rect.width + by * 2, rect.height + by * 2);
}

export function useDropzone(opts?: UseDropzoneProps) {
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
				setState((state) => {
					// Only call it during the state transition from 'idle' -> 'active' when no `elemBounds`
					if (opts?.onHover) {
						if (elemBounds) {
							if ((state === 'idle' || state === 'active') && isHovered)
								opts.onHover();
						} else {
							if (state === 'idle') opts.onHover();
						}
					}

					return isHovered ? 'hovered' : 'active';
				});

				if (opts?.onMove) opts.onMove(event.x, event.y);
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

type UseOnDndEnterProps = {
	// Ref to the element that is being dragged over.
	ref: React.RefObject<HTMLDivElement>;
	// Called when the file being actively drag and dropped leaves the bounds of the ref (+ `extendBoundsBy`).
	onLeave: () => void;
	// Added to the bounds of the shape and if the mouse is within it's counted as hovered.
	// This allows for the dropzone to be bigger than the actual element to make it easier to drop on.
	extendBoundsBy?: number;
};

/// is responsible for running an action when the file being actively drag and dropped leaves the bounds of the ref.
export function useOnDndLeave({ ref, onLeave, extendBoundsBy }: UseOnDndEnterProps) {
	const id = useId();
	const platform = usePlatform();
	const debugRect = useSnapshot(dndState).renderRects;

	useLayoutEffect(() => {
		if (!platform.subscribeToDragAndDropEvents) return;

		let finished = false;
		let mouseEnteredZone = false;
		let rect: DOMRect | null = null;

		// This timeout is super important. It ensures we get the ref after it's properly rendered.
		// This is important if we render this component within a portal.
		setTimeout(() => {
			// We do this before the early return so when the element is removed the debug rect is removed.
			const existingDebugRectElem = document.getElementById(id);
			if (existingDebugRectElem) existingDebugRectElem.remove();

			if (!ref.current) return;
			rect = ref.current.getBoundingClientRect();
			if (extendBoundsBy) rect = expandRect(rect, extendBoundsBy);

			if (debugRect) {
				const div = document.createElement('div');
				div.id = id;
				div.style.position = 'absolute';
				div.style.left = `${rect.left}px`;
				div.style.top = `${rect.top}px`;
				div.style.width = `${rect.width}px`;
				div.style.height = `${rect.height}px`;
				div.style.backgroundColor = 'rgba(0, 255, 0, 0.5)';
				div.style.pointerEvents = 'none';
				div.style.zIndex = '999';
				document.body.appendChild(div);
			}
		});

		const unsub = platform.subscribeToDragAndDropEvents((event) => {
			if (finished) return;

			if (event.type === 'Hovered') {
				if (!rect) return;
				const isWithinRectNow = isWithinRect(event.x, event.y, rect);
				if (mouseEnteredZone) {
					if (!isWithinRectNow) onLeave();
				} else {
					mouseEnteredZone = isWithinRectNow;
				}
			} else if (event.type === 'Dropped') {
				mouseEnteredZone = false;
			} else if (event.type === 'Cancelled') {
				mouseEnteredZone = false;
			}
		});

		return () => {
			finished = true;
			void unsub.then((unsub) => unsub());
		};
	}, [platform, ref, onLeave, extendBoundsBy, debugRect, id]);
}

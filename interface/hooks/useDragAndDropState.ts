import { RefObject, useEffect } from 'react';
import { proxy, subscribe, useSnapshot } from 'valtio';

const state = proxy({
	droppedFiles: [] as string[]
});

export const useDragAndDropState = () => useSnapshot(state);

export const getDragAndDropState = () => state;

export const subscribeDragAndDropState = (callback: () => void) => subscribe(state, callback);

export function useDroppedOn(ref: RefObject<HTMLDivElement>) {
	// TODO: Finish this hook

	useEffect(() => {
		if (ref.current) return;

		console.log(ref.current);

		// ref.current.addEventListener(
		// 	'mouseleave',
		// 	function (event) {
		// 		// isMouseHover = false;
		// 		// event.target.textContent = 'mouse out';
		// 		// console.log(isMouseHover);
		// 	},
		// 	false
		// );
		// ref.current.addEventListener(
		// 	'mouseover',
		// 	function (event) {
		// 		// isMouseHover = true;
		// 		// event.target.textContent = 'mouse in';
		// 		// console.log(isMouseHover);
		// 	},
		// 	false
		// );

		// ref.current.

		return () => {
			// TODO: Cleanup
		};
	}, [ref]);

	return ref;
}

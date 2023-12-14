import { proxy, subscribe, useSnapshot } from 'valtio';

const state = proxy({
	droppedFiles: [] as string[]
});

export const useDropAndDropState = () => useSnapshot(state);

export const getDropAndDropState = () => state;

export const subscribeDragAndDropState = (callback: () => void) => subscribe(state, callback);

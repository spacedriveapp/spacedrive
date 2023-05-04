import { proxy, subscribe, useSnapshot } from 'valtio';

const state = proxy({
	droppedFiles: [] as string[]
});

export const useSpacedropState = () => useSnapshot(state);

export const getSpacedropState = () => state;

export const subscribeSpacedropState = (callback: () => void) => subscribe(state, callback);

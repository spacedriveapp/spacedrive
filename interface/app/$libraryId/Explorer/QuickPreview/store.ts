import { proxy, useSnapshot } from 'valtio';

const store = proxy({
	open: false,
	itemIndex: 0
});

export const useQuickPreviewStore = () => useSnapshot(store);
export const getQuickPreviewStore = () => store;

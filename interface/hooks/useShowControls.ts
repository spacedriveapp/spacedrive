import { proxy, useSnapshot } from 'valtio';

const showControlsStore = proxy({ isEnabled: location.search.includes('showControls') });

export const useSearchStore = () => useSnapshot(showControlsStore);

export const getSearchStore = () => showControlsStore;

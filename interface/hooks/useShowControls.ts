import { proxy, useSnapshot } from 'valtio';

const showControlsStore = proxy({ isEnabled: location.search.includes('showControls') });

export const useShowControls = () => useSnapshot(showControlsStore);

export const getShowControls = () => showControlsStore;

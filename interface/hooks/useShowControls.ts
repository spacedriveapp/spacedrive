import { proxy, useSnapshot } from 'valtio';

const showControlsStore = proxy({
	isEnabled: location.search.includes('showControls') || localStorage.getItem('showControls'),
	transparentBg:
		location.search.includes('transparentBg') || localStorage.getItem('transparentBg')
});

export const useShowControls = () => useSnapshot(showControlsStore);

export const getShowControls = () => showControlsStore;

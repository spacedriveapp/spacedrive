import { useSnapshot } from 'valtio';

import { valtioPersist } from '../lib';

const explorerLayoutStore = valtioPersist('sd-explorer-layout', {
	showPathBar: true,
	showImageSlider: true
});

export function useExplorerLayoutStore() {
	return useSnapshot(explorerLayoutStore);
}

export function getExplorerLayoutStore() {
	return explorerLayoutStore;
}

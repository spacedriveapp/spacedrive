import { createMutable } from 'solid-js/store';

import { createPersistedMutable, useSolidStore } from '../solid';

export const explorerLayout = createPersistedMutable(
	'sd-explorer-layout',
	createMutable({
		showPathBar: true,
		showImageSlider: true
	})
);

export function useExplorerLayoutStore() {
	return useSolidStore(explorerLayout);
}

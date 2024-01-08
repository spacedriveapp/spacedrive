import { createMutable } from 'solid-js/store';

import { createPersistedMutable, useSolidStore } from '../solidjs-interop';

export const explorerLayoutStore = createPersistedMutable(
	'sd-explorer-layout',
	createMutable({
		showPathBar: true,
		showImageSlider: true
	})
);

export function useExplorerLayoutStore() {
	return useSolidStore(explorerLayoutStore);
}

import { createMutable } from 'solid-js/store';

import { ExplorerLayout } from '../core';
import { createPersistedMutable, useSolidStore } from '../solid';

export const explorerLayout = createPersistedMutable(
	'sd-explorer-layout',
	createMutable({
		showPathBar: true,
		showTags: true,
		showImageSlider: true,
		// might move this to a store called settings
		defaultView: 'grid' as ExplorerLayout
	})
);

export function useExplorerLayoutStore() {
	return useSolidStore(explorerLayout);
}

import { createMutable } from 'solid-js/store';

import { ExplorerLayout } from '../core';
import { createPersistedMutable, useSolidStore } from '../solid';
import { isTelemetryStateV0 } from './telemetryState';

interface ExplorerLayoutStore {
	showPathBar: boolean;
	showTags: boolean;
	showImageSlider: boolean;
	defaultView: ExplorerLayout;
}

export const explorerLayout = createPersistedMutable<ExplorerLayoutStore>(
	'sd-explorer-layout',
	createMutable<ExplorerLayoutStore>({
		showPathBar: true,
		showTags: true,
		showImageSlider: true,
		// might move this to a store called settings
		defaultView: 'grid' as ExplorerLayout
	}),
	{
		onLoad(value: unknown): ExplorerLayoutStore {
			// we had a bug previously where we saved the telemetry state in the wrong key
			// and we need to erase it from the layout store
			if (isTelemetryStateV0(value)) {
				// remove telemetry state from the layout store
				const { buildInfo, platform, shareFullTelemetry, ...rest } = value;
				return rest as ExplorerLayoutStore;
			}

			return value as ExplorerLayoutStore;
		}
	}
);

export function useExplorerLayoutStore() {
	return useSolidStore(explorerLayout);
}

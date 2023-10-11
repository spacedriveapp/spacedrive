import { useEffect, useRef } from 'react';
import { useRspcLibraryContext } from '@sd/client';

import { useExplorerContext } from '../app/$libraryId/Explorer/Context';
import { useExplorerSearchParams } from '../app/$libraryId/Explorer/util';
import { useOperatingSystem } from './useOperatingSystem';

export const useQuickRescan = () => {
	// subscription so that we can cancel it if in progress
	const quickRescanSubscription = useRef<() => void | undefined>();

	// gotta clean up any rescan subscriptions if the exist
	useEffect(() => () => quickRescanSubscription.current?.(), []);

	const { client } = useRspcLibraryContext();

	const { parent } = useExplorerContext();

	const [{ path }] = useExplorerSearchParams();

	const rescan = () => {
		if (parent?.type === 'Location') {
			quickRescanSubscription.current?.();
			quickRescanSubscription.current = client.addSubscription(
				[
					'locations.quickRescan',
					{
						location_id: parent.location.id,
						sub_path: path ?? ''
					}
				],
				{ onData() {} }
			);
		}
	};

	return rescan;
};

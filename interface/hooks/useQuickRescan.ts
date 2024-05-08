import { useEffect, useRef } from 'react';
import { useRspcLibraryContext } from '@sd/client';
import { toast } from '@sd/ui';
import { explorerStore } from '~/app/$libraryId/Explorer/store';

import { useExplorerContext } from '../app/$libraryId/Explorer/Context';
import { useExplorerSearchParams } from '../app/$libraryId/Explorer/util';
import { useLocale } from './useLocale';

export const useQuickRescan = () => {
	// subscription so that we can cancel it if in progress
	const quickRescanSubscription = useRef<() => void | undefined>();

	// gotta clean up any rescan subscriptions if the exist
	useEffect(() => () => quickRescanSubscription.current?.(), []);
	const { client } = useRspcLibraryContext();
	const explorer = useExplorerContext({ suspense: false });
	const [{ path }] = useExplorerSearchParams();
	const { t } = useLocale();

	const rescan = (id?: number) => {
		const locationId =
			id ?? (explorer?.parent?.type === 'Location' ? explorer.parent.location.id : undefined);

		if (locationId === undefined) return;
		if (Date.now() - explorerStore.quickRescanLastRun < 200) return;

		explorerStore.quickRescanLastRun = Date.now();

		quickRescanSubscription.current?.();
		quickRescanSubscription.current = client.addSubscription(
			[
				'locations.quickRescan',
				{
					location_id: locationId,
					sub_path: path ?? ''
				}
			],
			{ onData() {} }
		);

		toast.success({
			title: t('quick_rescan_started')
		});
	};

	return rescan;
};

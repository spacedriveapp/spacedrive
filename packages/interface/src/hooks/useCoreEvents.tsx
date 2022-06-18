import { transport } from '@sd/client';
import { CoreEvent } from '@sd/core';
import { useContext, useEffect } from 'react';
import { useQueryClient } from 'react-query';

import { AppPropsContext } from '../AppPropsContext';
import { useExplorerState } from './useExplorerState';

export function useCoreEvents() {
	const client = useQueryClient();

	const { addNewThumbnail } = useExplorerState();
	useEffect(() => {
		function handleCoreEvent(e: CoreEvent) {
			switch (e?.key) {
				case 'NewThumbnail':
					addNewThumbnail(e.data.cas_id);
					break;
				case 'InvalidateQuery':
				case 'InvalidateQueryDebounced':
					let query = [e.data.key];
					// TODO: find a way to make params accessible in TS
					// also this method will only work for queries that use the whole params obj as the second key
					// @ts-expect-error
					if (e.data.params) {
						// @ts-expect-error
						query.push(e.data.params);
					}
					client.invalidateQueries(e.data.key);
					break;

				default:
					break;
			}
		}
		// check Tauri Event type
		transport?.on('core_event', handleCoreEvent);

		return () => {
			transport?.off('core_event', handleCoreEvent);
		};

		// listen('core_event', (e: { payload: CoreEvent }) => {
		// });
	}, [transport]);
}

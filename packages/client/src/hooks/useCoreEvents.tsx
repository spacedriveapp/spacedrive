import { transport } from '@sd/client';
import { useExplorerStore } from '@sd/client';
import { CoreEvent } from '@sd/core';
import { useContext, useEffect } from 'react';
import { useQueryClient } from 'react-query';

import { AppPropsContext } from '../../../interface/src/AppPropsContext';

export function useCoreEvents() {
	const client = useQueryClient();

	const { addNewThumbnail } = useExplorerStore();
	useEffect(() => {
		function handleCoreEvent(e: CoreEvent) {
			switch (e?.key) {
				case 'NewThumbnail':
					addNewThumbnail(e.data.cas_id);
					break;
				case 'InvalidateQuery':
				case 'InvalidateQueryDebounced':
					let query = [];
					if (e.data.key === 'LibraryQuery') {
						query = [e.data.params.library_id, e.data.params.query.key];

						// TODO: find a way to make params accessible in TS
						// also this method will only work for queries that use the whole params obj as the second key
						// @ts-expect-error
						if (e.data.params.query.params) {
							// @ts-expect-error
							query.push(e.data.params.query.params);
						}
					} else {
						query = [e.data.key];

						// TODO: find a way to make params accessible in TS
						// also this method will only work for queries that use the whole params obj as the second key
						// @ts-expect-error
						if (e.data.params) {
							// @ts-expect-error
							query.push(e.data.params);
						}
					}

					client.invalidateQueries(query);
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

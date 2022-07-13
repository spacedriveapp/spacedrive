import { CoreEvent } from '@sd/core';
import { useContext, useEffect } from 'react';
import { useQueryClient } from 'react-query';

import { transport, useExplorerStore, useToastNotificationsStore } from '..';
import { usePairingCompleteStore } from '../stores/usePairingCompleteStore';

export function useCoreEvents() {
	const client = useQueryClient();

	const { addNewThumbnail } = useExplorerStore();
	const { addToast } = useToastNotificationsStore();
	const { pairingRequestCallbacks } = usePairingCompleteStore();
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

				case 'PeerPairingRequest':
					addToast({
						title: 'Device requested to pair',
						subtitle: `'${e.data.peer_metadata.name}' wants to pair with your device.`,
						payload: {
							type: 'pairingRequest',
							data: {
								id: e.data.peer_id,
								name: e.data.peer_metadata.name
							}
						}
					});
					break;

				case 'PeerPairingComplete':
					pairingRequestCallbacks.get(e.data.peer_id)?.(e.data.peer_metadata);
					addToast({
						title: 'Pairing Complete',
						subtitle: '',
						payload: {
							type: 'noaction'
						}
					});

				default:
					break;
			}
		}
		// check Tauri Event type
		transport?.on('core_event', handleCoreEvent);

		return () => {
			transport?.off('core_event', handleCoreEvent);
		};
	}, [transport]);
}

import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useSpacedriveClient } from '../context';
import type { Event } from '@sd/ts-client';

/**
 * Hook to set up global device event listeners that invalidate the devices.list query.
 *
 * When DeviceConnected or DeviceDisconnected events are received from the core,
 * this hook invalidates all devices.list queries to ensure the UI stays in sync.
 *
 * This hook should be called once at the root of the app (e.g., in Explorer.tsx).
 */
export function useDeviceEventInvalidation() {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();

	useEffect(() => {
		const handleEvent = (event: Event) => {
			if (typeof event === 'string') return;

			// Check for DeviceConnected or DeviceDisconnected events
			if ('DeviceConnected' in event || 'DeviceDisconnected' in event) {
				// Invalidate all devices.list queries
				// Using predicate to match any query that starts with 'devices.list'
				queryClient.invalidateQueries({
					predicate: (query) => {
						const key = query.queryKey;
						return Array.isArray(key) && key[0] === 'devices.list';
					},
				});
			}
		};

		// Listen to all events from the client
		client.on('spacedrive-event', handleEvent);

		return () => {
			client.off('spacedrive-event', handleEvent);
		};
	}, [client, queryClient]);
}

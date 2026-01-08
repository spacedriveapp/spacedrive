import { useState, useEffect, useRef } from 'react';
import { useLibraryQuery, useSpacedriveClient } from '../../../contexts/SpacedriveContext';
import type { SyncPeerActivity, SyncActivity, SyncState } from '../types';

interface SyncMonitorState {
	currentState: SyncState;
	peers: SyncPeerActivity[];
	recentActivity: SyncActivity[];
	errorCount: number;
	hasActivity: boolean;
}

export function useSyncMonitor() {
	const [state, setState] = useState<SyncMonitorState>({
		currentState: 'Uninitialized',
		peers: [],
		recentActivity: [],
		errorCount: 0,
		hasActivity: false,
	});

	const client = useSpacedriveClient();

	const { data, refetch } = useLibraryQuery({
		type: 'sync.activity',
		input: {},
	});

	const refetchRef = useRef(refetch);
	useEffect(() => {
		refetchRef.current = refetch;
	}, [refetch]);

	useEffect(() => {
		if (data) {
			const stateValue = data.currentState;
			let normalizedState: SyncState;

			if (typeof stateValue === 'string') {
				normalizedState = stateValue as SyncState;
			} else if (typeof stateValue === 'object' && stateValue !== null) {
				if ('Backfilling' in stateValue) {
					normalizedState = 'Backfilling';
				} else if ('CatchingUp' in stateValue) {
					normalizedState = 'CatchingUp';
				} else {
					normalizedState = 'Uninitialized';
				}
			} else {
				normalizedState = 'Uninitialized';
			}

			setState((prev) => ({
				...prev,
				currentState: normalizedState,
				peers: data.peers.map((p) => ({
					deviceId: p.deviceId,
					deviceName: p.deviceName,
					isOnline: p.isOnline,
					lastSeen: p.lastSeen,
					entriesReceived: p.entriesReceived,
					bytesReceived: p.bytesReceived,
					bytesSent: p.bytesSent,
					watermarkLagMs: p.watermarkLagMs,
				})),
				errorCount: data.errorCount,
				hasActivity: data.peers.some((p) => p.isOnline),
			}));
		}
	}, [data]);

	useEffect(() => {
		if (!client) return;

		let unsubscribe: (() => void) | undefined;
		let isCancelled = false;

		const handleEvent = (event: any) => {
			if ('SyncStateChanged' in event) {
				const { newState } = event.SyncStateChanged;
				setState((prev) => ({ ...prev, currentState: newState }));
			} else if ('SyncActivity' in event) {
				const activity = event.SyncActivity;
				const activityType = activity.activityType;

				let eventType: SyncActivity['eventType'] = 'broadcast';
				let description = 'Activity';

				if ('BroadcastSent' in activityType) {
					eventType = 'broadcast';
					description = `Broadcast ${activityType.BroadcastSent.changes} changes`;
				} else if ('ChangesReceived' in activityType) {
					eventType = 'received';
					description = `Received ${activityType.ChangesReceived.changes} changes`;
				} else if ('ChangesApplied' in activityType) {
					eventType = 'applied';
					description = `Applied ${activityType.ChangesApplied.changes} changes`;
				} else if ('BackfillStarted' in activityType) {
					eventType = 'backfill';
					description = 'Backfill started';
				} else if ('BackfillCompleted' in activityType) {
					eventType = 'backfill';
					description = `Backfill completed (${activityType.BackfillCompleted.records} records)`;
				} else if ('CatchUpStarted' in activityType) {
					eventType = 'backfill';
					description = 'Catch-up started';
				} else if ('CatchUpCompleted' in activityType) {
					eventType = 'backfill';
					description = 'Catch-up completed';
				}

				setState((prev) => ({
					...prev,
					recentActivity: [
						{
							timestamp: activity.timestamp,
							eventType,
							peerDeviceId: activity.peerDeviceId,
							description,
						},
						...prev.recentActivity.slice(0, 49),
					],
				}));
			} else if ('SyncConnectionChanged' in event) {
				const { peerDeviceId, peerName, connected } = event.SyncConnectionChanged;

				setState((prev) => ({
					...prev,
					peers: prev.peers.map((p) =>
						p.deviceId === peerDeviceId ? { ...p, isOnline: connected } : p
					),
					hasActivity: connected || prev.peers.some((p) => p.isOnline),
					recentActivity: [
						{
							timestamp: event.SyncConnectionChanged.timestamp,
							eventType: 'connection',
							peerDeviceId,
							description: `${peerName} ${connected ? 'connected' : 'disconnected'}`,
						},
						...prev.recentActivity.slice(0, 49),
					],
				}));
			} else if ('SyncError' in event) {
				const { message } = event.SyncError;
				setState((prev) => ({
					...prev,
					errorCount: prev.errorCount + 1,
					recentActivity: [
						{
							timestamp: event.SyncError.timestamp,
							eventType: 'error',
							peerDeviceId: event.SyncError.peerDeviceId,
							description: message,
						},
						...prev.recentActivity.slice(0, 49),
					],
				}));
			} else {
				refetchRef.current();
			}
		};

		const filter = {
			event_types: [
				'SyncStateChanged',
				'SyncActivity',
				'SyncConnectionChanged',
				'SyncError',
			],
		};

		client.subscribeFiltered(filter, handleEvent).then((unsub) => {
			if (isCancelled) {
				unsub();
			} else {
				unsubscribe = unsub;
			}
		});

		return () => {
			isCancelled = true;
			unsubscribe?.();
		};
	}, [client]);

	return state;
}
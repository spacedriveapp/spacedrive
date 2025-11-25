export interface SyncPeerActivity {
	deviceId: string;
	deviceName: string;
	isOnline: boolean;
	lastSeen: string;
	entriesReceived: number;
	bytesReceived: number;
	bytesSent: number;
	watermarkLagMs?: number;
}

export interface SyncActivity {
	timestamp: string;
	eventType: 'broadcast' | 'received' | 'applied' | 'backfill' | 'error' | 'connection';
	peerDeviceId?: string;
	description: string;
}

export type SyncState = 'Uninitialized' | 'Backfilling' | 'CatchingUp' | 'Ready' | 'Paused';

export const PEER_CARD_HEIGHT = 80;
export const ACTIVITY_HEIGHT = 40;

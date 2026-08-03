export type ConnectionMethod = 'LocalNetwork' | 'DirectInternet' | 'RelayProxy';

export type ConnectionBadgeState =
	| ConnectionMethod
	| 'Offline'
	| 'Current'
	| 'Connecting';

interface ConnectionStateInput {
	method?: ConnectionMethod | null;
	online: boolean;
	current: boolean;
}

export function getConnectionBadgeState({
	method,
	online,
	current
}: ConnectionStateInput): ConnectionBadgeState {
	if (current) return 'Current';
	if (!online) return 'Offline';
	return method ?? 'Connecting';
}

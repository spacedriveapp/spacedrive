import type { Platform } from './Platform';

export interface Client {
	uuid: string;
	name: string;
	platform: Platform;
	tcp_address: string;
	last_seen: string;
	last_synchronized: string;
}

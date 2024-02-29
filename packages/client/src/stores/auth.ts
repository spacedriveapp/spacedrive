import { RSPCError } from '@oscartbeaumont-sd/rspc-client';
import { createMutable } from 'solid-js/store';

import { nonLibraryClient } from '../rspc';
import { useSolidStore } from '../solid';

interface Store {
	state: { status: 'loading' | 'notLoggedIn' | 'loggingIn' | 'loggedIn' | 'loggingOut' };
}

export interface ProviderConfig {
	start(key: string): any;
	finish?(ret: any): void;
}

// inner object so we can overwrite it in one assignment
const store = createMutable<Store>({
	state: {
		status: 'loading'
	}
});

export function useStateSnapshot() {
	return useSolidStore(store).state;
}

nonLibraryClient
	.query(['auth.me'])
	.then(() => (store.state = { status: 'loggedIn' }))
	.catch((e) => {
		if (e instanceof RSPCError && e.code === 401) {
			// TODO: handle error?
		}
		store.state = { status: 'notLoggedIn' };
	});

type CallbackStatus = 'success' | { error: string } | 'cancel';
const loginCallbacks = new Set<(status: CallbackStatus) => void>();

function onError(error: string) {
	loginCallbacks.forEach((cb) => cb({ error }));
}

export function login(config: ProviderConfig) {
	if (store.state.status !== 'notLoggedIn') return;

	store.state = { status: 'loggingIn' };

	let authCleanup = nonLibraryClient.addSubscription(['auth.loginSession'], {
		onData(data) {
			if (data === 'Complete') {
				config.finish?.(authCleanup);
				loginCallbacks.forEach((cb) => cb('success'));
			} else if ('Error' in data) onError(data.Error);
			else {
				authCleanup = config.start(data.Start.verification_url_complete);
			}
		},
		onError(e) {
			onError(e.message);
		}
	});

	return new Promise<void>((res, rej) => {
		const cb = async (status: CallbackStatus) => {
			loginCallbacks.delete(cb);

			if (status === 'success') {
				store.state = { status: 'loggedIn' };
				nonLibraryClient.query(['auth.me']);
				res();
			} else {
				store.state = { status: 'notLoggedIn' };
				rej(JSON.stringify(status));
			}
		};
		loginCallbacks.add(cb);
	});
}

export function logout() {
	store.state = { status: 'loggingOut' };
	nonLibraryClient.mutation(['auth.logout']);
	nonLibraryClient.query(['auth.me']);
	store.state = { status: 'notLoggedIn' };
}

export function cancel() {
	loginCallbacks.forEach((cb) => cb('cancel'));
	loginCallbacks.clear();
	store.state = { status: 'notLoggedIn' };
}

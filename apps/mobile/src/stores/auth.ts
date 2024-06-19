import { RSPCError } from '@oscartbeaumont-sd/rspc-client';
import { nonLibraryClient, useSolidStore } from '@sd/client';
import { Linking } from 'react-native';
import { createMutable } from 'solid-js/store';

interface Store {
	state: { status: 'loading' | 'notLoggedIn' | 'loggingIn' | 'loggedIn' | 'loggingOut' };
}

// inner object so we can overwrite it in one assignment
const store = createMutable<Store>({
	state: {
		status: 'loading'
	}
});

export function useAuthStateSnapshot() {
	return useSolidStore(store).state;
}

nonLibraryClient
	.query(['auth.me'])
	.then(() => (store.state = { status: 'loggedIn' }))
	.catch((e) => {
		if (e instanceof RSPCError && e.code === 401) {
			// TODO: handle error?
			console.error("error", e);
		}
		store.state = { status: 'notLoggedIn' };
	});

type CallbackStatus = 'success' | { error: string } | 'cancel';
const loginCallbacks = new Set<(status: CallbackStatus) => void>();

function onError(error: string) {
	loginCallbacks.forEach((cb) => cb({ error }));
}

export function login() {
	if (store.state.status !== 'notLoggedIn') return;

	store.state = { status: 'loggingIn' };

	let authCleanup = nonLibraryClient.addSubscription(['auth.loginSession'], {
		onData(data) {
			if (data === 'Complete') {
				loginCallbacks.forEach((cb) => cb('success'));
			} else if ('Error' in data) {
				console.error('[auth] error: ', data.Error);
				onError(data.Error);
			} else {
				console.log('[auth] verification url: ', data.Start.verification_url_complete);
				Promise.resolve()
					.then(() => Linking.openURL(data.Start.verification_url_complete))
					.then(
						(res) => {
							authCleanup = res;
						},
						(e) => onError(e.message)
					);
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

export function set_logged_in() {
	store.state = { status: 'loggedIn' };
}

export function logout() {
	store.state = { status: 'loggingOut' };
	nonLibraryClient.mutation(['auth.logout']);
	nonLibraryClient.query(['auth.me']);
	store.state = { status: 'notLoggedIn' };
}

export async function cancel() {
	await loginCallbacks.forEach(async (cb) => await cb('cancel'));
	await loginCallbacks.clear();
	store.state = { status: 'notLoggedIn' };
}

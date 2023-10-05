import { RSPCError } from '@rspc/client';
import { createContext, PropsWithChildren, useContext, useEffect, useRef, useState } from 'react';
import {
	nonLibraryClient,
	useBridgeMutation,
	useBridgeQuery,
	useBridgeSubscription
} from '@sd/client';

import { usePlatform } from '..';

const Context = createContext<ReturnType<typeof useContextValue> | null>(null);

function useContextValue() {
	const [state, setState] = useState<'loading' | 'notLoggedIn' | 'loggingIn' | 'loggedIn'>(
		'loading'
	);

	const me = useBridgeQuery(['auth.me'], { retry: false });

	// initial auth check
	useEffect(() => {
		setState('loading');

		const controller = new AbortController();

		const run = async () => {
			try {
				await nonLibraryClient.query(['auth.me'], {
					signal: controller.signal
				});

				if (!controller.signal.aborted) {
					setState('loggedIn');
				}
			} catch (e) {
				if (e instanceof RSPCError && e.code === 401) {
					// TODO: handle error?
				}
				setState('notLoggedIn');
			}
		};

		run();

		return () => {
			controller.abort();
		};
	}, []);

	const logout = useBridgeMutation(['auth.logout']);

	const platform = usePlatform();
	const ret = useRef(null);
	const loginCallbacks = useRef(new Set<(status: 'success' | 'error') => void>());

	function onError() {
		loginCallbacks.current.forEach((cb) => cb('error'));
	}

	useBridgeSubscription(['auth.loginSession'], {
		enabled: state === 'loggingIn',
		onData(data) {
			if (data === 'Complete') {
				platform.auth.finish?.(ret.current);
				loginCallbacks.current.forEach((cb) => cb('success'));
			} else if (data === 'Error') onError();
			else {
				ret.current = platform.auth.start(data.Start.verification_url_complete);
			}
		},
		onError
	});

	if (state === 'notLoggedIn') {
		return {
			state,
			login() {
				setState('loggingIn');

				return new Promise<void>((res, rej) => {
					const cb = async (status: 'success' | 'error') => {
						loginCallbacks.current.delete(cb);

						if (status === 'success') {
							await me.refetch();
							setState('loggedIn');
							res();
						} else {
							setState('notLoggedIn');
							rej();
						}
					};
					loginCallbacks.current.add(cb);
				});
			}
		};
	} else if (state === 'loggedIn') {
		return {
			state,
			logoutLoading: logout.isLoading,
			async logout() {
				await logout.mutateAsync(undefined);
				await me.refetch();
				setState('notLoggedIn');
			}
		};
	} else return { state };
}

export function AuthProvider({ children }: PropsWithChildren) {
	return <Context.Provider value={useContextValue()}>{children}</Context.Provider>;
}

export function useAuthContext() {
	const context = useContext(Context);

	if (!context) throw new Error('AuthProvider not found');

	return context;
}

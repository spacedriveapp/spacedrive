import { useRef, useState } from 'react';
import { useBridgeSubscription } from '@sd/client';
import { Button } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

type State =
	| { status: 'Idle' }
	| { status: 'LoggingIn' }
	| {
			status: 'LoggedIn';
			token: string;
	  };

export function LoginButton() {
	const [state, setState] = useState<State>({ status: 'Idle' });

	const platform = usePlatform();

	const ret = useRef(null);

	useBridgeSubscription(['auth.loginSession'], {
		enabled: state.status === 'LoggingIn',
		onData(data) {
			if ('Start' in data) {
				const key = data.Start;
				ret.current = platform.auth.start(key);
			} else {
				setState({ status: 'LoggedIn', token: data.Token });
				platform.auth.finish?.(ret.current);
			}
		},
		onError() {
			setState({ status: 'Idle' });
		}
	});

	return (
		<Button
			variant={state.status === 'LoggedIn' ? 'outline' : 'accent'}
			disabled={state.status !== 'Idle'}
			onClick={() => setState({ status: 'LoggingIn' })}
		>
			{state.status === 'Idle'
				? 'Login'
				: state.status === 'LoggingIn'
				? 'Logging In...'
				: 'Logged In'}
		</Button>
	);
}

import { useRef, useState } from 'react';
import { useBridgeSubscription } from '@sd/client';
import { Button, ButtonProps } from '@sd/ui';

import { usePlatform } from '..';

type State = { status: 'Idle' } | { status: 'LoggingIn' };

interface Props extends ButtonProps {
	onLogin?(): void;
}

export function LoginButton({ children, onLogin, ...props }: Props) {
	const [state, setState] = useState<State>({ status: 'Idle' });

	const platform = usePlatform();

	const ret = useRef(null);

	useBridgeSubscription(['auth.loginSession'], {
		enabled: state.status === 'LoggingIn',
		onData(data) {
			if (data === 'Complete') {
				onLogin?.();
				platform.auth.finish?.(ret.current);
			} else if (data === 'Error') setState({ status: 'Idle' });
			else {
				ret.current = platform.auth.start(data.Start.verification_url_complete);
			}
		},
		onError() {
			setState({ status: 'Idle' });
		}
	});

	return (
		<Button
			variant="accent"
			disabled={state.status !== 'Idle'}
			onClick={() => setState({ status: 'LoggingIn' })}
			{...props}
		>
			{state.status === 'Idle' ? children || 'Log in' : 'Logging In...'}
		</Button>
	);
}

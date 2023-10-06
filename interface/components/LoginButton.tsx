import { auth } from '@sd/client';
import { Button, ButtonProps } from '@sd/ui';

import { usePlatform } from '..';

export function LoginButton({ children, ...props }: { onLogin?(): void } & ButtonProps) {
	const authState = auth.useStateSnapshot();

	const platform = usePlatform();

	return (
		<Button
			variant="accent"
			disabled={authState.status === 'loggingIn'}
			onClick={async () => {
				await auth.login(platform.auth);

				props.onLogin?.();
			}}
			{...props}
		>
			{authState.status !== 'loggingIn' ? children || 'Log in' : 'Logging In...'}
		</Button>
	);
}

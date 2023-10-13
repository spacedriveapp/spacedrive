import { auth } from '@sd/client';
import { Button, ButtonProps } from '@sd/ui';

import { usePlatform } from '..';

export function LoginButton({ children, ...props }: { onLogin?(): void } & ButtonProps) {
	const authState = auth.useStateSnapshot();
	const platform = usePlatform();

	return (
		<div className="flex flex-col items-center justify-center">
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
			{authState.status === 'loggingIn' && (
				<a
					href="#"
					onClick={(e) => {
						e.preventDefault();
						auth.cancel();
					}}
					className="light:text-gray-800 mt-2 text-sm dark:text-gray-200"
				>
					Cancel
				</a>
			)}
		</div>
	);
}

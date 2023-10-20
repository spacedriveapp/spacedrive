import clsx from 'clsx';
import { auth } from '@sd/client';
import { Button, ButtonProps } from '@sd/ui';

import { usePlatform } from '..';

export function LoginButton({
	children,
	cancelPosition = 'bottom',
	...props
}: { onLogin?(): void; cancelPosition?: 'bottom' | 'left' } & ButtonProps) {
	const authState = auth.useStateSnapshot();
	const platform = usePlatform();

	return (
		<div
			className={clsx(
				'flex items-center justify-center gap-2',
				cancelPosition === 'bottom' ? 'flex-col' : 'flex-row-reverse'
			)}
		>
			<Button
				variant="accent"
				disabled={authState.status === 'loggingIn'}
				onClick={async () => {
					await auth.login(platform.auth);
					props.onLogin?.();
				}}
				{...props}
			>
				{authState.status !== 'loggingIn' ? children || 'Log in' : 'Logging in...'}
			</Button>
			{authState.status === 'loggingIn' && (
				<button
					onClick={(e) => {
						e.preventDefault();
						auth.cancel();
					}}
					className="text-sm text-ink-faint"
				>
					Cancel
				</button>
			)}
		</div>
	);
}

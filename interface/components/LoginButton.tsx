import { Button, ButtonProps } from '@sd/ui';
import { useAuthContext } from '~/contexts/auth';

export function LoginButton({ children, ...props }: { onLogin?(): void } & ButtonProps) {
	const auth = useAuthContext();

	return (
		<Button
			variant="accent"
			disabled={auth.state === 'loggingIn'}
			onClick={async () => {
				await auth.login?.();
				props.onLogin?.();
			}}
			{...props}
		>
			{auth.state !== 'loggingIn' ? children || 'Log in' : 'Logging In...'}
		</Button>
	);
}
